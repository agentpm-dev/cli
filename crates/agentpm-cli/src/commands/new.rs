use crate::io::download::{
    InstallRoots, download_and_extract_all, download_to, ensure_sha256, extract_tar_gz,
};
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
use std::io::{self, IsTerminal, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::task;
use walkdir::WalkDir;

#[derive(clap::Parser, Debug, Clone)]
pub struct NewArgs {
    /// Template source: @namespace/name[@version], a local template directory, or a local agent.json path
    pub template_ref: String,

    /// Optional target directory for the generated project
    pub target_dir: Option<PathBuf>,

    /// Generation-time variable overrides (repeatable, KEY=VALUE)
    #[arg(long = "var", value_name = "KEY=VALUE")]
    pub vars: Vec<String>,

    /// Reduce output
    #[clap(long)]
    pub quiet: bool,

    /// Personal Access Token for registry-backed templates (overrides env/file)
    #[arg(long, value_name = "PAT", env = "AGENTPM_TOKEN")]
    pub token: Option<String>,
}

struct LoadedTemplate {
    manifest: TemplateManifest,
    extract_root: PathBuf,
    origin_source: String,
    origin_integrity: Option<String>,
    origin_path: Option<String>,
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
        let interactive = std::io::stdin().is_terminal() && std::io::stderr().is_terminal();

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

        let loaded_template =
            load_template_source(&self.template_ref, &mut client, template_work, quiet).await?;
        let downloaded_manifest = loaded_template.manifest;
        let template_extract = loaded_template.extract_root;

        let target_dir =
            determine_target_dir(self.target_dir.clone(), &downloaded_manifest, &overrides)?;
        let mut resolved_vars =
            resolve_template_variables(&downloaded_manifest, &overrides, &target_dir, interactive)?;
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
                let skills_dir = target_dir.join(".agentpm/skills");
                let knowledge_dir = target_dir.join(".agentpm/knowledge");
                let memory_dir = target_dir.join(".agentpm/memory");
                let profiles_dir = target_dir.join(".agentpm/profiles");
                fs::ensure_dirs(&[
                    &cache_dir,
                    &tools_dir,
                    &agents_dir,
                    &skills_dir,
                    &knowledge_dir,
                    &memory_dir,
                    &profiles_dir,
                ])?;
                download_and_extract_all(
                    &init,
                    &cache_dir,
                    InstallRoots {
                        tools_dir: &tools_dir,
                        agents_dir: &agents_dir,
                        skills_dir: &skills_dir,
                        knowledge_dir: &knowledge_dir,
                        memory_dir: &memory_dir,
                        profiles_dir: &profiles_dir,
                    },
                    false,
                    quiet,
                )
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
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    profiles: Vec::new(),
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
                    source: loaded_template.origin_source.clone(),
                    kind: "template".to_string(),
                    name: downloaded_manifest.name.clone(),
                    version: downloaded_manifest.version.clone(),
                    integrity: loaded_template.origin_integrity.clone(),
                    path: loaded_template.origin_path.clone(),
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

async fn load_template_source(
    template_ref: &str,
    client: &mut AgentPmClient,
    template_work: &Path,
    quiet: bool,
) -> Result<LoadedTemplate> {
    if is_local_template_ref(template_ref) {
        load_local_template_source(template_ref, quiet)
    } else {
        load_registry_template_source(template_ref, client, template_work, quiet).await
    }
}

fn is_local_template_ref(spec: &str) -> bool {
    let trimmed = spec.trim();
    !trimmed.starts_with('@')
}

async fn load_registry_template_source(
    template_ref: &str,
    client: &mut AgentPmClient,
    template_work: &Path,
    quiet: bool,
) -> Result<LoadedTemplate> {
    let mut step = crate::ui::Step::new("Resolving template", quiet);
    let template_req = parse_template_ref(template_ref)?;
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

    Ok(LoadedTemplate {
        manifest: downloaded_manifest,
        extract_root: template_extract,
        origin_source: "registry".to_string(),
        origin_integrity: Some(template_artifact.integrity.clone()),
        origin_path: None,
    })
}

fn load_local_template_source(template_ref: &str, quiet: bool) -> Result<LoadedTemplate> {
    let mut step = crate::ui::Step::new("Loading local template", quiet);
    let input_path = PathBuf::from(template_ref);
    let package_root = if input_path.file_name().and_then(|s| s.to_str()) == Some("agent.json") {
        input_path
            .parent()
            .ok_or_else(|| {
                anyhow!(
                    "local template path {} has no parent directory",
                    input_path.display()
                )
            })?
            .to_path_buf()
    } else {
        input_path
    };
    let package_root = package_root
        .canonicalize()
        .with_context(|| format!("resolving local template path {}", package_root.display()))?;
    let manifest_path = package_root.join("agent.json");
    if !manifest_path.exists() {
        return Err(anyhow!(
            "local template path {} is missing agent.json",
            package_root.display()
        ));
    }
    let (mut manifest_value, _) = load_manifest_value(&manifest_path)?;
    let schema_source = local_validation_schema_source();
    let (ok, issues) = validate_manifest_value(
        &schema_source,
        &manifest_path.to_string_lossy(),
        &mut manifest_value,
        false,
    )?;
    if !ok {
        return Err(anyhow!(
            "local template manifest {} is invalid: {issues:#?}",
            manifest_path.display()
        ));
    }
    let manifest_kind = manifest_value.get("kind").and_then(Value::as_str);
    if manifest_kind != Some("template") {
        return Err(anyhow!(
            "local template {} must have kind=\"template\"",
            manifest_path.display()
        ));
    }
    let manifest = parse_template_manifest(&manifest_value)?;
    step.ok("");

    Ok(LoadedTemplate {
        manifest,
        extract_root: package_root.clone(),
        origin_source: "local".to_string(),
        origin_integrity: None,
        origin_path: Some(package_root.display().to_string()),
    })
}

fn local_validation_schema_source() -> String {
    let repo_schema =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/agentpm.manifest.schema.json");
    if repo_schema.exists() {
        repo_schema.to_string_lossy().into_owned()
    } else {
        resolve_schema_source(None)
    }
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
    resolve_template_variables_with_prompter(
        manifest,
        overrides,
        target_dir,
        interactive,
        prompt_for_template_variable,
    )
}

fn resolve_template_variables_with_prompter<F>(
    manifest: &TemplateManifest,
    overrides: &BTreeMap<String, String>,
    target_dir: &Path,
    interactive: bool,
    mut prompter: F,
) -> Result<BTreeMap<String, String>>
where
    F: FnMut(&crate::manifest::TemplateVariable, &str) -> Result<String>,
{
    let target_name = target_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&manifest.name);
    let default_project_name = sanitized_project_name(target_name);

    let mut resolved = BTreeMap::new();
    for variable in &manifest.template.variables {
        let value = overrides
            .get(&variable.name)
            .cloned()
            .or_else(|| {
                if variable.name == "project_name" {
                    Some(default_project_name.clone())
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
                if interactive {
                    let prompt_project_name = resolved
                        .get("project_name")
                        .map(String::as_str)
                        .unwrap_or(&default_project_name);
                    resolved.insert(
                        variable.name.clone(),
                        prompter(variable, prompt_project_name)?,
                    );
                } else {
                    return Err(anyhow!(
                        "required template variable {} has no value; pass --var {}=...",
                        variable.name,
                        variable.name
                    ));
                }
            }
            None => {}
        }
    }

    if !resolved.contains_key("project_name") {
        resolved.insert("project_name".to_string(), default_project_name);
    }

    Ok(resolved)
}

fn prompt_for_template_variable(
    variable: &crate::manifest::TemplateVariable,
    project_name: &str,
) -> Result<String> {
    let mut stderr = io::stderr().lock();
    write!(
        stderr,
        "Enter value for template variable {}",
        variable.name
    )?;
    if variable.name != "project_name" {
        write!(stderr, " (project_name: {project_name})")?;
    }
    if let Some(description) = &variable.description {
        write!(stderr, " - {description}")?;
    }
    write!(stderr, ": ")?;
    stderr.flush()?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read interactive template variable input")?;
    validate_interactive_template_value(&variable.name, &input)
}

fn validate_interactive_template_value(variable_name: &str, raw_input: &str) -> Result<String> {
    let value = raw_input.trim().to_string();
    if value.is_empty() {
        return Err(anyhow!(
            "no value entered for required template variable {}",
            variable_name
        ));
    }
    Ok(value)
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
    for skill in &manifest.template.dependencies.skills {
        let (name, range) = package_ref_parts(skill)?;
        items.push(agentpm_sdk::models::install::PackageRequirement {
            kind: agentpm_sdk::models::install::PackageKind::Skill,
            name,
            range,
        });
    }
    for knowledge in &manifest.template.dependencies.knowledge {
        let (name, range) = package_ref_parts(knowledge)?;
        items.push(agentpm_sdk::models::install::PackageRequirement {
            kind: agentpm_sdk::models::install::PackageKind::Knowledge,
            name,
            range,
        });
    }
    for memory in &manifest.template.dependencies.memory {
        let (name, range) = package_ref_parts(memory)?;
        items.push(agentpm_sdk::models::install::PackageRequirement {
            kind: agentpm_sdk::models::install::PackageKind::Memory,
            name,
            range,
        });
    }
    for profile in &manifest.template.dependencies.profiles {
        let (name, range) = package_ref_parts(profile)?;
        items.push(agentpm_sdk::models::install::PackageRequirement {
            kind: agentpm_sdk::models::install::PackageKind::Profile,
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
                    PackageKind::Skill => agentpm_sdk::models::install::PackageKind::Skill,
                    PackageKind::Knowledge => agentpm_sdk::models::install::PackageKind::Knowledge,
                    PackageKind::Memory => agentpm_sdk::models::install::PackageKind::Memory,
                    PackageKind::Profile => agentpm_sdk::models::install::PackageKind::Profile,
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
    let skill_refs = resolved_skill_manifest_refs(template_manifest, plan)?;
    let knowledge_refs = resolved_knowledge_manifest_refs(template_manifest, plan)?;
    let memory_refs = resolved_memory_manifest_refs(template_manifest, plan)?;
    let profile_refs = resolved_profile_manifest_refs(template_manifest, plan)?;
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
        "skills": skill_refs,
        "knowledge": knowledge_refs,
        "memory": memory_refs,
        "profiles": profile_refs
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

fn resolved_skill_manifest_refs(
    template_manifest: &TemplateManifest,
    plan: &ResolvePlan,
) -> Result<Vec<Value>> {
    let direct_skill_refs = template_manifest
        .template
        .dependencies
        .skills
        .iter()
        .map(package_ref_parts)
        .collect::<Result<Vec<_>>>()?;
    let packages = plan_packages_by_name(plan, PackageKind::Skill);
    let mut refs = Vec::new();
    for (name, range) in direct_skill_refs {
        let resolved = resolve_matching_plan_item(&packages, &name, &range)?.ok_or_else(|| {
            anyhow!(
                "resolved skill dependency {}@{} missing from plan",
                name,
                range
            )
        })?;
        refs.push(json!({
            "name": resolved.name,
            "version": resolved.version,
        }));
    }
    Ok(refs)
}

fn resolved_knowledge_manifest_refs(
    template_manifest: &TemplateManifest,
    plan: &ResolvePlan,
) -> Result<Vec<Value>> {
    let direct_knowledge_refs = template_manifest
        .template
        .dependencies
        .knowledge
        .iter()
        .map(package_ref_parts)
        .collect::<Result<Vec<_>>>()?;
    let packages = plan_packages_by_name(plan, PackageKind::Knowledge);
    let mut refs = Vec::new();
    for (name, range) in direct_knowledge_refs {
        let resolved = resolve_matching_plan_item(&packages, &name, &range)?.ok_or_else(|| {
            anyhow!(
                "resolved knowledge dependency {}@{} missing from plan",
                name,
                range
            )
        })?;
        refs.push(json!({
            "name": resolved.name,
            "version": resolved.version,
        }));
    }
    Ok(refs)
}

fn resolved_memory_manifest_refs(
    template_manifest: &TemplateManifest,
    plan: &ResolvePlan,
) -> Result<Vec<Value>> {
    let direct_memory_refs = template_manifest
        .template
        .dependencies
        .memory
        .iter()
        .map(package_ref_parts)
        .collect::<Result<Vec<_>>>()?;
    let packages = plan_packages_by_name(plan, PackageKind::Memory);
    let mut refs = Vec::new();
    for (name, range) in direct_memory_refs {
        let resolved = resolve_matching_plan_item(&packages, &name, &range)?.ok_or_else(|| {
            anyhow!(
                "resolved memory dependency {}@{} missing from plan",
                name,
                range
            )
        })?;
        refs.push(json!({
            "name": resolved.name,
            "version": resolved.version,
        }));
    }
    Ok(refs)
}

fn resolved_profile_manifest_refs(
    template_manifest: &TemplateManifest,
    plan: &ResolvePlan,
) -> Result<Vec<Value>> {
    let direct_profile_refs = template_manifest
        .template
        .dependencies
        .profiles
        .iter()
        .map(package_ref_parts)
        .collect::<Result<Vec<_>>>()?;
    let packages = plan_packages_by_name(plan, PackageKind::Profile);
    let mut refs = Vec::new();
    for (name, range) in direct_profile_refs {
        let resolved = resolve_matching_plan_item(&packages, &name, &range)?.ok_or_else(|| {
            anyhow!(
                "resolved profile dependency {}@{} missing from plan",
                name,
                range
            )
        })?;
        refs.push(json!({
            "name": resolved.name,
            "version": resolved.version,
        }));
    }
    Ok(refs)
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
    fn build_dependency_request_includes_template_skill_dependencies() {
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
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": [],
                    "skills": ["@zack/triage-skill@^0.2"]
                },
                "entrypoints": []
            }
        }))
        .unwrap();

        let request = build_dependency_request(&manifest, Path::new("."), &[]).unwrap();

        assert_eq!(request.items.len(), 1);
        assert_eq!(
            request.items[0].kind,
            agentpm_sdk::models::install::PackageKind::Skill
        );
        assert_eq!(request.items[0].name, "@zack/triage-skill");
        assert_eq!(request.items[0].range, "^0.2");
    }

    #[test]
    fn build_dependency_request_includes_template_knowledge_dependencies() {
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
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": [],
                    "knowledge": ["@zack/python-docs@0.1.0"]
                },
                "entrypoints": []
            }
        }))
        .unwrap();

        let request = build_dependency_request(&manifest, Path::new("."), &[]).unwrap();

        assert_eq!(request.items.len(), 1);
        assert_eq!(
            request.items[0].kind,
            agentpm_sdk::models::install::PackageKind::Knowledge
        );
        assert_eq!(request.items[0].name, "@zack/python-docs");
        assert_eq!(request.items[0].range, "0.1.0");
    }

    #[test]
    fn build_dependency_request_includes_template_memory_dependencies() {
        let manifest = parse_template_manifest(&json!({
            "kind": "template",
            "name": "memory-assistant",
            "version": "0.1.0",
            "description": "Template",
            "template": {
                "display_name": "Memory Assistant",
                "use_case": "support",
                "execution_surfaces": ["python-sdk"],
                "stack": ["python"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": [],
                    "memory": ["@zack/session-memory@0.1.0"]
                },
                "entrypoints": []
            }
        }))
        .unwrap();

        let request = build_dependency_request(&manifest, Path::new("."), &[]).unwrap();

        assert_eq!(request.items.len(), 1);
        assert_eq!(
            request.items[0].kind,
            agentpm_sdk::models::install::PackageKind::Memory
        );
        assert_eq!(request.items[0].name, "@zack/session-memory");
        assert_eq!(request.items[0].range, "0.1.0");
    }

    #[test]
    fn build_dependency_request_includes_template_profile_dependencies() {
        let manifest = parse_template_manifest(&json!({
            "kind": "template",
            "name": "support-workspace",
            "version": "0.1.0",
            "description": "Template",
            "template": {
                "display_name": "Support Workspace",
                "use_case": "support",
                "execution_surfaces": ["python-sdk"],
                "stack": ["python"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": [],
                    "profiles": ["@zack/support-style@^0.2"]
                },
                "entrypoints": []
            }
        }))
        .unwrap();

        let request = build_dependency_request(&manifest, Path::new("."), &[]).unwrap();

        assert_eq!(request.items.len(), 1);
        assert_eq!(
            request.items[0].kind,
            agentpm_sdk::models::install::PackageKind::Profile
        );
        assert_eq!(request.items[0].name, "@zack/support-style");
        assert_eq!(request.items[0].range, "^0.2");
    }

    #[test]
    fn prompts_interactively_for_required_variable_without_default() {
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
                "variables": [
                    {
                        "name":"project_name",
                        "description":"Generated project name",
                        "required":true,
                        "default":"my-agent"
                    },
                    {
                        "name":"topic",
                        "description":"Research topic",
                        "required":true
                    }
                ],
                "dependencies": {"tools":[],"agents":[]},
                "entrypoints": []
            }
        }))
        .unwrap();

        let mut prompted = Vec::new();
        let resolved = resolve_template_variables_with_prompter(
            &manifest,
            &BTreeMap::new(),
            Path::new("my-agent"),
            true,
            |variable, project_name| {
                prompted.push((variable.name.clone(), project_name.to_string()));
                Ok("AgentPM".to_string())
            },
        )
        .unwrap();

        assert_eq!(resolved["project_name"], "my-agent");
        assert_eq!(resolved["topic"], "AgentPM");
        assert_eq!(
            prompted,
            vec![("topic".to_string(), "my-agent".to_string())]
        );
    }

    #[test]
    fn interactive_prompt_is_bypassed_when_var_override_is_present() {
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
                "variables": [
                    {
                        "name":"project_name",
                        "description":"Generated project name",
                        "required":true,
                        "default":"my-agent"
                    },
                    {
                        "name":"topic",
                        "description":"Research topic",
                        "required":true
                    }
                ],
                "dependencies": {"tools":[],"agents":[]},
                "entrypoints": []
            }
        }))
        .unwrap();

        let overrides = BTreeMap::from([("topic".to_string(), "OpenAI".to_string())]);
        let mut prompt_calls = 0usize;
        let resolved = resolve_template_variables_with_prompter(
            &manifest,
            &overrides,
            Path::new("my-agent"),
            true,
            |_variable, _project_name| {
                prompt_calls += 1;
                Ok("should-not-be-used".to_string())
            },
        )
        .unwrap();

        assert_eq!(resolved["topic"], "OpenAI");
        assert_eq!(prompt_calls, 0);
    }

    #[test]
    fn interactive_prompt_uses_resolved_project_name_override_as_context() {
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
                "variables": [
                    {
                        "name":"project_name",
                        "description":"Generated project name",
                        "required":true,
                        "default":"my-agent"
                    },
                    {
                        "name":"topic",
                        "description":"Research topic",
                        "required":true
                    }
                ],
                "dependencies": {"tools":[],"agents":[]},
                "entrypoints": []
            }
        }))
        .unwrap();

        let overrides = BTreeMap::from([("project_name".to_string(), "custom-name".to_string())]);
        let mut prompted = Vec::new();
        let resolved = resolve_template_variables_with_prompter(
            &manifest,
            &overrides,
            Path::new("my-project"),
            true,
            |variable, project_name| {
                prompted.push((variable.name.clone(), project_name.to_string()));
                Ok("AgentPM".to_string())
            },
        )
        .unwrap();

        assert_eq!(resolved["project_name"], "custom-name");
        assert_eq!(resolved["topic"], "AgentPM");
        assert_eq!(
            prompted,
            vec![("topic".to_string(), "custom-name".to_string())]
        );
    }

    #[test]
    fn non_interactive_required_variable_failure_is_unchanged() {
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
                "variables": [
                    {
                        "name":"project_name",
                        "description":"Generated project name",
                        "required":true,
                        "default":"my-agent"
                    },
                    {
                        "name":"topic",
                        "description":"Research topic",
                        "required":true
                    }
                ],
                "dependencies": {"tools":[],"agents":[]},
                "entrypoints": []
            }
        }))
        .unwrap();

        let err = resolve_template_variables_with_prompter(
            &manifest,
            &BTreeMap::new(),
            Path::new("my-agent"),
            false,
            |_variable, _project_name| Ok("ignored".to_string()),
        )
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("required template variable topic has no value"),
            "{err:#}"
        );
    }

    #[test]
    fn interactive_empty_input_has_interactive_specific_error() {
        let err = validate_interactive_template_value("topic", "").unwrap_err();

        assert!(
            format!("{err:#}").contains("no value entered for required template variable topic"),
            "{err:#}"
        );
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
            knowledge_tar: Vec::new(),
            knowledge_sha: "3".repeat(64),
            memory_tar: Vec::new(),
            memory_sha: "4".repeat(64),
            profile_tar: Vec::new(),
            profile_sha: "5".repeat(64),
        });
        let app = Router::new()
            .route("/v1/tools/install/resolve", post(test_resolve))
            .route("/v1/tools/install/init", post(test_init))
            .route("/v1/tools/install/finalize", post(test_finalize))
            .route("/artifact/template", get(get_template))
            .route("/artifact/tool", get(get_tool))
            .route("/artifact/agent", get(get_agent))
            .route("/artifact/knowledge", get(get_knowledge))
            .route("/artifact/memory", get(get_memory))
            .route("/artifact/profile", get(get_profile))
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
                                "agents":[{"name":"@zack/support-agent","version":"0.1.0"}],
                                "knowledge":[{"name":"@zack/python-docs","version":"0.1.0"}],
                                "memory":[{"name":"@zack/session-memory","version":"0.1.0"}],
                                "profiles":[{"name":"@zack/support-style","version":"0.1.0"}]
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
                        "profiles":[{"name":"@zack/escalation-style","version":"0.1.0"}]
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
        let knowledge_tar = build_tarball(&[
            (
                "agent.json",
                serde_json::to_string_pretty(&json!({
                    "kind":"knowledge",
                    "name":"python-docs",
                    "version":"0.1.0",
                    "description":"Knowledge",
                    "knowledge":{
                        "mode":"context",
                        "documents":[{"path":"knowledge/docs/context.md"}]
                    }
                }))
                .unwrap(),
            ),
            ("knowledge/docs/context.md", "# Python Docs\n".to_string()),
        ]);
        let template_sha = sha_hex(&template_tar);
        let memory_tar = build_tarball(&[
            (
                "agent.json",
                serde_json::to_string_pretty(&json!({
                    "kind":"memory",
                    "name":"session-memory",
                    "version":"0.1.0",
                    "description":"Memory",
                    "memory":{
                        "scopes":{
                            "user":{"description":"User scope."}
                        },
                        "record_types":{
                            "user_preference":{
                                "description":"Preference record.",
                                "schema":"schemas/user-preference.schema.json",
                                "version":"1.0.0"
                            }
                        },
                        "spaces":{
                            "profile":{
                                "description":"Profile space.",
                                "model":"document",
                                "scope":["user"],
                                "record_types":["user_preference"],
                                "retrieval":{"modes":["key"]}
                            }
                        }
                    }
                }))
                .unwrap(),
            ),
            (
                "schemas/user-preference.schema.json",
                serde_json::to_string_pretty(&json!({
                    "$schema":"https://json-schema.org/draft/2020-12/schema",
                    "type":"object",
                    "additionalProperties":false,
                    "properties":{"theme":{"type":"string"}}
                }))
                .unwrap(),
            ),
            (
                "memory/build.json",
                serde_json::to_string_pretty(&json!({
                    "type":"agentpm-memory-contracts",
                    "format_version":1,
                    "manifest_path":"agent.json",
                    "source_manifest_hash":"sha256:manifest",
                    "source_schemas_hash":"sha256:schemas",
                    "source_contract_inputs_hash":"sha256:inputs",
                    "contracts_index_hash":"sha256:index",
                    "contracts_hash":"sha256:contracts",
                    "contract_count":1,
                    "source_schemas":[{
                        "path":"schemas/user-preference.schema.json",
                        "sha256":"sha256:schema-file"
                    }]
                }))
                .unwrap(),
            ),
            (
                "memory/contracts/index.json",
                serde_json::to_string_pretty(&json!({
                    "type":"agentpm-memory-contract-index",
                    "format_version":1,
                    "contracts":[{
                        "space":"profile",
                        "record_type":"user_preference",
                        "schema_version":"1.0.0",
                        "path":"memory/contracts/profile.user_preference.schema.json",
                        "sha256":"sha256:contract-file"
                    }]
                }))
                .unwrap(),
            ),
            (
                "memory/contracts/profile.user_preference.schema.json",
                serde_json::to_string_pretty(&json!({
                    "$schema":"https://json-schema.org/draft/2020-12/schema",
                    "type":"object",
                    "properties":{"id":{"type":"string"}},
                    "required":["id"]
                }))
                .unwrap(),
            ),
        ]);
        let profile_tar = build_tarball(&[
            (
                "agent.json",
                serde_json::to_string_pretty(&json!({
                    "kind":"profile",
                    "name":"support-style",
                    "version":"0.1.0",
                    "description":"Support response style profile.",
                    "profile":{
                        "identity":{"role":"Support responder"},
                        "objectives":["Acknowledge the issue clearly."],
                        "communication":{
                            "tone":"calm",
                            "verbosity":"moderate"
                        }
                    },
                    "readme":"README.md"
                }))
                .unwrap(),
            ),
            (
                "README.md",
                "Keep installed placeholders literal: {{ project_name }}\n".to_string(),
            ),
        ]);
        let initial_state = TestState {
            base_url: String::new(),
            template_sha: template_sha.clone(),
            template_tar,
            tool_sha: sha_hex(&tool_tar),
            tool_tar,
            agent_sha: sha_hex(&agent_tar),
            agent_tar,
            knowledge_sha: sha_hex(&knowledge_tar),
            knowledge_tar,
            memory_sha: sha_hex(&memory_tar),
            memory_tar,
            profile_sha: sha_hex(&profile_tar),
            profile_tar,
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
            .route("/artifact/knowledge", get(get_knowledge))
            .route("/artifact/memory", get(get_memory))
            .route("/artifact/profile", get(get_profile))
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
        assert_eq!(
            workspace["package_roots"]["memory"],
            json!([]),
            "template package roots should not gain standalone memory roots: {workspace:#}"
        );
        assert_eq!(
            workspace["package_roots"]["profiles"],
            json!([]),
            "template package roots should not gain standalone profile roots: {workspace:#}"
        );

        let lock: Value =
            serde_json::from_str(&std::fs::read_to_string(target.join("agent.lock")).unwrap())
                .unwrap();
        assert!(lock["packages"].get("tool:@zack/echo@0.1.0").is_some());
        assert!(lock["packages"].get("tool:@zack/summarize@0.1.0").is_some());
        assert!(
            lock["packages"]
                .get("knowledge:@zack/python-docs@0.1.0")
                .is_some()
        );
        assert!(
            lock["packages"]
                .get("memory:@zack/session-memory@0.1.0")
                .is_some()
        );
        assert!(
            lock["packages"]
                .get("profile:@zack/support-style@0.1.0")
                .is_some()
        );
        assert!(
            lock["packages"]
                .get("profile:@zack/escalation-style@0.1.0")
                .is_some()
        );
        assert!(
            lock["roots"]
                .get("agent:@zack/support-agent@0.1.0")
                .is_some()
        );
        assert!(
            lock["roots"]
                .get("memory:@zack/session-memory@0.1.0")
                .is_none()
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
        assert_eq!(
            lock["roots"]["local:agent"]["knowledge"],
            json!(["knowledge:@zack/python-docs@0.1.0"])
        );
        assert_eq!(
            lock["roots"]["local:agent"]["memory"],
            json!(["memory:@zack/session-memory@0.1.0"])
        );
        assert_eq!(
            lock["roots"]["local:agent"]["profiles"],
            json!(["profile:@zack/support-style@0.1.0"])
        );
        assert_eq!(
            lock["roots"]["local:agent:agents/reviewer.agent.json"]["profiles"],
            json!(["profile:@zack/escalation-style@0.1.0"])
        );
        assert!(
            target
                .join(".agentpm/knowledge/zack/python-docs/0.1.0/agent.json")
                .exists()
        );
        assert!(
            target
                .join(".agentpm/memory/zack/session-memory/0.1.0/agent.json")
                .exists()
        );
        assert!(
            target
                .join(".agentpm/profiles/zack/support-style/0.1.0/agent.json")
                .exists()
        );
        assert!(
            target
                .join(".agentpm/profiles/zack/escalation-style/0.1.0/agent.json")
                .exists()
        );
        assert_eq!(
            std::fs::read_to_string(
                target.join(".agentpm/profiles/zack/support-style/0.1.0/README.md")
            )
            .unwrap(),
            "Keep installed placeholders literal: {{ project_name }}\n"
        );
        let root_manifest: Value =
            serde_json::from_str(&std::fs::read_to_string(target.join("agent.json")).unwrap())
                .unwrap();
        assert_eq!(
            root_manifest["knowledge"],
            json!([{"name":"@zack/python-docs","version":"0.1.0"}])
        );
        assert_eq!(
            root_manifest["memory"],
            json!([{"name":"@zack/session-memory","version":"0.1.0"}])
        );
        assert_eq!(
            root_manifest["profiles"],
            json!([{"name":"@zack/support-style","version":"0.1.0"}])
        );
        let reviewer_manifest: Value = serde_json::from_str(
            &std::fs::read_to_string(target.join("agents/reviewer.agent.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            reviewer_manifest["profiles"],
            json!([{"name":"@zack/escalation-style","version":"0.1.0"}])
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
        assert_eq!(template_meta["integrity"], Value::String(template_sha));
        assert!(template_meta.get("path").is_none());
        assert_eq!(template_meta["variables"]["project_name"], "generated");

        server.abort();
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn rejects_wrong_kind_template_profile_dependency() {
        let root = temp_dir("wrong-kind-profile-dep");
        let template_tar = build_tarball(&[
            (
                "agent.json",
                serde_json::to_string_pretty(&json!({
                    "kind":"template",
                    "name":"support-template",
                    "version":"0.1.0",
                    "description":"A template.",
                    "template":{
                        "display_name":"Support Template",
                        "use_case":"support",
                        "execution_surfaces":["multi-agent-workspace"],
                        "stack":["python"],
                        "files_root":"template",
                        "variables":[{"name":"project_name","required":true,"default":"generated"}],
                        "dependencies":{
                            "tools":[],
                            "agents":[],
                            "profiles":[{"name":"@zack/not-a-profile","version":"0.1.0"}]
                        },
                        "entrypoints":[]
                    }
                }))
                .unwrap(),
            ),
            ("template/README.md", "# generated\n".to_string()),
        ]);
        let memory_tar = build_tarball(&[(
            "agent.json",
            serde_json::to_string_pretty(&json!({
                "kind":"memory",
                "name":"not-a-profile",
                "version":"0.1.0",
                "description":"Actually memory.",
                "memory":{
                    "scopes":{"user":{"description":"User scope."}},
                    "record_types":{
                        "note":{
                            "description":"Note.",
                            "schema":"schemas/note.schema.json",
                            "version":"1.0.0"
                        }
                    },
                    "spaces":{
                        "profile":{
                            "description":"Profile space.",
                            "model":"document",
                            "scope":["user"],
                            "record_types":["note"],
                            "retrieval":{"modes":["key"]}
                        }
                    }
                }
            }))
            .unwrap(),
        )]);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let base_url = format!("http://127.0.0.1:{}", addr.port());
        let state = Arc::new(TestState {
            base_url: base_url.clone(),
            template_sha: sha_hex(&template_tar),
            template_tar,
            tool_tar: Vec::new(),
            tool_sha: "1".repeat(64),
            agent_tar: Vec::new(),
            agent_sha: "2".repeat(64),
            knowledge_tar: Vec::new(),
            knowledge_sha: "3".repeat(64),
            memory_sha: sha_hex(&memory_tar),
            memory_tar,
            profile_tar: Vec::new(),
            profile_sha: "5".repeat(64),
        });
        let app = Router::new()
            .route("/v1/tools/install/resolve", post(test_resolve))
            .route("/v1/tools/install/init", post(test_init))
            .route("/v1/tools/install/finalize", post(test_finalize))
            .route("/artifact/template", get(get_template))
            .route("/artifact/tool", get(get_tool))
            .route("/artifact/agent", get(get_agent))
            .route("/artifact/knowledge", get(get_knowledge))
            .route("/artifact/memory", get(get_memory))
            .route("/artifact/profile", get(get_profile))
            .with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let target = root.join("generated");
        let args = NewArgs {
            template_ref: "@zack/support-template@0.1.0".to_string(),
            target_dir: Some(target),
            vars: Vec::new(),
            quiet: true,
            token: None,
        };

        let err = args.run(base_url).await.unwrap_err();
        assert!(
            format!("{err:#}").contains(
                "resolved profile dependency @zack/not-a-profile@0.1.0 missing from plan"
            ),
            "{err:#}"
        );

        server.abort();
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn generates_workspace_from_local_template_path() {
        let root = temp_dir("local-template");
        let template_root = root.join("my-template");
        std::fs::create_dir_all(template_root.join("template")).unwrap();
        std::fs::write(
            template_root.join("agent.json"),
            serde_json::to_string_pretty(&json!({
                "kind":"template",
                "name":"local-template",
                "version":"0.1.0",
                "description":"A local template.",
                "template":{
                    "display_name":"Local Template",
                    "use_case":"research",
                    "execution_surfaces":["python-sdk"],
                    "stack":["python"],
                    "files_root":"template",
                    "variables":[{"name":"project_name","description":"Generated project name","required":true,"default":"local-generated"}],
                    "dependencies":{
                        "tools":[{"name":"@zack/echo","version":"0.1.0"}],
                        "agents":[]
                    },
                    "entrypoints":[{"label":"Run","command":"python main.py"}]
                }
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            template_root.join("template/README.md"),
            "# {{ project_name }}\n",
        )
        .unwrap();

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
        let state = Arc::new(TestState {
            base_url: String::new(),
            template_tar: Vec::new(),
            template_sha: "0".repeat(64),
            tool_sha: sha_hex(&tool_tar),
            tool_tar,
            agent_sha: "2".repeat(64),
            agent_tar: Vec::new(),
            knowledge_sha: "3".repeat(64),
            knowledge_tar: Vec::new(),
            memory_sha: "4".repeat(64),
            memory_tar: Vec::new(),
            profile_sha: "5".repeat(64),
            profile_tar: Vec::new(),
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
            .route("/artifact/tool", get(get_tool))
            .route("/artifact/agent", get(get_agent))
            .route("/artifact/knowledge", get(get_knowledge))
            .route("/artifact/memory", get(get_memory))
            .route("/artifact/profile", get(get_profile))
            .with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let target = root.join("generated");
        let args = NewArgs {
            template_ref: template_root.display().to_string(),
            target_dir: Some(target.clone()),
            vars: Vec::new(),
            quiet: true,
            token: None,
        };

        args.run(base_url).await.unwrap();

        assert!(target.join("README.md").exists());
        assert!(target.join("agent.lock").exists());
        assert!(
            target
                .join(".agentpm/tools/zack/echo/0.1.0/agent.json")
                .exists()
        );

        let template_meta: Value = serde_json::from_str(
            &std::fs::read_to_string(target.join(".agentpm/template.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(template_meta["source"], "local");
        assert_eq!(template_meta["kind"], "template");
        assert_eq!(template_meta["name"], "local-template");
        assert_eq!(template_meta["version"], "0.1.0");
        assert_eq!(
            template_meta["path"],
            Value::String(template_root.canonicalize().unwrap().display().to_string())
        );
        assert!(template_meta.get("integrity").is_none());

        server.abort();
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn generates_workspace_from_local_template_manifest_path() {
        let root = temp_dir("local-template-manifest-path");
        let template_root = root.join("my-template");
        std::fs::create_dir_all(template_root.join("template")).unwrap();
        std::fs::write(
            template_root.join("agent.json"),
            serde_json::to_string_pretty(&json!({
                "kind":"template",
                "name":"local-template",
                "version":"0.1.0",
                "description":"A local template.",
                "template":{
                    "display_name":"Local Template",
                    "use_case":"research",
                    "execution_surfaces":["python-sdk"],
                    "stack":["python"],
                    "files_root":"template",
                    "variables":[{"name":"project_name","description":"Generated project name","required":true,"default":"local-generated"}],
                    "dependencies":{
                        "tools":[{"name":"@zack/echo","version":"0.1.0"}],
                        "agents":[]
                    },
                    "entrypoints":[{"label":"Run","command":"python main.py"}]
                }
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            template_root.join("template/README.md"),
            "# {{ project_name }}\n",
        )
        .unwrap();

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
        let state = Arc::new(TestState {
            base_url: String::new(),
            template_tar: Vec::new(),
            template_sha: "0".repeat(64),
            tool_sha: sha_hex(&tool_tar),
            tool_tar,
            agent_sha: "2".repeat(64),
            agent_tar: Vec::new(),
            knowledge_sha: "3".repeat(64),
            knowledge_tar: Vec::new(),
            memory_sha: "4".repeat(64),
            memory_tar: Vec::new(),
            profile_sha: "5".repeat(64),
            profile_tar: Vec::new(),
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
            .route("/artifact/tool", get(get_tool))
            .route("/artifact/agent", get(get_agent))
            .route("/artifact/knowledge", get(get_knowledge))
            .route("/artifact/memory", get(get_memory))
            .route("/artifact/profile", get(get_profile))
            .with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let target = root.join("generated");
        let args = NewArgs {
            template_ref: template_root.join("agent.json").display().to_string(),
            target_dir: Some(target.clone()),
            vars: Vec::new(),
            quiet: true,
            token: None,
        };

        args.run(base_url).await.unwrap();

        assert!(target.join("README.md").exists());
        assert!(target.join("agent.lock").exists());
        assert!(
            target
                .join(".agentpm/tools/zack/echo/0.1.0/agent.json")
                .exists()
        );

        let template_meta: Value = serde_json::from_str(
            &std::fs::read_to_string(target.join(".agentpm/template.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(template_meta["source"], "local");
        assert_eq!(template_meta["kind"], "template");
        assert_eq!(template_meta["name"], "local-template");
        assert_eq!(template_meta["version"], "0.1.0");
        assert_eq!(
            template_meta["path"],
            Value::String(template_root.canonicalize().unwrap().display().to_string())
        );
        assert!(template_meta.get("integrity").is_none());

        server.abort();
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn rejects_local_template_path_missing_agent_json() {
        let root = temp_dir("missing-local-template-manifest");
        let template_root = root.join("my-template");
        std::fs::create_dir_all(&template_root).unwrap();

        let args = NewArgs {
            template_ref: template_root.display().to_string(),
            target_dir: Some(root.join("generated")),
            vars: Vec::new(),
            quiet: true,
            token: None,
        };

        let err = args
            .run("http://localhost:5000".to_string())
            .await
            .unwrap_err();
        assert!(
            format!("{err:#}").contains("is missing agent.json"),
            "{err:#}"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn rejects_local_template_path_with_non_template_manifest() {
        let root = temp_dir("wrong-kind-local-template");
        let template_root = root.join("my-template");
        std::fs::create_dir_all(&template_root).unwrap();
        std::fs::write(
            template_root.join("agent.json"),
            serde_json::to_string_pretty(&json!({
                "kind":"tool",
                "name":"not-a-template",
                "version":"0.1.0",
                "description":"nope",
                "entrypoint":{"command":"python","args":["main.py"]},
                "runtime":{"type":"python","version":"3.12"},
                "inputs":{},
                "outputs":{},
                "files":["main.py"]
            }))
            .unwrap(),
        )
        .unwrap();

        let args = NewArgs {
            template_ref: template_root.display().to_string(),
            target_dir: Some(root.join("generated")),
            vars: Vec::new(),
            quiet: true,
            token: None,
        };

        let err = args
            .run("http://localhost:5000".to_string())
            .await
            .unwrap_err();
        assert!(
            format!("{err:#}").contains("must have kind=\"template\""),
            "{err:#}"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn rejects_local_template_path_with_schema_invalid_manifest() {
        let root = temp_dir("invalid-local-template");
        let template_root = root.join("my-template");
        std::fs::create_dir_all(&template_root).unwrap();
        std::fs::write(
            template_root.join("agent.json"),
            serde_json::to_string_pretty(&json!({
                "kind":"template",
                "name":"bad-template",
                "version":"0.1.0",
                "description":"invalid",
                "template":{
                    "display_name":"Bad Template",
                    "use_case":"research",
                    "execution_surfaces":["python-sdk"],
                    "variables":[{"name":"BadVar","required":true}]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let args = NewArgs {
            template_ref: template_root.display().to_string(),
            target_dir: Some(root.join("generated")),
            vars: Vec::new(),
            quiet: true,
            token: None,
        };

        let err = args
            .run("http://localhost:5000".to_string())
            .await
            .unwrap_err();
        assert!(
            format!("{err:#}").contains("local template manifest"),
            "{err:#}"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn failed_generation_cleans_up_target_directory() {
        let root = temp_dir("failed-generation");
        let template_root = root.join("template-src");
        std::fs::create_dir_all(template_root.join("template/agents")).unwrap();
        std::fs::write(
            template_root.join("agent.json"),
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
                    "variables":[{"name":"project_name","description":"Project name","required":true,"default":"broken-workspace"}],
                    "dependencies":{"tools":[],"agents":[]},
                    "entrypoints":[{"label":"Run","command":"python main.py"}]
                }
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            template_root.join("template/agents/reviewer.agent.json"),
            serde_json::to_string_pretty(&json!({
                "kind":"agent",
                "name":"reviewer",
                "version":"0.1.0",
                "tools":[]
            }))
            .unwrap(),
        )
        .unwrap();

        let target = root.join("generated");
        let args = NewArgs {
            template_ref: template_root.display().to_string(),
            target_dir: Some(target.clone()),
            vars: Vec::new(),
            quiet: true,
            token: None,
        };

        let err = args.run("http://unused".to_string()).await.unwrap_err();
        let err_text = format!("{err:#}");
        assert!(
            err_text.contains("generated manifest agents/reviewer.agent.json"),
            "{err_text}"
        );
        assert!(!target.exists() || std::fs::read_dir(&target).unwrap().next().is_none());

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn generation_rejects_local_agent_manifests_outside_agents_directory() {
        let root = temp_dir("bad-agent-convention");
        let template_root = root.join("template-src");
        std::fs::create_dir_all(template_root.join("template/services")).unwrap();
        std::fs::write(
            template_root.join("agent.json"),
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
                    "variables":[{"name":"project_name","description":"Project name","required":true,"default":"bad-layout"}],
                    "dependencies":{"tools":[],"agents":[]},
                    "entrypoints":[{"label":"Run","command":"python main.py"}]
                }
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            template_root.join("template/services/reviewer.agent.json"),
            serde_json::to_string_pretty(&json!({
                "kind":"agent",
                "name":"reviewer",
                "version":"0.1.0",
                "description":"Reviewer"
            }))
            .unwrap(),
        )
        .unwrap();

        let target = root.join("generated");
        let args = NewArgs {
            template_ref: template_root.display().to_string(),
            target_dir: Some(target.clone()),
            vars: Vec::new(),
            quiet: true,
            token: None,
        };

        let err = args.run("http://unused".to_string()).await.unwrap_err();
        assert!(
            format!("{err:#}").contains("must live under agents/"),
            "{err:#}"
        );
        assert!(!target.exists() || std::fs::read_dir(&target).unwrap().next().is_none());

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
        knowledge_tar: Vec<u8>,
        knowledge_sha: String,
        memory_tar: Vec<u8>,
        memory_sha: String,
        profile_tar: Vec<u8>,
        profile_sha: String,
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
                "knowledge" => items.push(json!({
                    "kind":"knowledge",
                    "name":name,
                    "version":"0.1.0",
                    "integrity":state.knowledge_sha
                })),
                "memory" => items.push(json!({
                    "kind":"memory",
                    "name":name,
                    "version":"0.1.0",
                    "integrity":state.memory_sha
                })),
                "profile" if name == "@zack/not-a-profile" => items.push(json!({
                    "kind":"memory",
                    "name":name,
                    "version":"0.1.0",
                    "integrity":state.memory_sha
                })),
                "profile" => items.push(json!({
                    "kind":"profile",
                    "name":name,
                    "version":"0.1.0",
                    "integrity":state.profile_sha
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
                "knowledge" => format!("{}/artifact/knowledge", state.base_url),
                "memory" => format!("{}/artifact/memory", state.base_url),
                "profile" => format!("{}/artifact/profile", state.base_url),
                _ => return Err(StatusCode::BAD_REQUEST),
            };
            let integrity = match kind {
                "template" => state.template_sha.as_str(),
                "tool" => state.tool_sha.as_str(),
                "agent" => state.agent_sha.as_str(),
                "knowledge" => state.knowledge_sha.as_str(),
                "memory" => state.memory_sha.as_str(),
                "profile" => state.profile_sha.as_str(),
                _ => return Err(StatusCode::BAD_REQUEST),
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
    async fn get_knowledge(State(state): State<Arc<TestState>>) -> Response<Body> {
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(state.knowledge_tar.clone()))
            .unwrap()
    }

    async fn get_memory(State(state): State<Arc<TestState>>) -> Response<Body> {
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(state.memory_tar.clone()))
            .unwrap()
    }

    async fn get_profile(State(state): State<Arc<TestState>>) -> Response<Body> {
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(state.profile_tar.clone()))
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
