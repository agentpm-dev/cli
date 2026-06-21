use crate::assets::{EXAMPLES_MD_TPL, RUN_SH_TPL, SKILL_MD_TPL, TOOL_CONTRACT_MD_TPL};
use crate::io::download::{download_to, extract_tar_gz};
use crate::manifest::write_manifest_pretty_atomic;
use crate::prelude::*;
use crate::runner::{
    ERR_INSTALLED_TOOL_DIR_MISSING, ERR_INSTALLED_TOOL_NOT_FOUND, ERR_LOCKFILE_DEPENDENCY_MISSING,
    ERR_LOCKFILE_MISSING, ERR_NO_INSTALLED_VERSION_SATISFIES, ERR_NO_INSTALLED_VERSIONS_FOUND,
    ResolvedTool, RunnerManifest, ToolSelector, parse_tool_spec, resolve_installed_tool,
};
use crate::semver::adapt::plan_to_sdk_resolve;
use crate::semver::types::{PackageKind, ResolvePlan, ResolvedPackage};
use anyhow::{Context, anyhow, bail};
use reqwest::Client;
use semver::Version;
use serde_json::Value;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

#[derive(Args, Debug, Clone)]
pub struct ExportArgs {
    /// Generate a starter skill scaffold from the installed tool
    #[arg(long, value_name = "PACKAGE_REF")]
    pub skill: String,

    /// Override the output directory (default: skills/<tool-name>)
    #[arg(long, value_name = "PATH")]
    pub output: Option<PathBuf>,

    /// Also generate a starter kind="skill" agent.json
    #[arg(long)]
    pub manifest: bool,

    /// Overwrite an existing output directory
    #[arg(long)]
    pub force: bool,

    /// Personal Access Token for private remote export fallback
    #[arg(long, value_name = "PAT", env = "AGENTPM_TOKEN")]
    pub token: Option<String>,
}

impl ExportArgs {
    pub async fn run(self, base_url: String) -> Result<()> {
        let cfg = Config::load(base_url)?;
        let project_dir = std::env::current_dir().context("reading current directory")?;
        let token = resolve_token(&cfg, self.token.clone())?;
        self.run_with_dir(&project_dir, &cfg.base_url, token.as_deref())
            .await
    }

    async fn run_with_dir(
        self,
        project_dir: &Path,
        base_url: &str,
        token: Option<&str>,
    ) -> Result<()> {
        let spec = parse_tool_spec(&self.skill)?;
        let resolved = match resolve_installed_tool(project_dir, &spec) {
            Ok(resolved) => ExportResolvedTool::from_installed(resolved),
            Err(err) if remote_export_fallback_allowed(&err) => {
                resolve_remote_tool(base_url, token, &spec).await?
            }
            Err(err) => return Err(err),
        };
        let namespace_collisions = if self.output.is_none() {
            find_default_output_namespace_collisions(project_dir, &resolved)?
        } else {
            Vec::new()
        };
        let output_dir = self
            .output
            .clone()
            .unwrap_or_else(|| default_output_dir(project_dir, &resolved));

        if output_dir.exists() {
            if !self.force {
                bail!(
                    "output directory already exists: {} (pass --force to overwrite)",
                    output_dir.display()
                );
            }
            remove_existing_path(&output_dir)?;
        }

        write_skill_scaffold(&output_dir, &resolved)?;
        if self.manifest {
            write_skill_manifest(&output_dir, &resolved)?;
        }
        if !namespace_collisions.is_empty() {
            eprintln!(
                "Warning: the default output path {} is based only on the tool name. Also installed with this leaf name: {}. Use --output to avoid namespace collisions.",
                output_dir.display(),
                namespace_collisions.join(", ")
            );
        }
        eprintln!("Generated skill scaffold at {}", output_dir.display());
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct ExportResolvedTool {
    package: String,
    version: Version,
    manifest: RunnerManifest,
}

impl ExportResolvedTool {
    fn from_installed(resolved: ResolvedTool) -> Self {
        Self {
            package: resolved.package,
            version: resolved.version,
            manifest: resolved.manifest,
        }
    }
}

fn default_output_dir(project_dir: &Path, resolved: &ExportResolvedTool) -> PathBuf {
    project_dir
        .join("skills")
        .join(slugify_tool_name(&resolved.package))
}

fn find_default_output_namespace_collisions(
    project_dir: &Path,
    resolved: &ExportResolvedTool,
) -> Result<Vec<String>> {
    let package_ref = resolved.package.as_str();
    let Some((namespace, name)) = package_ref.trim_start_matches('@').split_once('/') else {
        return Ok(Vec::new());
    };

    let tools_root = project_dir.join(".agentpm").join("tools");
    if !tools_root.exists() {
        return Ok(Vec::new());
    }

    let mut collisions = Vec::new();
    for entry in
        fs::read_dir(&tools_root).with_context(|| format!("reading {}", tools_root.display()))?
    {
        let entry = entry.with_context(|| format!("reading {}", tools_root.display()))?;
        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }
        let other_namespace = entry.file_name().to_string_lossy().to_string();
        if other_namespace == namespace {
            continue;
        }
        if entry_path.join(name).exists() {
            collisions.push(format!("@{other_namespace}/{name}"));
        }
    }
    collisions.sort();
    Ok(collisions)
}

fn slugify_tool_name(package_ref: &str) -> String {
    package_ref
        .trim_start_matches('@')
        .split_once('/')
        .map(|(_, name)| name.to_string())
        .unwrap_or_else(|| package_ref.trim_start_matches('@').to_string())
}

fn remove_existing_path(path: &Path) -> Result<()> {
    let metadata = fs::metadata(path).with_context(|| format!("reading {}", path.display()))?;
    if metadata.is_dir() {
        fs::remove_dir_all(path).with_context(|| format!("removing {}", path.display()))?;
    } else {
        fs::remove_file(path).with_context(|| format!("removing {}", path.display()))?;
    }
    Ok(())
}

fn write_skill_scaffold(output_dir: &Path, resolved: &ExportResolvedTool) -> Result<()> {
    let references_dir = output_dir.join("references");
    let scripts_dir = output_dir.join("scripts");
    fs::create_dir_all(&references_dir)
        .with_context(|| format!("creating {}", references_dir.display()))?;
    fs::create_dir_all(&scripts_dir)
        .with_context(|| format!("creating {}", scripts_dir.display()))?;

    let skill_md = render_skill_md(resolved);
    let contract_md = render_tool_contract_md(resolved)?;
    let examples_md = render_examples_md(resolved)?;
    let run_sh = render_run_script(resolved);

    fs::write(output_dir.join("SKILL.md"), skill_md)
        .with_context(|| format!("writing {}", output_dir.join("SKILL.md").display()))?;
    fs::write(references_dir.join("tool-contract.md"), contract_md).with_context(|| {
        format!(
            "writing {}",
            references_dir.join("tool-contract.md").display()
        )
    })?;
    fs::write(references_dir.join("examples.md"), examples_md)
        .with_context(|| format!("writing {}", references_dir.join("examples.md").display()))?;
    fs::write(scripts_dir.join("run.sh"), run_sh)
        .with_context(|| format!("writing {}", scripts_dir.join("run.sh").display()))?;
    mark_run_script_executable(&scripts_dir.join("run.sh"))?;
    Ok(())
}

fn write_skill_manifest(output_dir: &Path, resolved: &ExportResolvedTool) -> Result<()> {
    let manifest = serde_json::json!({
        "kind": "skill",
        "name": format!("{}-skill", slugify_tool_name(&resolved.package)),
        "version": "0.1.0",
        "description": format!(
            "Starter skill scaffold for using the {} tool through AgentPM.",
            resolved.package
        ),
        "tools": [
            {
                "name": resolved.package,
                "version": resolved.version.to_string()
            }
        ],
        "skill": {
            "entrypoint": "SKILL.md",
            "references": [
                "references/tool-contract.md",
                "references/examples.md"
            ],
            "scripts": [
                "scripts/run.sh"
            ],
            "compatibility": {
                "runtimes": ["agentpm-run", "shell"]
            }
        }
    });
    write_manifest_pretty_atomic(&output_dir.join("agent.json"), &manifest)
        .with_context(|| format!("writing {}", output_dir.join("agent.json").display()))
}

fn remote_export_fallback_allowed(err: &anyhow::Error) -> bool {
    let message = format!("{err:#}");
    message.contains(ERR_LOCKFILE_MISSING)
        || message.contains(ERR_INSTALLED_TOOL_NOT_FOUND)
        || message.contains(ERR_INSTALLED_TOOL_DIR_MISSING)
        || message.contains(ERR_LOCKFILE_DEPENDENCY_MISSING)
        || message.contains(ERR_NO_INSTALLED_VERSIONS_FOUND)
        || message.contains(ERR_NO_INSTALLED_VERSION_SATISFIES)
}

async fn resolve_remote_tool(
    base_url: &str,
    token: Option<&str>,
    spec: &crate::runner::ToolSpec,
) -> Result<ExportResolvedTool> {
    let mut client = AgentPmClient::new(base_url.to_string())?;
    if let Some(token) = token {
        client = client.with_token(token.to_string());
    }

    let range = match &spec.selector {
        ToolSelector::Locked | ToolSelector::Latest => "*".to_string(),
        ToolSelector::Exact(version) => version.to_string(),
        ToolSelector::Range(req) => req.to_string(),
    };
    let req = agentpm_sdk::models::install::ResolveRequest {
        items: vec![agentpm_sdk::models::install::PackageRequirement {
            kind: agentpm_sdk::models::install::PackageKind::Tool,
            name: spec.package.clone(),
            range,
        }],
    };

    let resolved = client.resolve_install(&req).await?;
    let item = resolved
        .items
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("expected a resolved tool package for {}", spec.package))?;
    if item.kind != agentpm_sdk::models::install::PackageKind::Tool {
        bail!(
            "agentpm export --skill expects a tool package, but registry resolution returned kind=\"{:?}\" for {}",
            item.kind,
            item.name
        );
    }

    let plan = ResolvePlan {
        items: vec![ResolvedPackage {
            kind: PackageKind::Tool,
            name: item.name.clone(),
            version: item.version.clone(),
            integrity: item.integrity.clone(),
        }],
    };
    let init = client.install_init(&plan_to_sdk_resolve(&plan)).await?;
    let artifact = init
        .artifacts
        .into_iter()
        .find(|artifact| artifact.name == item.name && artifact.version == item.version)
        .ok_or_else(|| {
            anyhow!(
                "missing install artifact for {}@{}",
                item.name,
                item.version
            )
        })?;

    read_remote_tool_manifest(&artifact).await
}

async fn read_remote_tool_manifest(
    artifact: &agentpm_sdk::models::install::PackageArtifact,
) -> Result<ExportResolvedTool> {
    let temp_root = temp_export_dir("agentpm-export-remote");
    let artifact_path = temp_root.join("artifact.tgz");
    let extract_dir = temp_root.join("extract");
    let client = Client::new();

    let result = async {
        tokio::fs::create_dir_all(&temp_root)
            .await
            .with_context(|| format!("creating {}", temp_root.display()))?;
        download_to(&client, &artifact.presigned_url, &artifact_path)
            .await
            .with_context(|| format!("downloading {}@{}", artifact.name, artifact.version))?;
        extract_tar_gz(&artifact_path, &extract_dir)
            .await
            .with_context(|| format!("extracting {}@{}", artifact.name, artifact.version))?;

        let manifest_path = extract_dir.join("agent.json");
        let manifest_text = tokio::fs::read_to_string(&manifest_path)
            .await
            .with_context(|| format!("reading {}", manifest_path.display()))?;
        let raw: Value = serde_json::from_str(&manifest_text)
            .with_context(|| format!("parsing JSON from {}", manifest_path.display()))?;
        let manifest: RunnerManifest = serde_json::from_value(raw)
            .with_context(|| format!("parsing manifest at {}", manifest_path.display()))?;
        if manifest.kind != "tool" {
            bail!(
                "manifest at {} has kind=\"{}\"; agentpm export --skill requires a tool source package",
                manifest_path.display(),
                manifest.kind
            );
        }

        Ok::<ExportResolvedTool, anyhow::Error>(ExportResolvedTool {
            package: artifact.name.clone(),
            version: Version::parse(&artifact.version).with_context(|| {
                format!("resolved version for {} is not valid semver", artifact.name)
            })?,
            manifest,
        })
    }
    .await;

    let _ = tokio::fs::remove_dir_all(&temp_root).await;
    result
}

fn temp_export_dir(prefix: &str) -> PathBuf {
    let unique = format!(
        "{prefix}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos()
    );
    std::env::temp_dir().join(unique)
}

fn mark_run_script_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(path)
            .with_context(|| format!("reading {}", path.display()))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .with_context(|| format!("updating permissions for {}", path.display()))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

fn render_skill_md(resolved: &ExportResolvedTool) -> String {
    let package_ref = &resolved.package;
    let tool_name = &resolved.manifest.name;
    let title = title_case(tool_name);
    let resolved_version = resolved.version.to_string();
    let skill_name = slugify_tool_name(package_ref);
    let skill_description = resolved
        .manifest
        .description
        .clone()
        .unwrap_or_else(|| {
            format!(
                "Use this skill when you want to run the {} tool through AgentPM from a skill-capable client while keeping execution delegated to agentpm run.",
                package_ref
            )
        });
    let quick_start_input = minimal_input_from_schema(&resolved.manifest.inputs, None);
    let inline_input =
        serde_json::to_string(&quick_start_input).expect("serializing generated example input");
    let description = resolved
        .manifest
        .description
        .as_deref()
        .unwrap_or("No description provided in the tool manifest.");
    render_template(
        SKILL_MD_TPL,
        &[
            ("SKILL_NAME", skill_name.as_str()),
            ("SKILL_DESCRIPTION", skill_description.as_str()),
            ("TITLE", title.as_str()),
            ("PACKAGE_REF", package_ref.as_str()),
            ("INLINE_INPUT", inline_input.as_str()),
            ("RESOLVED_VERSION", resolved_version.as_str()),
            ("DESCRIPTION", description),
        ],
    )
}

fn render_tool_contract_md(resolved: &ExportResolvedTool) -> Result<String> {
    let runtime = match &resolved.manifest.runtime {
        Some(runtime) => match &runtime.version {
            Some(version) => format!("{} ({})", runtime.runtime_type, version),
            None => runtime.runtime_type.clone(),
        },
        None => "Not declared".to_string(),
    };
    let entrypoint_args = serde_json::to_string_pretty(&resolved.manifest.entrypoint.args)
        .context("formatting entrypoint args as JSON")?;

    let environment = render_environment_requirements(resolved);
    let input_schema = format_optional_schema(&resolved.manifest.inputs)
        .context("formatting input schema as JSON")?;
    let output_schema = format_optional_schema(&resolved.manifest.outputs)
        .context("formatting output schema as JSON")?;

    let resolved_version = resolved.version.to_string();
    let timeout_ms = resolved.manifest.entrypoint.timeout_ms.to_string();
    Ok(render_template(
        TOOL_CONTRACT_MD_TPL,
        &[
            ("PACKAGE_REF", resolved.package.as_str()),
            ("RESOLVED_VERSION", resolved_version.as_str()),
            ("MANIFEST_NAME", resolved.manifest.name.as_str()),
            ("MANIFEST_VERSION", resolved.manifest.version.as_str()),
            (
                "DESCRIPTION",
                resolved
                    .manifest
                    .description
                    .as_deref()
                    .unwrap_or("No description provided in the tool manifest."),
            ),
            ("RUNTIME", runtime.as_str()),
            (
                "ENTRYPOINT_COMMAND",
                resolved.manifest.entrypoint.command.as_str(),
            ),
            ("ENTRYPOINT_ARGS", entrypoint_args.as_str()),
            ("ENTRYPOINT_CWD", resolved.manifest.entrypoint.cwd.as_str()),
            ("TIMEOUT_MS", timeout_ms.as_str()),
            ("ENVIRONMENT", environment.as_str()),
            ("INPUT_SCHEMA", input_schema.as_str()),
            ("OUTPUT_SCHEMA", output_schema.as_str()),
        ],
    ))
}

fn render_environment_requirements(resolved: &ExportResolvedTool) -> String {
    let Some(environment) = &resolved.manifest.environment else {
        return "No environment requirements declared.".to_string();
    };
    if environment.vars.is_empty() {
        return "No environment requirements declared.".to_string();
    }

    let mut lines = Vec::new();
    for (name, rule) in &environment.vars {
        let mut parts = Vec::new();
        if rule.required {
            parts.push("required".to_string());
        } else {
            parts.push("optional".to_string());
        }
        if let Some(default) = &rule.default {
            parts.push(format!("default: `{default}`"));
        }
        lines.push(format!("- `{name}` — {}", parts.join(", ")));
    }
    lines.join("\n")
}

fn render_examples_md(resolved: &ExportResolvedTool) -> Result<String> {
    let package_ref = &resolved.package;
    let minimal_example = minimal_input_from_schema(&resolved.manifest.inputs, None);
    let optional_example = single_optional_input_from_schema(&resolved.manifest.inputs)
        .unwrap_or_else(|| minimal_example.clone());
    let richer_example = example_input_from_schema(&resolved.manifest.inputs, None);
    let inline_input =
        serde_json::to_string(&minimal_example).context("formatting inline example JSON")?;
    let optional_input = serde_json::to_string_pretty(&optional_example)
        .context("formatting optional example payload")?;
    let richer_input =
        serde_json::to_string_pretty(&richer_example).context("formatting richer example JSON")?;
    let richer_inline_input =
        serde_json::to_string(&richer_example).context("formatting richer inline example JSON")?;

    Ok(render_template(
        EXAMPLES_MD_TPL,
        &[
            ("PACKAGE_REF", package_ref.as_str()),
            ("INLINE_INPUT", inline_input.as_str()),
            ("OPTIONAL_INPUT", optional_input.as_str()),
            ("RICHER_INPUT", richer_input.as_str()),
            ("RICHER_INLINE_INPUT", richer_inline_input.as_str()),
        ],
    ))
}

fn render_run_script(resolved: &ExportResolvedTool) -> String {
    render_template(RUN_SH_TPL, &[("PACKAGE_REF", resolved.package.as_str())])
}

fn example_input_from_schema(schema: &Value, property_name: Option<&str>) -> Value {
    if let Some(example) = schema.get("example") {
        return example.clone();
    }
    if let Some(default) = schema.get("default") {
        return default.clone();
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("object") => {
            let mut map = serde_json::Map::new();
            if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
                for (name, property) in properties {
                    map.insert(
                        name.clone(),
                        example_input_from_schema(property, Some(name)),
                    );
                }
            }
            Value::Object(map)
        }
        Some("array") => Value::Array(Vec::new()),
        Some("string") => Value::String(default_string_example(property_name)),
        Some("integer") => default_integer_example(schema),
        Some("number") => default_number_example(schema),
        Some("boolean") => Value::Bool(true),
        _ => Value::Object(Default::default()),
    }
}

fn minimal_input_from_schema(schema: &Value, property_name: Option<&str>) -> Value {
    if let Some(example) = schema.get("example") {
        return example.clone();
    }
    if let Some(default) = schema.get("default") {
        return default.clone();
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("object") => {
            let mut map = serde_json::Map::new();
            let required = schema
                .get("required")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<std::collections::HashSet<_>>()
                })
                .unwrap_or_default();
            if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
                for (name, property) in properties {
                    if required.contains(name.as_str()) {
                        map.insert(
                            name.clone(),
                            minimal_input_from_schema(property, Some(name)),
                        );
                    }
                }
            }
            Value::Object(map)
        }
        Some("array") => Value::Array(Vec::new()),
        Some("string") => Value::String(default_string_example(property_name)),
        Some("integer") => default_integer_example(schema),
        Some("number") => default_number_example(schema),
        Some("boolean") => Value::Bool(true),
        _ => Value::Object(Default::default()),
    }
}

fn single_optional_input_from_schema(schema: &Value) -> Option<Value> {
    let Value::Object(minimal) = minimal_input_from_schema(schema, None) else {
        return None;
    };
    let properties = schema.get("properties")?.as_object()?;
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<std::collections::HashSet<_>>()
        })
        .unwrap_or_default();

    // Heuristic: promote the first optional property we encounter into the
    // "one optional field" example. With serde_json's default Map this is
    // alphabetic by key; if preserve_order is enabled later it will instead
    // follow manifest insertion order. That is acceptable for a starter
    // scaffold, but this function does not promise a semantically "best"
    // optional field.
    for (name, property) in properties {
        if !required.contains(name.as_str()) {
            let mut with_optional = minimal.clone();
            with_optional.insert(
                name.clone(),
                example_input_from_schema(property, Some(name.as_str())),
            );
            return Some(Value::Object(with_optional));
        }
    }

    None
}

fn default_string_example(property_name: Option<&str>) -> String {
    match property_name {
        Some("text") => "Hello world".to_string(),
        Some("message") => "Hello world".to_string(),
        Some(name) => format!("<{name}>"),
        None => "<value>".to_string(),
    }
}

fn default_integer_example(schema: &Value) -> Value {
    schema
        .get("minimum")
        .and_then(Value::as_i64)
        .map(Value::from)
        .unwrap_or_else(|| Value::from(0))
}

fn default_number_example(schema: &Value) -> Value {
    schema
        .get("minimum")
        .and_then(Value::as_f64)
        .and_then(serde_json::Number::from_f64)
        .map(Value::Number)
        .unwrap_or_else(|| Value::from(0))
}

fn format_optional_schema(schema: &Value) -> Result<String> {
    if schema.is_null() || matches!(schema, Value::Object(map) if map.is_empty()) {
        Ok("Not declared.".to_string())
    } else {
        serde_json::to_string_pretty(schema).context("formatting schema as JSON")
    }
}

fn title_case(raw: &str) -> String {
    raw.split(['-', '_', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_template(tpl: &str, vars: &[(&str, &str)]) -> String {
    let mut out = tpl.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{{{}}}}}", k), v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{resolve_schema_source, validate_manifest_value};
    use agentpm_sdk::models::install as sdkm;
    use axum::Router;
    use axum::body::Body;
    use axum::extract::State;
    use axum::http::header::AUTHORIZATION;
    use axum::http::{HeaderMap, Response, StatusCode};
    use axum::routing::{get, post};
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use sha2::{Digest, Sha256};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tar::Builder;

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    const TEST_BASE_URL: &str = "http://127.0.0.1:9";

    #[tokio::test]
    async fn generates_expected_file_structure() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            rich_tool_manifest("python3", "0.1.0"),
            python_echo_script(),
        );

        let args = ExportArgs {
            skill: "@zack/echo-json".to_string(),
            output: None,
            manifest: false,
            force: false,
            token: None,
        };

        args.run_with_dir(root.path(), TEST_BASE_URL, None)
            .await
            .unwrap();

        let skill_dir = root.path().join("skills").join("echo-json");
        assert!(skill_dir.join("SKILL.md").exists());
        assert!(skill_dir.join("references/tool-contract.md").exists());
        assert!(skill_dir.join("references/examples.md").exists());
        assert!(skill_dir.join("scripts/run.sh").exists());
        assert!(!skill_dir.join("agent.json").exists());

        #[cfg(unix)]
        {
            let mode = fs::metadata(skill_dir.join("scripts/run.sh"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111);
        }
    }

    #[tokio::test]
    async fn refuses_to_overwrite_existing_output_without_force() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            rich_tool_manifest("python3", "0.1.0"),
            python_echo_script(),
        );

        let args = ExportArgs {
            skill: "@zack/echo-json".to_string(),
            output: None,
            manifest: false,
            force: false,
            token: None,
        };
        args.clone()
            .run_with_dir(root.path(), TEST_BASE_URL, None)
            .await
            .unwrap();

        let err = args
            .run_with_dir(root.path(), TEST_BASE_URL, None)
            .await
            .unwrap_err();
        assert!(
            format!("{err:#}").contains("output directory already exists"),
            "{err:#}"
        );
    }

    #[tokio::test]
    async fn overwrites_existing_output_with_force() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            rich_tool_manifest("python3", "0.1.0"),
            python_echo_script(),
        );

        let first = ExportArgs {
            skill: "@zack/echo-json".to_string(),
            output: None,
            manifest: true,
            force: false,
            token: None,
        };
        first
            .run_with_dir(root.path(), TEST_BASE_URL, None)
            .await
            .unwrap();

        let skill_dir = root.path().join("skills").join("echo-json");
        fs::write(skill_dir.join("SKILL.md"), "stale content").unwrap();
        fs::write(skill_dir.join("agent.json"), "{\"stale\":true}\n").unwrap();

        let second = ExportArgs {
            skill: "@zack/echo-json".to_string(),
            output: None,
            manifest: true,
            force: true,
            token: None,
        };
        second
            .run_with_dir(root.path(), TEST_BASE_URL, None)
            .await
            .unwrap();

        let skill_md = fs::read_to_string(skill_dir.join("SKILL.md")).unwrap();
        let exported_manifest = fs::read_to_string(skill_dir.join("agent.json")).unwrap();
        assert!(skill_md.starts_with("---\nname: echo-json\n"));
        assert!(skill_md.contains("When to use this skill"));
        assert!(!skill_md.contains("stale content"));
        assert!(exported_manifest.contains("\"kind\": \"skill\""));
        assert!(!exported_manifest.contains("stale"));
    }

    #[tokio::test]
    async fn writes_manifest_derived_content() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            rich_tool_manifest("python3", "0.1.0"),
            python_echo_script(),
        );

        let args = ExportArgs {
            skill: "@zack/echo-json".to_string(),
            output: None,
            manifest: false,
            force: false,
            token: None,
        };
        args.run_with_dir(root.path(), TEST_BASE_URL, None)
            .await
            .unwrap();

        let skill_dir = root.path().join("skills").join("echo-json");
        let skill_md = fs::read_to_string(skill_dir.join("SKILL.md")).unwrap();
        let contract_md =
            fs::read_to_string(skill_dir.join("references/tool-contract.md")).unwrap();
        let examples_md = fs::read_to_string(skill_dir.join("references/examples.md")).unwrap();

        assert!(skill_md.starts_with("---\nname: echo-json\n"));
        assert!(skill_md.contains("description: Echo tool for skill export tests"));
        assert!(skill_md.contains("When to use this skill"));
        assert!(
            skill_md
                .contains("agentpm run @zack/echo-json --input '{\"message\":\"Hello world\"}'")
        );
        assert!(skill_md.contains("TODO: Add the specific workflow cues"));
        assert!(contract_md.contains("Echo tool for skill export tests"));
        assert!(contract_md.contains("\"message\""));
        assert!(contract_md.contains("`API_TOKEN` — required"));
        assert!(examples_md.contains("./scripts/run.sh"));
        assert!(examples_md.contains("## Expanded example"));
    }

    #[tokio::test]
    async fn supports_output_override() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            rich_tool_manifest("python3", "0.1.0"),
            python_echo_script(),
        );

        let custom = root.path().join("custom-skill-output");
        let args = ExportArgs {
            skill: "@zack/echo-json".to_string(),
            output: Some(custom.clone()),
            manifest: false,
            force: false,
            token: None,
        };
        args.run_with_dir(root.path(), TEST_BASE_URL, None)
            .await
            .unwrap();

        assert!(custom.join("SKILL.md").exists());
        assert!(custom.join("references/tool-contract.md").exists());
    }

    #[tokio::test]
    async fn falls_back_to_generic_frontmatter_description_when_manifest_omits_one() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            minimal_tool_manifest_without_description("python3", "0.1.0"),
            python_echo_script(),
        );

        let args = ExportArgs {
            skill: "@zack/echo-json".to_string(),
            output: None,
            manifest: false,
            force: false,
            token: None,
        };
        args.run_with_dir(root.path(), TEST_BASE_URL, None)
            .await
            .unwrap();

        let skill_dir = root.path().join("skills").join("echo-json");
        let skill_md = fs::read_to_string(skill_dir.join("SKILL.md")).unwrap();
        assert!(skill_md.contains(
            "description: Use this skill when you want to run the @zack/echo-json tool through AgentPM from a skill-capable client while keeping execution delegated to agentpm run."
        ));
    }

    #[test]
    fn detects_default_output_namespace_collisions() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            rich_tool_manifest("python3", "0.1.0"),
            python_echo_script(),
        );
        root.write_tool(
            "@acme/echo-json",
            "0.2.0",
            rich_tool_manifest("python3", "0.2.0"),
            python_echo_script(),
        );

        let spec = parse_tool_spec("@zack/echo-json").expect("parse tool spec");
        let resolved = ExportResolvedTool::from_installed(
            resolve_installed_tool(root.path(), &spec).expect("resolve installed tool"),
        );

        let collisions = find_default_output_namespace_collisions(root.path(), &resolved)
            .expect("detect namespace collisions");

        assert_eq!(collisions, vec!["@acme/echo-json"]);
    }

    #[tokio::test]
    async fn installed_tool_export_with_manifest_generates_valid_skill_manifest() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            rich_tool_manifest("python3", "0.1.0"),
            python_echo_script(),
        );

        let args = ExportArgs {
            skill: "@zack/echo-json".to_string(),
            output: None,
            manifest: true,
            force: false,
            token: None,
        };
        args.run_with_dir(root.path(), TEST_BASE_URL, None)
            .await
            .unwrap();

        let manifest_path = root.path().join("skills/echo-json/agent.json");
        let mut manifest: Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        let schema_source = resolve_schema_source(None);
        let (ok, issues) =
            validate_manifest_value(&schema_source, "agent.json", &mut manifest, false).unwrap();
        assert!(
            ok,
            "expected exported skill manifest to validate: {issues:#?}"
        );
        assert_eq!(manifest["kind"], "skill");
        assert_eq!(manifest["name"], "echo-json-skill");
        assert_eq!(
            manifest["tools"],
            serde_json::json!([{ "name": "@zack/echo-json", "version": "0.1.0" }])
        );
        assert_eq!(manifest["skill"]["entrypoint"], "SKILL.md");
        assert_eq!(
            manifest["skill"]["references"],
            serde_json::json!(["references/tool-contract.md", "references/examples.md"])
        );
        assert_eq!(
            manifest["skill"]["scripts"],
            serde_json::json!(["scripts/run.sh"])
        );
        assert_eq!(
            manifest["skill"]["compatibility"]["runtimes"],
            serde_json::json!(["agentpm-run", "shell"])
        );
    }

    #[tokio::test]
    async fn output_override_and_manifest_work_together() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            rich_tool_manifest("python3", "0.1.0"),
            python_echo_script(),
        );

        let custom = root.path().join("custom-export-with-manifest");
        let args = ExportArgs {
            skill: "@zack/echo-json".to_string(),
            output: Some(custom.clone()),
            manifest: true,
            force: false,
            token: None,
        };
        args.run_with_dir(root.path(), TEST_BASE_URL, None)
            .await
            .unwrap();

        assert!(custom.join("SKILL.md").exists());
        assert!(custom.join("references/tool-contract.md").exists());
        assert!(custom.join("references/examples.md").exists());
        assert!(custom.join("scripts/run.sh").exists());
        assert!(custom.join("agent.json").exists());

        let mut manifest: Value =
            serde_json::from_str(&fs::read_to_string(custom.join("agent.json")).unwrap()).unwrap();
        let schema_source = resolve_schema_source(None);
        let (ok, issues) =
            validate_manifest_value(&schema_source, "agent.json", &mut manifest, false).unwrap();
        assert!(
            ok,
            "expected output override manifest to validate: {issues:#?}"
        );
        assert_eq!(manifest["name"], "echo-json-skill");
    }

    #[tokio::test]
    async fn remote_export_with_manifest_generates_scaffold_without_local_install_mutation() {
        let root = TestProject::new();
        let workspace_before = r#"{"schema_version":1,"manifests":["agent.json"],"package_roots":{"tools":[],"agents":[],"skills":[]}}"#;
        root.write_lock(
            r#"{"lockfile_version":1,"generated":"2026-05-03T00:00:00Z","dependencies":{}}"#
                .to_string(),
        );
        fs::write(root.path().join("agentpm.workspace.json"), workspace_before).unwrap();

        let tool_tar = build_tarball(&[
            ("agent.json", rich_tool_manifest("python3", "0.1.23")),
            ("script.py", python_echo_script()),
        ]);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://127.0.0.1:{}", addr.port());
        let state = Arc::new(RemoteExportState {
            base_url: base_url.clone(),
            artifact: tool_tar.clone(),
            artifact_sha: sha_hex(&tool_tar),
            auth_headers: Arc::new(Mutex::new(Vec::new())),
        });
        let app = Router::new()
            .route("/v1/tools/install/resolve", post(remote_resolve))
            .route("/v1/tools/install/init", post(remote_init))
            .route("/artifact/tool", get(remote_artifact))
            .with_state(state.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let args = ExportArgs {
            skill: "@zack/echo-json".to_string(),
            output: Some(root.path().join("remote-export")),
            manifest: true,
            force: false,
            token: None,
        };
        args.run_with_dir(root.path(), &base_url, None)
            .await
            .unwrap();

        let out = root.path().join("remote-export");
        assert!(out.join("SKILL.md").exists());
        assert!(out.join("references/tool-contract.md").exists());
        assert!(out.join("references/examples.md").exists());
        assert!(out.join("scripts/run.sh").exists());
        assert!(out.join("agent.json").exists());

        let mut exported_manifest: Value =
            serde_json::from_str(&fs::read_to_string(out.join("agent.json")).unwrap()).unwrap();
        let schema_source = resolve_schema_source(None);
        let (ok, issues) =
            validate_manifest_value(&schema_source, "agent.json", &mut exported_manifest, false)
                .unwrap();
        assert!(
            ok,
            "expected remote exported manifest to validate: {issues:#?}"
        );
        assert_eq!(
            exported_manifest["tools"],
            serde_json::json!([{ "name": "@zack/echo-json", "version": "0.1.23" }])
        );

        assert_eq!(
            fs::read_to_string(root.path().join("agent.lock")).unwrap(),
            r#"{"lockfile_version":1,"generated":"2026-05-03T00:00:00Z","dependencies":{}}"#
        );
        assert_eq!(
            fs::read_to_string(root.path().join("agentpm.workspace.json")).unwrap(),
            workspace_before
        );
        assert!(!root.path().join(".agentpm").exists());

        server.abort();
    }

    #[tokio::test]
    async fn remote_export_without_manifest_keeps_backward_compatible_scaffold_shape() {
        let root = TestProject::new();
        let tool_tar = build_tarball(&[
            ("agent.json", rich_tool_manifest("python3", "0.1.23")),
            ("script.py", python_echo_script()),
        ]);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://127.0.0.1:{}", addr.port());
        let state = Arc::new(RemoteExportState {
            base_url: base_url.clone(),
            artifact: tool_tar.clone(),
            artifact_sha: sha_hex(&tool_tar),
            auth_headers: Arc::new(Mutex::new(Vec::new())),
        });
        let app = Router::new()
            .route("/v1/tools/install/resolve", post(remote_resolve))
            .route("/v1/tools/install/init", post(remote_init))
            .route("/artifact/tool", get(remote_artifact))
            .with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let args = ExportArgs {
            skill: "@zack/echo-json".to_string(),
            output: Some(root.path().join("remote-export-no-manifest")),
            manifest: false,
            force: false,
            token: None,
        };
        args.run_with_dir(root.path(), &base_url, None)
            .await
            .unwrap();

        let out = root.path().join("remote-export-no-manifest");
        assert!(out.join("SKILL.md").exists());
        assert!(out.join("references/tool-contract.md").exists());
        assert!(out.join("references/examples.md").exists());
        assert!(out.join("scripts/run.sh").exists());
        assert!(!out.join("agent.json").exists());
        assert!(!root.path().join(".agentpm").exists());

        server.abort();
    }

    #[tokio::test]
    async fn remote_export_sends_credentials_when_available() {
        let root = TestProject::new();
        let tool_tar = build_tarball(&[
            ("agent.json", rich_tool_manifest("python3", "0.1.23")),
            ("script.py", python_echo_script()),
        ]);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://127.0.0.1:{}", addr.port());
        let auth_headers = Arc::new(Mutex::new(Vec::new()));
        let state = Arc::new(RemoteExportState {
            base_url: base_url.clone(),
            artifact: tool_tar.clone(),
            artifact_sha: sha_hex(&tool_tar),
            auth_headers: auth_headers.clone(),
        });
        let app = Router::new()
            .route("/v1/tools/install/resolve", post(remote_resolve))
            .route("/v1/tools/install/init", post(remote_init))
            .route("/artifact/tool", get(remote_artifact))
            .with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let args = ExportArgs {
            skill: "@zack/echo-json".to_string(),
            output: Some(root.path().join("private-remote-export")),
            manifest: true,
            force: false,
            token: Some("apm_test_remote_token".to_string()),
        };
        args.run_with_dir(root.path(), &base_url, Some("apm_test_remote_token"))
            .await
            .unwrap();

        let seen = auth_headers.lock().unwrap().clone();
        assert!(
            seen.iter()
                .all(|value| value == "Bearer apm_test_remote_token"),
            "expected bearer token on resolve/init requests, got: {seen:#?}"
        );
        assert!(
            seen.len() >= 2,
            "expected auth header on both resolve and init requests, got: {seen:#?}"
        );

        server.abort();
    }

    #[test]
    fn minimal_input_uses_required_fields_only() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "text": { "type": "string" },
                "model": { "type": "string" },
                "doSummary": { "type": "boolean" },
                "maxSummaryChars": { "type": "integer", "minimum": 40 }
            },
            "required": ["text"]
        });

        let input = minimal_input_from_schema(&schema, None);

        assert_eq!(input, serde_json::json!({ "text": "Hello world" }));
    }

    #[test]
    fn richer_input_respects_integer_minimums() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "text": { "type": "string" },
                "maxSummaryChars": { "type": "integer", "minimum": 40 }
            },
            "required": ["text"]
        });

        let input = example_input_from_schema(&schema, None);

        assert_eq!(
            input,
            serde_json::json!({
                "text": "Hello world",
                "maxSummaryChars": 40
            })
        );
    }

    #[test]
    fn optional_input_adds_one_optional_field() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "text": { "type": "string" },
                "doSummary": { "type": "boolean" },
                "model": { "type": "string" }
            },
            "required": ["text"]
        });

        let input = single_optional_input_from_schema(&schema).expect("optional example");

        assert_eq!(
            input,
            serde_json::json!({
                "text": "Hello world",
                "doSummary": true
            })
        );
    }

    #[test]
    fn string_placeholder_uses_property_name() {
        assert_eq!(default_string_example(Some("model")), "<model>");
        assert_eq!(default_string_example(Some("query")), "<query>");
        assert_eq!(default_string_example(None), "<value>");
        assert_eq!(default_string_example(Some("text")), "Hello world");
    }

    fn lock_for(package: &str, version: &str) -> String {
        format!(
            r#"{{
  "lockfile_version": 1,
  "generated": "2026-05-03T00:00:00Z",
  "dependencies": {{
    "{package}": {{
      "version": "{version}",
      "integrity": "abc"
    }}
  }}
}}"#
        )
    }

    fn rich_tool_manifest(command: &str, version: &str) -> String {
        format!(
            r#"{{
  "kind": "tool",
  "name": "echo-json",
  "version": "{version}",
  "description": "Echo tool for skill export tests",
  "entrypoint": {{
    "command": "{command}",
    "args": ["script.py"],
    "cwd": ".",
    "timeout_ms": 5000,
    "env": {{
      "MODE": "skill"
    }}
  }},
  "runtime": {{
    "type": "python",
    "version": ">=3.10"
  }},
  "environment": {{
    "vars": {{
      "API_TOKEN": {{
        "required": true
      }},
      "REGION": {{
        "default": "us-west-2"
      }}
    }}
  }},
  "inputs": {{
    "type": "object",
    "properties": {{
      "message": {{
        "type": "string"
      }}
    }},
    "required": ["message"]
  }},
  "outputs": {{
    "type": "object",
    "properties": {{
      "upper": {{
        "type": "string"
      }}
    }},
    "required": ["upper"]
  }}
}}"#
        )
    }

    fn minimal_tool_manifest_without_description(command: &str, version: &str) -> String {
        format!(
            r#"{{
  "kind": "tool",
  "name": "echo-json",
  "version": "{version}",
  "entrypoint": {{
    "command": "{command}",
    "args": ["script.py"],
    "cwd": ".",
    "timeout_ms": 5000
  }},
  "inputs": {{
    "type": "object",
    "properties": {{
      "message": {{
        "type": "string"
      }}
    }}
  }},
  "outputs": {{
    "type": "object",
    "properties": {{
      "upper": {{
        "type": "string"
      }}
    }}
  }}
}}"#
        )
    }

    fn python_echo_script() -> String {
        r#"import json
import sys

payload = json.load(sys.stdin)
json.dump({"upper": payload.get("message", "").upper()}, sys.stdout)
"#
        .to_string()
    }

    fn build_tarball(entries: &[(&str, String)]) -> Vec<u8> {
        let gz = GzEncoder::new(Vec::new(), Compression::default());
        let mut tar = Builder::new(gz);
        for (path, contents) in entries {
            let bytes = contents.as_bytes();
            let mut header = tar::Header::new_gnu();
            header.set_path(path).unwrap();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append(&header, bytes).unwrap();
        }
        tar.finish().unwrap();
        tar.into_inner().unwrap().finish().unwrap()
    }

    fn sha_hex(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hex::encode(hasher.finalize())
    }

    #[derive(Clone)]
    struct RemoteExportState {
        base_url: String,
        artifact: Vec<u8>,
        artifact_sha: String,
        auth_headers: Arc<Mutex<Vec<String>>>,
    }

    async fn remote_resolve(
        headers: HeaderMap,
        State(state): State<Arc<RemoteExportState>>,
        axum::Json(req): axum::Json<sdkm::ResolveRequest>,
    ) -> (StatusCode, axum::Json<sdkm::ResolveResponse>) {
        capture_auth(&headers, &state);
        assert_eq!(req.items.len(), 1);
        assert_eq!(req.items[0].name, "@zack/echo-json");
        (
            StatusCode::OK,
            axum::Json(sdkm::ResolveResponse {
                items: vec![sdkm::ResolvedPackage {
                    kind: sdkm::PackageKind::Tool,
                    name: "@zack/echo-json".to_string(),
                    version: "0.1.23".to_string(),
                    integrity: state.artifact_sha.clone(),
                }],
            }),
        )
    }

    async fn remote_init(
        headers: HeaderMap,
        State(state): State<Arc<RemoteExportState>>,
        axum::Json(req): axum::Json<sdkm::ResolveResponse>,
    ) -> (StatusCode, axum::Json<sdkm::InstallInitResponse>) {
        capture_auth(&headers, &state);
        assert_eq!(req.items.len(), 1);
        (
            StatusCode::OK,
            axum::Json(sdkm::InstallInitResponse {
                session_id: "sess_export".to_string(),
                expires_at: "2026-06-21T18:00:00Z".to_string(),
                artifacts: vec![sdkm::PackageArtifact {
                    kind: sdkm::PackageKind::Tool,
                    name: "@zack/echo-json".to_string(),
                    version: "0.1.23".to_string(),
                    integrity: state.artifact_sha.clone(),
                    presigned_url: format!("{}/artifact/tool", state.base_url),
                    size: Some(state.artifact.len() as u64),
                    content_type: Some("application/gzip".to_string()),
                    signing: None,
                    runtime: None,
                }],
            }),
        )
    }

    async fn remote_artifact(State(state): State<Arc<RemoteExportState>>) -> Response<Body> {
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(state.artifact.clone()))
            .unwrap()
    }

    fn capture_auth(headers: &HeaderMap, state: &RemoteExportState) {
        if let Some(value) = headers.get(AUTHORIZATION) {
            state
                .auth_headers
                .lock()
                .unwrap()
                .push(value.to_str().unwrap().to_string());
        }
    }

    struct TestProject {
        root: PathBuf,
    }

    impl TestProject {
        fn new() -> Self {
            let mut root = std::env::temp_dir();
            let unique = format!(
                "agentpm-export-test-{}-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            );
            root.push(unique);
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn path(&self) -> &Path {
            &self.root
        }

        fn write_lock(&self, lock: String) {
            fs::write(self.root.join("agent.lock"), lock).unwrap();
        }

        fn write_tool(&self, package: &str, version: &str, manifest: String, script: String) {
            let dir = self.tool_dir(package, version);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("agent.json"), manifest).unwrap();
            fs::write(dir.join("script.py"), script).unwrap();
        }

        fn tool_dir(&self, package: &str, version: &str) -> PathBuf {
            let trimmed = package.trim_start_matches('@');
            let (namespace, name) = trimmed.split_once('/').unwrap();
            self.root
                .join(".agentpm")
                .join("tools")
                .join(namespace)
                .join(name)
                .join(version)
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
