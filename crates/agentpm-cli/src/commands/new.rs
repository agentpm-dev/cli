use crate::io::download::{download_and_extract_all, download_to, ensure_sha256, extract_tar_gz};
use crate::io::fs;
use crate::manifest::{
    PackageReference, TemplateManifest, load_manifest_value, parse_template_manifest,
    resolve_schema_source, validate_manifest_value, write_lock, write_manifest_pretty_atomic,
};
use crate::prelude::*;
use crate::semver::types::{PackageKind, ResolvePlan};
use crate::workspace::{
    TemplateOriginMetadata, WorkspaceMetadata, WorkspacePackageRoot, WorkspacePackageRoots,
    build_workspace_lock, load_workspace_local_manifests, write_template_metadata,
    write_workspace_metadata,
};
use anyhow::anyhow;
use chrono::Utc;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::task;
use walkdir::WalkDir;

#[derive(clap::Parser, Debug, Clone)]
pub struct NewArgs {
    /// Published template package reference, e.g. @owner/name@0.1.0
    pub template_ref: String,

    /// Optional target directory for the generated project
    pub target_dir: Option<PathBuf>,

    /// Generation-time variable overrides (repeatable)
    #[arg(long = "var", value_name = "KEY=VALUE")]
    pub vars: Vec<String>,

    /// Reduce output
    #[clap(long)]
    pub quiet: bool,

    /// Personal Access Token for headless auth (overrides env/file)
    #[arg(long, value_name = "PAT", env = "AGENTPM_TOKEN")]
    pub token: Option<String>,
}

impl NewArgs {
    pub async fn run(&self, base_url: String) -> Result<()> {
        let template_work = temp_dir("template-new");
        let result = self.run_inner(base_url, &template_work).await;
        let _ = std::fs::remove_dir_all(&template_work);
        result
    }

    async fn run_inner(&self, base_url: String, template_work: &Path) -> Result<()> {
        let cfg = Config::load(base_url.clone())?;
        let auto_quiet = !std::io::stderr().is_terminal();
        let quiet = self.quiet || auto_quiet;

        let mut step = crate::ui::Step::new("Reading credentials", quiet);
        let token = resolve_token(&cfg, self.token.clone())?;
        if let Some(t) = &token {
            step.ok(format!("using {}", mask_token(t)));
        } else {
            step.ok("none (public templates only)");
        }

        let mut client = AgentPmClient::new(base_url)?;
        if let Some(tok) = token {
            client = client.with_token(tok);
        }

        let overrides = parse_var_overrides(&self.vars)?;

        let mut step = crate::ui::Step::new("Resolving template", quiet);
        let template_req = parse_template_ref(&self.template_ref)?;
        let template_resolved = client
            .resolve_install(&agentpm_sdk::models::install::ResolveRequest {
                items: vec![template_req],
            })
            .await?;
        let template_item = template_resolved
            .items
            .first()
            .ok_or_else(|| anyhow!("template resolution returned no items"))?;
        if template_resolved.items.len() != 1
            || template_item.kind != agentpm_sdk::models::install::PackageKind::Template
        {
            return Err(anyhow!(
                "`agentpm new` expected a single template package, got {:?}",
                template_resolved.items
            ));
        }
        step.ok("");

        let mut step = crate::ui::Step::new("Downloading template", quiet);
        let template_init = client.install_init(&template_resolved).await?;
        let template_artifact = template_init
            .artifacts
            .first()
            .ok_or_else(|| anyhow!("template init returned no artifacts"))?;
        let template_cache = template_work.join("template.tgz");
        let template_extract = template_work.join("extracted");
        download_to(
            &reqwest::Client::new(),
            &template_artifact.presigned_url,
            &template_cache,
        )
        .await?;
        ensure_sha256(&template_cache, &template_artifact.integrity).await?;
        extract_tar_gz(&template_cache, &template_extract).await?;
        client.install_finalize(&template_init.session_id).await?;

        let downloaded_manifest_path = template_extract.join("agent.json");
        let (downloaded_manifest_value, _) = load_manifest_value(&downloaded_manifest_path)?;
        let downloaded_manifest = parse_template_manifest(&downloaded_manifest_value)?;
        step.ok("");

        let target_dir =
            determine_target_dir(self.target_dir.clone(), &downloaded_manifest, &overrides)?;
        let mut resolved_vars =
            resolve_template_variables(&downloaded_manifest, &overrides, &target_dir, false)?;
        if !resolved_vars.contains_key("project_name") {
            resolved_vars.insert(
                "project_name".to_string(),
                sanitized_project_name(
                    target_dir
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&downloaded_manifest.name),
                ),
            );
        }

        ensure_safe_target_dir(&target_dir)?;
        let target_existed = target_dir.exists();
        std::fs::create_dir_all(&target_dir)
            .with_context(|| format!("creating {}", target_dir.display()))?;

        let generation_result = async {
            let files_root = validated_files_root(&downloaded_manifest.template.files_root)?;
            let template_source_root = template_extract.join(files_root);

            let mut step = crate::ui::Step::new("Rendering template files", quiet);
            copy_and_render_template(&template_source_root, &target_dir, &resolved_vars, quiet)?;
            validate_scaffold_agent_manifest_locations(&target_dir)?;
            let extra_local_manifests = scan_generated_local_manifests(&target_dir)?;
            step.ok("");

            let mut step = crate::ui::Step::new("Resolving workspace dependencies", quiet);
            let dependency_request = build_dependency_request(
                &downloaded_manifest,
                &target_dir,
                &extra_local_manifests,
            )?;
            let dependency_response = if dependency_request.items.is_empty() {
                agentpm_sdk::models::install::ResolveResponse { items: Vec::new() }
            } else {
                client.resolve_install(&dependency_request).await?
            };
            let plan: ResolvePlan = dependency_response.clone().into();
            step.ok("");

            let mut step = crate::ui::Step::new("Installing workspace dependencies", quiet);
            if !dependency_response.items.is_empty() {
                let init = client
                    .install_init(&agentpm_sdk::models::install::ResolveResponse {
                        items: dependency_response.items.clone(),
                    })
                    .await?;
                let cache_dir = target_dir.join(".agentpm/cache");
                let tools_dir = target_dir.join(".agentpm/tools");
                let agents_dir = target_dir.join(".agentpm/agents");
                fs::ensure_dirs(&[&cache_dir, &tools_dir, &agents_dir])?;
                download_and_extract_all(&init, &cache_dir, &tools_dir, &agents_dir, false, quiet)
                    .await?;
                client.install_finalize(&init.session_id).await?;
            }
            step.ok("");

            let root_manifest = synthesize_root_manifest(
                &downloaded_manifest,
                &resolved_vars,
                &plan,
                target_dir
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&downloaded_manifest.name),
            )?;
            write_manifest_pretty_atomic(&target_dir.join("agent.json"), &root_manifest)?;

            let workspace_metadata = WorkspaceMetadata {
                schema_version: 1,
                manifests: std::iter::once("agent.json".to_string())
                    .chain(extra_local_manifests.into_iter())
                    .collect(),
                package_roots: WorkspacePackageRoots {
                    tools: Vec::new(),
                    agents: resolved_agent_package_roots(&downloaded_manifest, &plan)?,
                },
            };
            write_workspace_metadata(&target_dir, &workspace_metadata)?;

            validate_generated_manifests(&target_dir, &workspace_metadata).await?;
            let local_manifests = load_workspace_local_manifests(&target_dir, &workspace_metadata)?;
            let lock = build_workspace_lock(
                &local_manifests,
                &workspace_metadata.package_roots,
                &plan,
                &target_dir,
            )?;
            write_lock(&target_dir, &lock)?;

            write_template_metadata(
                &target_dir,
                &TemplateOriginMetadata {
                    schema_version: 1,
                    source: "registry".to_string(),
                    kind: "template".to_string(),
                    name: downloaded_manifest.name.clone(),
                    version: downloaded_manifest.version.clone(),
                    integrity: template_artifact.integrity.clone(),
                    generated_at: Utc::now().to_rfc3339(),
                    variables: resolved_vars.clone(),
                },
            )?;

            print_success(&target_dir, &downloaded_manifest, quiet);
            Ok(())
        }
        .await;

        if generation_result.is_err() {
            cleanup_failed_target_dir(&target_dir, target_existed);
        }

        generation_result
    }
}

fn parse_var_overrides(entries: &[String]) -> Result<BTreeMap<String, String>> {
    let mut vars = BTreeMap::new();
    for entry in entries {
        let Some((key, value)) = entry.split_once('=') else {
            return Err(anyhow!(
                "--var values must be in key=value form (got {})",
                entry
            ));
        };
        if key.trim().is_empty() {
            return Err(anyhow!("--var keys must not be empty"));
        }
        vars.insert(key.trim().to_string(), value.to_string());
    }
    Ok(vars)
}

fn parse_template_ref(spec: &str) -> Result<agentpm_sdk::models::install::PackageRequirement> {
    let s = spec.trim();
    if !s.starts_with('@') || !s[1..].contains('/') {
        return Err(anyhow!(
            "template ref must be '@owner/name' or '@owner/name@version'"
        ));
    }

    let (name, range) = match s.rfind('@') {
        Some(last_at)
            if last_at > 0 && s[..last_at].starts_with('@') && s[..last_at].contains('/') =>
        {
            let range = if last_at == s.len() - 1 {
                "*".to_string()
            } else {
                s[last_at + 1..].to_string()
            };
            (s[..last_at].to_string(), range)
        }
        _ => (s.to_string(), "*".to_string()),
    };

    Ok(agentpm_sdk::models::install::PackageRequirement {
        kind: agentpm_sdk::models::install::PackageKind::Template,
        name,
        range,
    })
}

fn determine_target_dir(
    explicit: Option<PathBuf>,
    manifest: &TemplateManifest,
    overrides: &BTreeMap<String, String>,
) -> Result<PathBuf> {
    if let Some(target) = explicit {
        return Ok(target);
    }

    if let Some(project_name) = overrides.get("project_name") {
        return Ok(PathBuf::from(project_name));
    }

    if let Some(default) = manifest
        .template
        .variables
        .iter()
        .find(|var| var.name == "project_name")
        .and_then(|var| var.default.clone())
    {
        return Ok(PathBuf::from(default));
    }

    Ok(PathBuf::from(&manifest.name))
}

fn resolve_template_variables(
    manifest: &TemplateManifest,
    overrides: &BTreeMap<String, String>,
    target_dir: &Path,
    interactive: bool,
) -> Result<BTreeMap<String, String>> {
    // Milestone 4 intentionally ships a deterministic non-interactive path:
    // required variables must come from `--var` or manifest defaults. Prompting
    // is deferred to Milestone 4b so the initial `agentpm new` scope stays
    // focused on safe generation, workspace topology, and lock correctness.
    if interactive {
        return Err(anyhow!("interactive template prompts are not implemented"));
    }

    let target_name = target_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&manifest.name);

    let mut resolved = BTreeMap::new();
    for variable in &manifest.template.variables {
        let value = overrides
            .get(&variable.name)
            .cloned()
            .or_else(|| {
                if variable.name == "project_name" {
                    Some(sanitized_project_name(target_name))
                } else {
                    None
                }
            })
            .or_else(|| variable.default.clone());

        match value {
            Some(value) => {
                resolved.insert(variable.name.clone(), value);
            }
            None if variable.required => {
                return Err(anyhow!(
                    "required template variable {} has no value; pass --var {}=...",
                    variable.name,
                    variable.name
                ));
            }
            None => {}
        }
    }

    if !resolved.contains_key("project_name") {
        resolved.insert(
            "project_name".to_string(),
            sanitized_project_name(target_name),
        );
    }

    Ok(resolved)
}

fn sanitized_project_name(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
        } else if (lower == '-' || lower == '_' || lower == ' ') && !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "generated-agent".to_string()
    } else {
        trimmed.to_string()
    }
}

fn ensure_safe_target_dir(target_dir: &Path) -> Result<()> {
    if target_dir.exists() {
        let mut entries = std::fs::read_dir(target_dir)
            .with_context(|| format!("reading {}", target_dir.display()))?;
        if entries.next().transpose()?.is_some() {
            return Err(anyhow!(
                "target directory {} already exists and is not empty",
                target_dir.display()
            ));
        }
    }
    Ok(())
}

fn cleanup_failed_target_dir(target_dir: &Path, existed_before: bool) {
    if !target_dir.exists() {
        return;
    }

    if existed_before {
        if let Ok(entries) = std::fs::read_dir(target_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let _ = std::fs::remove_dir_all(path);
                } else {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    } else {
        let _ = std::fs::remove_dir_all(target_dir);
    }
}

fn validated_files_root(files_root: &str) -> Result<PathBuf> {
    if files_root.trim().is_empty() {
        return Err(anyhow!("template.files_root must not be empty"));
    }
    let path = Path::new(files_root);
    if path.is_absolute() {
        return Err(anyhow!("template.files_root must be relative"));
    }
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(anyhow!("template.files_root must not contain .."));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(anyhow!("template.files_root must be relative"));
            }
        }
    }
    Ok(clean)
}

fn copy_and_render_template(
    source_root: &Path,
    target_dir: &Path,
    vars: &BTreeMap<String, String>,
    quiet: bool,
) -> Result<()> {
    for entry in WalkDir::new(source_root) {
        let entry = entry?;
        let src = entry.path();
        let rel = src
            .strip_prefix(source_root)
            .with_context(|| format!("stripping prefix from {}", src.display()))?;
        let dest = target_dir.join(rel);

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest)?;
            continue;
        }

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let data = std::fs::read(src)?;
        match String::from_utf8(data.clone()) {
            Ok(text) => {
                let rendered = render_template_text(&text, vars)?;
                if !quiet && !rendered.preserved_placeholders.is_empty() {
                    eprintln!(
                        "Warning: preserved unknown template placeholder(s) {} in {}",
                        rendered.preserved_placeholders.join(", "),
                        rel.display()
                    );
                }
                std::fs::write(&dest, rendered.text.as_bytes())?;
            }
            Err(_) => {
                std::fs::write(&dest, data)?;
            }
        }

        let permissions = std::fs::metadata(src)?.permissions();
        std::fs::set_permissions(&dest, permissions)?;
    }
    Ok(())
}

struct RenderedTemplateText {
    text: String,
    preserved_placeholders: Vec<String>,
}

fn render_template_text(
    text: &str,
    vars: &BTreeMap<String, String>,
) -> Result<RenderedTemplateText> {
    let mut out = String::with_capacity(text.len());
    let mut preserved_placeholders = Vec::new();
    let mut idx = 0usize;

    while let Some(start) = text[idx..].find("{{") {
        let start_idx = idx + start;
        out.push_str(&text[idx..start_idx]);
        let after_start = start_idx + 2;
        let Some(end_rel) = text[after_start..].find("}}") else {
            return Err(anyhow!("unterminated template placeholder"));
        };
        let end_idx = after_start + end_rel;
        let key = text[after_start..end_idx].trim();
        // `agentpm new` only substitutes variables declared by the template
        // manifest. Other `{{ ... }}` text is preserved so authors can include
        // docs, examples, or other templating syntax without escaping it.
        if let Some(value) = vars.get(key) {
            out.push_str(value);
        } else {
            preserved_placeholders.push(format!("{{{{ {} }}}}", key));
            out.push_str(&text[start_idx..end_idx + 2]);
        }
        idx = end_idx + 2;
    }
    out.push_str(&text[idx..]);
    Ok(RenderedTemplateText {
        text: out,
        preserved_placeholders,
    })
}

fn build_dependency_request(
    manifest: &TemplateManifest,
    target_dir: &Path,
    extra_local_manifests: &[String],
) -> Result<agentpm_sdk::models::install::ResolveRequest> {
    let mut items = Vec::new();

    for tool in &manifest.template.dependencies.tools {
        let (name, range) = package_ref_parts(tool)?;
        items.push(agentpm_sdk::models::install::PackageRequirement {
            kind: agentpm_sdk::models::install::PackageKind::Tool,
            name,
            range,
        });
    }
    for agent in &manifest.template.dependencies.agents {
        let (name, range) = package_ref_parts(agent)?;
        items.push(agentpm_sdk::models::install::PackageRequirement {
            kind: agentpm_sdk::models::install::PackageKind::Agent,
            name,
            range,
        });
    }

    for item in local_manifest_dependency_items(target_dir, extra_local_manifests)? {
        items.push(item);
    }

    Ok(agentpm_sdk::models::install::ResolveRequest { items })
}

fn local_manifest_dependency_items(
    target_dir: &Path,
    extra_local_manifests: &[String],
) -> Result<Vec<agentpm_sdk::models::install::PackageRequirement>> {
    let mut items = Vec::new();
    for rel_path in extra_local_manifests {
        let manifest_path = target_dir.join(rel_path);
        let (manifest_value, _) = load_manifest_value(&manifest_path).with_context(|| {
            format!(
                "loading generated local manifest {}",
                manifest_path.display()
            )
        })?;
        let desired =
            crate::semver::types::DesiredSet::from_cli_or_agent_json(&manifest_value, None, false)?;
        for item in desired.items {
            items.push(agentpm_sdk::models::install::PackageRequirement {
                kind: match item.kind {
                    PackageKind::Tool => agentpm_sdk::models::install::PackageKind::Tool,
                    PackageKind::Agent => agentpm_sdk::models::install::PackageKind::Agent,
                },
                name: item.name,
                range: item.range,
            });
        }
    }
    Ok(items)
}

fn package_ref_parts(reference: &PackageReference) -> Result<(String, String)> {
    match reference {
        PackageReference::String(raw) => {
            let Some(last_at) = raw.rfind('@') else {
                return Ok((raw.clone(), "*".to_string()));
            };
            if last_at == 0 {
                return Ok((raw.clone(), "*".to_string()));
            }
            Ok((raw[..last_at].to_string(), raw[last_at + 1..].to_string()))
        }
        PackageReference::Object { name, version } => Ok((
            name.clone(),
            version.clone().unwrap_or_else(|| "*".to_string()),
        )),
    }
}

fn synthesize_root_manifest(
    template_manifest: &TemplateManifest,
    resolved_vars: &BTreeMap<String, String>,
    plan: &ResolvePlan,
    target_name: &str,
) -> Result<Value> {
    let tool_refs = resolved_tool_manifest_refs(template_manifest, plan)?;
    Ok(json!({
        "kind": "agent",
        "name": resolved_vars
            .get("project_name")
            .cloned()
            .unwrap_or_else(|| sanitized_project_name(target_name)),
        "version": "0.1.0",
        "description": format!(
            "Generated from {}@{}.",
            template_manifest.name, template_manifest.version
        ),
        "tools": tool_refs,
        "skills": [],
        "knowledge": [],
        "memory": [],
        "profiles": []
    }))
}

fn resolved_tool_manifest_refs(
    template_manifest: &TemplateManifest,
    plan: &ResolvePlan,
) -> Result<Vec<Value>> {
    let direct_tool_refs = template_manifest
        .template
        .dependencies
        .tools
        .iter()
        .map(package_ref_parts)
        .collect::<Result<Vec<_>>>()?;

    let packages = plan_packages_by_name(plan, PackageKind::Tool);
    let mut out = Vec::new();
    for (name, range) in direct_tool_refs {
        let resolved = resolve_matching_plan_item(&packages, &name, &range)?.ok_or_else(|| {
            anyhow!(
                "resolved tool dependency {}@{} missing from plan",
                name,
                range
            )
        })?;
        out.push(json!({
            "name": resolved.name,
            "version": resolved.version,
        }));
    }
    Ok(out)
}

fn resolved_agent_package_roots(
    template_manifest: &TemplateManifest,
    plan: &ResolvePlan,
) -> Result<Vec<WorkspacePackageRoot>> {
    let direct_agent_refs = template_manifest
        .template
        .dependencies
        .agents
        .iter()
        .map(package_ref_parts)
        .collect::<Result<Vec<_>>>()?;
    let packages = plan_packages_by_name(plan, PackageKind::Agent);
    let mut roots = Vec::new();
    for (name, range) in direct_agent_refs {
        let resolved = resolve_matching_plan_item(&packages, &name, &range)?.ok_or_else(|| {
            anyhow!(
                "resolved agent dependency {}@{} missing from plan",
                name,
                range
            )
        })?;
        roots.push(WorkspacePackageRoot {
            name: resolved.name.clone(),
            version: resolved.version.clone(),
        });
    }
    Ok(roots)
}

fn plan_packages_by_name(
    plan: &ResolvePlan,
    kind: PackageKind,
) -> BTreeMap<String, Vec<&crate::semver::types::ResolvedPackage>> {
    let mut grouped = BTreeMap::new();
    for item in &plan.items {
        if item.kind == kind {
            grouped
                .entry(item.name.clone())
                .or_insert_with(Vec::new)
                .push(item);
        }
    }
    grouped
}

fn resolve_matching_plan_item<'a>(
    packages: &'a BTreeMap<String, Vec<&'a crate::semver::types::ResolvedPackage>>,
    name: &str,
    range: &str,
) -> Result<Option<&'a crate::semver::types::ResolvedPackage>> {
    let exact = if range == "*" {
        None
    } else {
        semver::Version::parse(range).ok()
    };
    let req = if exact.is_none() && range != "*" {
        Some(
            semver::VersionReq::parse(range)
                .with_context(|| format!("invalid semver range {}", range))?,
        )
    } else {
        None
    };

    let mut candidates: Vec<(semver::Version, &'a crate::semver::types::ResolvedPackage)> =
        Vec::new();
    for item in packages.get(name).cloned().unwrap_or_default() {
        let version = match semver::Version::parse(&item.version) {
            Ok(version) => version,
            Err(_) => continue,
        };
        let is_match = if let Some(exact) = &exact {
            &version == exact
        } else if let Some(req) = &req {
            req.matches(&version)
        } else {
            true
        };
        if is_match {
            candidates.push((version, item));
        }
    }

    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(candidates.pop().map(|(_, item)| item))
}

fn scan_generated_local_manifests(target_dir: &Path) -> Result<Vec<String>> {
    let agents_dir = target_dir.join("agents");
    if !agents_dir.exists() {
        return Ok(Vec::new());
    }
    let mut manifests = Vec::new();
    for entry in WalkDir::new(&agents_dir) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(name) = entry.path().file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if name == "agent.json" || name.ends_with(".agent.json") {
            let rel = entry
                .path()
                .strip_prefix(target_dir)
                .with_context(|| format!("stripping {}", entry.path().display()))?;
            manifests.push(path_to_slash_string(rel));
        }
    }
    manifests.sort();
    manifests.dedup();
    Ok(manifests)
}

fn validate_scaffold_agent_manifest_locations(target_dir: &Path) -> Result<()> {
    for entry in WalkDir::new(target_dir) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(target_dir)
            .with_context(|| format!("stripping {}", entry.path().display()))?;
        let rel_str = path_to_slash_string(rel);
        let Some(name) = entry.path().file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if (name == "agent.json" || name.ends_with(".agent.json"))
            && !rel_str.starts_with("agents/")
        {
            return Err(anyhow!(
                "generated local agent manifests must live under agents/; found {}",
                rel_str
            ));
        }
    }
    Ok(())
}

async fn validate_generated_manifests(
    target_dir: &Path,
    workspace_metadata: &WorkspaceMetadata,
) -> Result<()> {
    let target_dir = target_dir.to_path_buf();
    let workspace_metadata = workspace_metadata.clone();
    task::spawn_blocking(move || {
        validate_generated_manifests_blocking(&target_dir, &workspace_metadata)
    })
    .await?
}

fn validate_generated_manifests_blocking(
    target_dir: &Path,
    workspace_metadata: &WorkspaceMetadata,
) -> Result<()> {
    let schema_source = resolve_schema_source(None);
    let manifest_paths = std::iter::once("agent.json".to_string()).chain(
        workspace_metadata
            .manifests
            .iter()
            .filter(|p| p.as_str() != "agent.json")
            .cloned(),
    );

    for rel_path in manifest_paths {
        let path = target_dir.join(&rel_path);
        let (mut value, _) = load_manifest_value(&path)?;
        let (ok, issues) =
            validate_manifest_value(&schema_source, &path.to_string_lossy(), &mut value, false)?;
        if !ok {
            return Err(anyhow!(
                "generated manifest {} is invalid: {issues:#?}",
                rel_path
            ));
        }
        if value.get("kind").and_then(Value::as_str) != Some("agent") {
            return Err(anyhow!(
                "generated manifest {} must be kind=\"agent\"",
                rel_path
            ));
        }
        if value.get("agents").is_some() {
            return Err(anyhow!(
                "generated manifest {} must not include recursive agents dependencies",
                rel_path
            ));
        }
    }
    Ok(())
}

fn print_success(target_dir: &Path, template_manifest: &TemplateManifest, quiet: bool) {
    crate::ui::Step::final_msg(
        format!("Generated workspace ✓ {}", target_dir.display()),
        quiet,
    );
    for entrypoint in &template_manifest.template.entrypoints {
        println!("{}: {}", entrypoint.label, entrypoint.command);
    }
}

fn path_to_slash_string(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("agentpm-new-{label}-{nanos}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::Body;
    use axum::extract::State;
    use axum::http::{Response, StatusCode};
    use axum::routing::{get, post};
    use serde_json::Value;
    use std::net::SocketAddr;
    use std::sync::Arc;

    #[test]
    fn parses_var_overrides() {
        let parsed = parse_var_overrides(&["project_name=my-app".to_string()]).unwrap();
        assert_eq!(parsed["project_name"], "my-app");
    }

    #[test]
    fn parse_template_ref_allows_unversioned_package_name() {
        let req = parse_template_ref("@zack/research-template").unwrap();
        assert_eq!(req.name, "@zack/research-template");
        assert_eq!(req.range, "*");
        assert_eq!(
            req.kind,
            agentpm_sdk::models::install::PackageKind::Template
        );
    }

    #[test]
    fn render_template_text_preserves_unknown_placeholders() {
        let rendered = render_template_text("hello {{ missing }}", &BTreeMap::new()).unwrap();
        assert_eq!(rendered.text, "hello {{ missing }}");
        assert_eq!(rendered.preserved_placeholders, vec!["{{ missing }}"]);
    }

    #[test]
    fn determine_target_dir_uses_project_name_default() {
        let manifest = parse_template_manifest(&json!({
            "kind": "template",
            "name": "research-assistant",
            "version": "0.1.0",
            "description": "Template",
            "template": {
                "display_name": "Research Assistant",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "stack": ["python"],
                "files_root": "template",
                "variables": [{"name":"project_name","required":true,"default":"my-agent"}],
                "dependencies": {"tools":[],"agents":[]},
                "entrypoints": []
            }
        }))
        .unwrap();

        let target = determine_target_dir(None, &manifest, &BTreeMap::new()).unwrap();
        assert_eq!(target, PathBuf::from("my-agent"));
    }

    #[test]
    fn resolve_matching_plan_item_uses_semver_ordering_for_wildcards_and_ranges() {
        let older = crate::semver::types::ResolvedPackage {
            kind: PackageKind::Tool,
            name: "@zack/echo".to_string(),
            version: "0.2.0".to_string(),
            integrity: "sha256-older".to_string(),
        };
        let newer = crate::semver::types::ResolvedPackage {
            kind: PackageKind::Tool,
            name: "@zack/echo".to_string(),
            version: "0.10.0".to_string(),
            integrity: "sha256-newer".to_string(),
        };
        let packages = BTreeMap::from([("@zack/echo".to_string(), vec![&older, &newer])]);

        let wildcard = resolve_matching_plan_item(&packages, "@zack/echo", "*")
            .unwrap()
            .expect("wildcard match");
        assert_eq!(wildcard.version, "0.10.0");

        let ranged = resolve_matching_plan_item(&packages, "@zack/echo", ">=0.1.0, <1.0.0")
            .unwrap()
            .expect("range match");
        assert_eq!(ranged.version, "0.10.0");
    }

    #[test]
    fn rejects_non_empty_target_directory() {
        let root = temp_dir("non-empty");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("README.md"), "existing").unwrap();

        let err = ensure_safe_target_dir(&root).unwrap_err();
        assert!(format!("{err:#}").contains("already exists and is not empty"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cleanup_failed_target_dir_removes_generated_contents() {
        let created = temp_dir("cleanup-created");
        std::fs::create_dir_all(created.join("agents")).unwrap();
        std::fs::write(created.join("agent.json"), "{}").unwrap();
        std::fs::write(created.join("agents/reviewer.agent.json"), "{}").unwrap();
        cleanup_failed_target_dir(&created, false);
        assert!(!created.exists());

        let existing = temp_dir("cleanup-existing");
        std::fs::create_dir_all(existing.join("nested")).unwrap();
        std::fs::write(existing.join("generated.txt"), "x").unwrap();
        std::fs::write(existing.join("nested/child.txt"), "x").unwrap();
        cleanup_failed_target_dir(&existing, true);
        assert!(existing.exists());
        assert!(std::fs::read_dir(&existing).unwrap().next().is_none());

        let _ = std::fs::remove_dir_all(existing);
    }

    #[test]
    fn validate_scaffold_agent_manifest_locations_rejects_non_agents_paths() {
        let root = temp_dir("bad-agent-location");
        std::fs::create_dir_all(root.join("services")).unwrap();
        std::fs::write(root.join("services/reviewer.agent.json"), "{}").unwrap();

        let err = validate_scaffold_agent_manifest_locations(&root).unwrap_err();
        assert!(
            format!("{err:#}").contains("must live under agents/"),
            "{err:#}"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn rejects_non_template_package_resolution() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let base_url = format!("http://127.0.0.1:{}", addr.port());
        let state = Arc::new(TestState {
            base_url: base_url.clone(),
            template_tar: Vec::new(),
            template_sha: "0".repeat(64),
            tool_tar: Vec::new(),
            tool_sha: "1".repeat(64),
            agent_tar: Vec::new(),
            agent_sha: "2".repeat(64),
        });
        let app = Router::new()
            .route("/v1/tools/install/resolve", post(test_resolve))
            .route("/v1/tools/install/init", post(test_init))
            .route("/v1/tools/install/finalize", post(test_finalize))
            .route("/artifact/template", get(get_template))
            .route("/artifact/tool", get(get_tool))
            .route("/artifact/agent", get(get_agent))
            .with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let root = temp_dir("reject-non-template");
        let args = NewArgs {
            template_ref: "@zack/not-a-template@0.1.0".to_string(),
            target_dir: Some(root.join("generated")),
            vars: Vec::new(),
            quiet: true,
            token: None,
        };

        let err = args.run(base_url).await.unwrap_err();
        assert!(
            format!("{err:#}").contains("expected a single template package"),
            "{err:#}"
        );

        server.abort();
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn generates_workspace_from_published_template() {
        let root = temp_dir("integration");
        let template_tar = build_tarball(
            &[
                (
                    "agent.json",
                    serde_json::to_string_pretty(&json!({
                        "kind":"template",
                        "name":"research-template",
                        "version":"0.1.0",
                        "description":"A template.",
                        "template":{
                            "display_name":"Research Template",
                            "use_case":"research",
                            "execution_surfaces":["multi-agent-workspace"],
                            "stack":["python"],
                            "files_root":"template",
                            "variables":[{"name":"project_name","required":true,"default":"my-workspace"}],
                            "dependencies":{
                                "tools":[{"name":"@zack/echo","version":"0.1.0"}],
                                "agents":[{"name":"@zack/support-agent","version":"0.1.0"}]
                            },
                            "entrypoints":[{"label":"Run","command":"python main.py"}]
                        }
                    }))
                    .unwrap(),
                ),
                ("template/README.md", "# {{ project_name }}\n".to_string()),
                (
                    "template/pre_generate.sh",
                    "#!/bin/sh\ntouch sentinel-from-template\n".to_string(),
                ),
                (
                    "template/agents/reviewer.agent.json",
                    serde_json::to_string_pretty(&json!({
                        "kind":"agent",
                        "name":"reviewer",
                        "version":"0.1.0",
                        "description":"Reviewer",
                        "tools":[
                            {"name":"@zack/echo","version":"0.1.0"},
                            {"name":"@zack/summarize","version":"0.1.0"}
                        ],
                        "skills":[],
                        "knowledge":[],
                        "memory":[],
                        "profiles":[]
                    }))
                    .unwrap(),
                ),
            ],
        );
        let tool_tar = build_tarball(&[(
            "agent.json",
            serde_json::to_string_pretty(&json!({
                "kind":"tool",
                "name":"echo",
                "version":"0.1.0",
                "description":"Echo",
                "entrypoint":{"command":"python","args":["echo.py"]},
                "runtime":{"type":"python","version":"3.12"},
                "inputs":{},
                "outputs":{},
                "files":["echo.py"]
            }))
            .unwrap(),
        )]);
        let agent_tar = build_tarball(&[(
            "agent.json",
            serde_json::to_string_pretty(&json!({
                "kind":"agent",
                "name":"support-agent",
                "version":"0.1.0",
                "description":"Support Agent",
                "tools":[{"name":"@zack/echo","version":"0.1.0"}],
                "skills":[],
                "knowledge":[],
                "memory":[],
                "profiles":[]
            }))
            .unwrap(),
        )]);
        let initial_state = TestState {
            base_url: String::new(),
            template_sha: sha_hex(&template_tar),
            template_tar,
            tool_sha: sha_hex(&tool_tar),
            tool_tar,
            agent_sha: sha_hex(&agent_tar),
            agent_tar,
        };

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let base_url = format!("http://127.0.0.1:{}", addr.port());
        let state = Arc::new(TestState {
            base_url: base_url.clone(),
            ..initial_state
        });
        let app = Router::new()
            .route("/v1/tools/install/resolve", post(test_resolve))
            .route("/v1/tools/install/init", post(test_init))
            .route("/v1/tools/install/finalize", post(test_finalize))
            .route("/artifact/template", get(get_template))
            .route("/artifact/tool", get(get_tool))
            .route("/artifact/agent", get(get_agent))
            .with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let target = root.join("generated");
        let args = NewArgs {
            template_ref: "@zack/research-template@0.1.0".to_string(),
            target_dir: Some(target.clone()),
            vars: Vec::new(),
            quiet: true,
            token: None,
        };

        args.run(base_url).await.unwrap();

        let readme = std::fs::read_to_string(target.join("README.md")).unwrap();
        assert!(readme.contains("generated"));
        assert!(target.join("agent.lock").exists());
        assert!(target.join("agentpm.workspace.json").exists());
        assert!(target.join(".agentpm/template.json").exists());
        assert!(!target.join("sentinel-from-template").exists());

        let workspace: Value = serde_json::from_str(
            &std::fs::read_to_string(target.join("agentpm.workspace.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(workspace["manifests"][0], "agent.json");
        assert_eq!(workspace["manifests"][1], "agents/reviewer.agent.json");

        let lock: Value =
            serde_json::from_str(&std::fs::read_to_string(target.join("agent.lock")).unwrap())
                .unwrap();
        assert!(lock["packages"].get("tool:@zack/echo@0.1.0").is_some());
        assert!(lock["packages"].get("tool:@zack/summarize@0.1.0").is_some());
        assert!(
            lock["roots"]
                .get("agent:@zack/support-agent@0.1.0")
                .is_some()
        );
        assert!(lock["roots"].get("local:agent").is_some());
        assert!(
            lock["roots"]
                .get("local:agent:agents/reviewer.agent.json")
                .is_some()
        );
        assert_eq!(
            lock["roots"]["local:agent:agents/reviewer.agent.json"]["tools"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert!(
            lock["packages"]
                .get("template:@zack/research-template@0.1.0")
                .is_none()
        );
        assert!(
            target
                .join(".agentpm/tools/zack/summarize/0.1.0/agent.json")
                .exists()
        );

        let template_meta: Value = serde_json::from_str(
            &std::fs::read_to_string(target.join(".agentpm/template.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(template_meta["source"], "registry");
        assert_eq!(template_meta["kind"], "template");
        assert_eq!(template_meta["name"], "research-template");
        assert_eq!(template_meta["version"], "0.1.0");
        assert_eq!(template_meta["variables"]["project_name"], "generated");

        server.abort();
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn failed_generation_cleans_up_target_directory() {
        let root = temp_dir("failed-generation");
        let template_tar = build_tarball(
            &[
                (
                    "agent.json",
                    serde_json::to_string_pretty(&json!({
                        "kind":"template",
                        "name":"broken-template",
                        "version":"0.1.0",
                        "description":"Broken template.",
                        "template":{
                            "display_name":"Broken Template",
                            "use_case":"research",
                            "execution_surfaces":["multi-agent-workspace"],
                            "stack":["python"],
                            "files_root":"template",
                            "variables":[{"name":"project_name","required":true,"default":"broken-workspace"}],
                            "dependencies":{"tools":[],"agents":[]},
                            "entrypoints":[]
                        }
                    }))
                    .unwrap(),
                ),
                (
                    "template/agents/reviewer.agent.json",
                    serde_json::to_string_pretty(&json!({
                        "kind":"agent",
                        "name":"reviewer",
                        "version":"0.1.0",
                        "tools":[]
                    }))
                    .unwrap(),
                ),
            ],
        );
        let state = Arc::new(TestState {
            base_url: String::new(),
            template_sha: sha_hex(&template_tar),
            template_tar,
            tool_sha: "1".repeat(64),
            tool_tar: Vec::new(),
            agent_sha: "2".repeat(64),
            agent_tar: Vec::new(),
        });

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let base_url = format!("http://127.0.0.1:{}", addr.port());
        let state = Arc::new(TestState {
            base_url: base_url.clone(),
            ..(*state).clone()
        });
        let app = Router::new()
            .route("/v1/tools/install/resolve", post(test_resolve))
            .route("/v1/tools/install/init", post(test_init))
            .route("/v1/tools/install/finalize", post(test_finalize))
            .route("/artifact/template", get(get_template))
            .route("/artifact/tool", get(get_tool))
            .route("/artifact/agent", get(get_agent))
            .with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let target = root.join("generated");
        let args = NewArgs {
            template_ref: "@zack/broken-template@0.1.0".to_string(),
            target_dir: Some(target.clone()),
            vars: Vec::new(),
            quiet: true,
            token: None,
        };

        let err = args.run(base_url).await.unwrap_err();
        let err_text = format!("{err:#}");
        assert!(
            err_text.contains("generated manifest agents/reviewer.agent.json"),
            "{err_text}"
        );
        assert!(!target.exists() || std::fs::read_dir(&target).unwrap().next().is_none());

        server.abort();
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn generation_rejects_local_agent_manifests_outside_agents_directory() {
        let root = temp_dir("bad-agent-convention");
        let template_tar = build_tarball(
            &[
                (
                    "agent.json",
                    serde_json::to_string_pretty(&json!({
                        "kind":"template",
                        "name":"bad-layout-template",
                        "version":"0.1.0",
                        "description":"Bad layout template.",
                        "template":{
                            "display_name":"Bad Layout Template",
                            "use_case":"research",
                            "execution_surfaces":["multi-agent-workspace"],
                            "stack":["python"],
                            "files_root":"template",
                            "variables":[{"name":"project_name","required":true,"default":"bad-layout"}],
                            "dependencies":{"tools":[],"agents":[]},
                            "entrypoints":[]
                        }
                    }))
                    .unwrap(),
                ),
                (
                    "template/services/reviewer.agent.json",
                    serde_json::to_string_pretty(&json!({
                        "kind":"agent",
                        "name":"reviewer",
                        "version":"0.1.0",
                        "description":"Reviewer"
                    }))
                    .unwrap(),
                ),
            ],
        );
        let initial_state = TestState {
            base_url: String::new(),
            template_sha: sha_hex(&template_tar),
            template_tar,
            tool_sha: "1".repeat(64),
            tool_tar: Vec::new(),
            agent_sha: "2".repeat(64),
            agent_tar: Vec::new(),
        };

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let base_url = format!("http://127.0.0.1:{}", addr.port());
        let state = Arc::new(TestState {
            base_url: base_url.clone(),
            ..initial_state
        });
        let app = Router::new()
            .route("/v1/tools/install/resolve", post(test_resolve))
            .route("/v1/tools/install/init", post(test_init))
            .route("/v1/tools/install/finalize", post(test_finalize))
            .route("/artifact/template", get(get_template))
            .route("/artifact/tool", get(get_tool))
            .route("/artifact/agent", get(get_agent))
            .with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let target = root.join("generated");
        let args = NewArgs {
            template_ref: "@zack/bad-layout-template@0.1.0".to_string(),
            target_dir: Some(target.clone()),
            vars: Vec::new(),
            quiet: true,
            token: None,
        };

        let err = args.run(base_url).await.unwrap_err();
        assert!(
            format!("{err:#}").contains("must live under agents/"),
            "{err:#}"
        );
        assert!(!target.exists() || std::fs::read_dir(&target).unwrap().next().is_none());

        server.abort();
        let _ = std::fs::remove_dir_all(root);
    }

    #[derive(Clone)]
    struct TestState {
        base_url: String,
        template_tar: Vec<u8>,
        template_sha: String,
        tool_tar: Vec<u8>,
        tool_sha: String,
        agent_tar: Vec<u8>,
        agent_sha: String,
    }

    async fn test_resolve(
        State(state): State<Arc<TestState>>,
        body: String,
    ) -> Result<Response<Body>, StatusCode> {
        let req: Value = serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
        let mut items = Vec::new();
        let empty = Vec::new();
        for item in req["items"].as_array().unwrap_or(&empty) {
            let kind = item["kind"].as_str().unwrap();
            let name = item["name"].as_str().unwrap();
            match kind {
                "template" if name == "@zack/not-a-template" => items.push(json!({
                    "kind":"tool",
                    "name":name,
                    "version":"0.1.0",
                    "integrity":state.tool_sha
                })),
                "template" => items.push(json!({
                    "kind":"template",
                    "name":name,
                    "version":"0.1.0",
                    "integrity":state.template_sha
                })),
                "tool" => items.push(json!({
                    "kind":"tool",
                    "name":name,
                    "version":"0.1.0",
                    "integrity":state.tool_sha
                })),
                "agent" => {
                    items.push(json!({
                        "kind":"agent",
                        "name":name,
                        "version":"0.1.0",
                        "integrity":state.agent_sha
                    }));
                    items.push(json!({
                        "kind":"tool",
                        "name":"@zack/echo",
                        "version":"0.1.0",
                        "integrity":state.tool_sha
                    }));
                }
                _ => return Err(StatusCode::BAD_REQUEST),
            }
        }
        let response = json!({ "items": items });
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(response.to_string()))
            .unwrap())
    }

    async fn test_init(
        State(state): State<Arc<TestState>>,
        body: String,
    ) -> Result<Response<Body>, StatusCode> {
        let req: Value = serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
        let mut artifacts = Vec::new();
        let empty = Vec::new();
        for item in req["items"].as_array().unwrap_or(&empty) {
            let kind = item["kind"].as_str().unwrap();
            let url = match kind {
                "template" => format!("{}/artifact/template", state.base_url),
                "tool" => format!("{}/artifact/tool", state.base_url),
                "agent" => format!("{}/artifact/agent", state.base_url),
                _ => return Err(StatusCode::BAD_REQUEST),
            };
            let integrity = match kind {
                "template" => state.template_sha.as_str(),
                "tool" => state.tool_sha.as_str(),
                _ => state.agent_sha.as_str(),
            };
            artifacts.push(json!({
                "kind":kind,
                "name":item["name"],
                "version":"0.1.0",
                "integrity":integrity,
                "presigned_url":url,
                "size":12,
                "content_type":"application/gzip"
            }));
        }
        let response = json!({
            "session_id":"session-1",
            "expires_at":"2026-06-01T00:00:00Z",
            "artifacts":artifacts
        });
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(response.to_string()))
            .unwrap())
    }

    async fn test_finalize() -> StatusCode {
        StatusCode::NO_CONTENT
    }

    async fn get_template(State(state): State<Arc<TestState>>) -> Response<Body> {
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(state.template_tar.clone()))
            .unwrap()
    }
    async fn get_tool(State(state): State<Arc<TestState>>) -> Response<Body> {
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(state.tool_tar.clone()))
            .unwrap()
    }
    async fn get_agent(State(state): State<Arc<TestState>>) -> Response<Body> {
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(state.agent_tar.clone()))
            .unwrap()
    }

    fn build_tarball(files: &[(&str, String)]) -> Vec<u8> {
        let mut tar_bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_bytes);
            for (path, contents) in files {
                let bytes = contents.as_bytes();
                let mut header = tar::Header::new_gnu();
                header.set_size(bytes.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder.append_data(&mut header, *path, bytes).unwrap();
            }
            builder.finish().unwrap();
        }

        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        use std::io::Write;
        encoder.write_all(&tar_bytes).unwrap();
        encoder.finish().unwrap()
    }

    fn sha_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hex::encode(hasher.finalize())
    }
}
