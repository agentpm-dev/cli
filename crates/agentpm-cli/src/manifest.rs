use crate::semver::types::Lock;
use anyhow::{Context, Result, anyhow};
use jsonschema::{Draft, JSONSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

const DEFAULT_MANIFEST_SCHEMA_URL: &str = "https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json";
const EMBEDDED_MANIFEST_SCHEMA_JSON: &str =
    include_str!("../../../schemas/agentpm.manifest.schema.json");

#[derive(Serialize, Debug, Clone)]
pub struct LintIssue {
    pub file: String,
    pub level: &'static str, // "error" | "warning"
    pub message: String,
    pub instance_path: String,
    pub schema_path: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct LintFileReport {
    pub file: String,
    pub ok: bool,
    pub issues: Vec<LintIssue>,
}

fn default_cwd() -> String {
    ".".into()
}
fn default_timeout_ms() -> u64 {
    60_000
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Entrypoint {
    pub command: String,   // required
    pub args: Vec<String>, // default: []
    #[serde(default = "default_cwd")]
    pub cwd: String, // default: "."
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64, // default: 60000
    pub env: HashMap<String, String>, // default: {}
}

/// Minimal shape we need from agent.json for publish.
/// Keep it liberal (Value) for forward-compat fields.
#[derive(Debug, Deserialize)]
pub struct ToolManifest {
    pub kind: String,
    pub name: String,
    pub version: String,
    #[allow(dead_code)]
    pub description: Option<String>,
    pub entrypoint: Entrypoint,
    #[serde(default)]
    pub files: Vec<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub runtime: Value,
    #[allow(dead_code)]
    #[serde(default)]
    pub inputs: Value,
    #[allow(dead_code)]
    #[serde(default)]
    pub outputs: Value,
    // allow unknowns to pass through
}

#[derive(Debug, Deserialize)]
pub struct AgentManifest {
    pub kind: String,
    pub name: String,
    pub version: String,
    #[allow(dead_code)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TemplateMetadata {
    #[allow(dead_code)]
    pub display_name: Option<String>,
    #[allow(dead_code)]
    pub use_case: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub execution_surfaces: Vec<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub stack: Vec<String>,
    pub files_root: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub variables: Vec<TemplateVariable>,
    #[allow(dead_code)]
    #[serde(default)]
    pub dependencies: TemplateDependencies,
    #[allow(dead_code)]
    #[serde(default)]
    pub entrypoints: Vec<TemplateEntrypoint>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TemplateVariable {
    pub name: String,
    #[allow(dead_code)]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum PackageReference {
    String(String),
    Object {
        name: String,
        #[serde(default)]
        version: Option<String>,
    },
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct TemplateDependencies {
    pub tools: Vec<PackageReference>,
    pub agents: Vec<PackageReference>,
    pub skills: Vec<PackageReference>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TemplateEntrypoint {
    pub label: String,
    pub command: String,
}

#[derive(Debug, Deserialize)]
pub struct TemplateManifest {
    pub kind: String,
    pub name: String,
    pub version: String,
    #[allow(dead_code)]
    pub description: Option<String>,
    pub template: TemplateMetadata,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
#[allow(dead_code)]
pub struct SkillCompatibility {
    pub model_families: Vec<String>,
    pub runtimes: Vec<String>,
    pub environments: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct SkillMetadata {
    pub entrypoint: String,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub scripts: Vec<String>,
    #[serde(default)]
    pub compatibility: SkillCompatibility,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SkillManifest {
    pub kind: String,
    pub name: String,
    pub version: String,
    #[allow(dead_code)]
    pub description: Option<String>,
    #[serde(default)]
    pub tools: Vec<PackageReference>,
    pub skill: SkillMetadata,
}

#[derive(Debug)]
pub enum PublishManifest {
    Tool(Box<ToolManifest>),
    Agent(Box<AgentManifest>),
    Template(Box<TemplateManifest>),
    Skill(Box<SkillManifest>),
}

/// Resolve the schema source (local file if present; else hosted URL)
pub fn resolve_schema_source(override_opt: Option<String>) -> String {
    if let Some(x) = override_opt {
        return x;
    }
    let local_path = PathBuf::from("schemas/agentpm.manifest.schema.json");
    if local_path.exists() {
        local_path.to_string_lossy().into_owned()
    } else {
        DEFAULT_MANIFEST_SCHEMA_URL.to_string()
    }
}

/// Load a JSON schema from a file or URL.
pub fn load_schema_value(source: &str) -> Result<Value> {
    if source == DEFAULT_MANIFEST_SCHEMA_URL {
        return Ok(serde_json::from_str(EMBEDDED_MANIFEST_SCHEMA_JSON)?);
    }
    if source.starts_with("http://") || source.starts_with("https://") {
        let resp = reqwest::blocking::get(source)
            .with_context(|| format!("fetching schema from {source}"))?;
        let text = resp.text()?;
        Ok(serde_json::from_str(&text)?)
    } else {
        let text = fs::read_to_string(source)
            .with_context(|| format!("reading schema from {}", source))?;
        Ok(serde_json::from_str(&text)?)
    }
}

/// Read and parse a manifest file.
pub fn load_manifest_value(path: &Path) -> Result<(Value, String)> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let value: Value = serde_json::from_str(&text)
        .with_context(|| format!("parsing JSON from {}", path.display()))?;
    Ok((value, text))
}

/// Validate a manifest `Value` against the schema and our semantic warnings.
/// If `fix == true`, this may mutate the `value` (e.g., auto-insert $schema).
/// Returns (ok, issues).
pub fn validate_manifest_value(
    schema_source: &str,
    file_label: &str,
    value: &mut Value,
    fix: bool,
) -> Result<(bool, Vec<LintIssue>)> {
    // Compile schema (keep simple for now; we can cache later if needed)
    let schema_value = load_schema_value(schema_source)?;
    let schema_static: &'static serde_json::Value = Box::leak(Box::new(schema_value));
    let compiled = JSONSchema::options()
        .with_draft(Draft::Draft202012)
        .compile(schema_static)?;

    let mut issues: Vec<LintIssue> = Vec::new();

    // Schema errors
    if let Err(errors) = compiled.validate(value) {
        for e in errors {
            issues.push(LintIssue {
                file: file_label.to_string(),
                level: "error",
                message: e.to_string(),
                instance_path: e.instance_path.to_string(),
                schema_path: e.schema_path.to_string(),
            });
        }
    }

    // Semantic warnings (mirror what `lint` had)
    if value.get("$schema").is_none() {
        issues.push(LintIssue {
            file: file_label.to_string(),
            level: "warning",
            message: "Missing $schema; editors may lack IntelliSense.".into(),
            instance_path: "".into(),
            schema_path: "".into(),
        });
        if fix && let Some(obj) = value.as_object_mut() {
            obj.insert("$schema".into(), Value::String(schema_source.to_string()));
        }
    }

    if let Some(Value::String(desc)) = value.get("description")
        && desc.trim().is_empty()
    {
        issues.push(LintIssue {
            file: file_label.to_string(),
            level: "warning",
            message: "`description` should not be empty".into(),
            instance_path: "/description".into(),
            schema_path: "".into(),
        });
    }

    if value.get("kind").and_then(Value::as_str) == Some("agent") {
        for field in ["knowledge", "memory", "profiles"] {
            if value
                .get(field)
                .and_then(Value::as_array)
                .map(|items| !items.is_empty())
                .unwrap_or(false)
            {
                issues.push(LintIssue {
                    file: file_label.to_string(),
                    level: "warning",
                    message: format!(
                        "`{field}` is validated and preserved, but not resolved in Phase 3."
                    ),
                    instance_path: format!("/{field}"),
                    schema_path: "".into(),
                });
            }
        }
    }

    if value.get("kind").and_then(Value::as_str) == Some("template")
        && let Some(Value::Object(template)) = value.get("template")
        && let Some(Value::Array(variables)) = template.get("variables")
    {
        let mut seen = HashSet::new();
        for (idx, variable) in variables.iter().enumerate() {
            if let Some(name) = variable.get("name").and_then(Value::as_str)
                && !seen.insert(name.to_string())
            {
                issues.push(LintIssue {
                    file: file_label.to_string(),
                    level: "error",
                    message: format!("duplicate template variable name `{name}`"),
                    instance_path: format!("/template/variables/{idx}/name"),
                    schema_path: "".into(),
                });
            }
        }
    }

    if let Some(Value::Object(runtime)) = value.get("runtime")
        && let Some(Value::String(runtime_type)) = runtime.get("type")
        && let Some(Value::Object(entrypoint)) = value.get("entrypoint")
    {
        let command = entrypoint
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let canon = canonical_interpreter(command);
        let runtime_interpreter = canonical_interpreter(runtime_type);

        if canon != runtime_interpreter && !is_interpreter_match(&runtime_interpreter, &canon) {
            issues.push(LintIssue {
                file: file_label.to_string(),
                level: "error",
                message: format!(
                    "`runtime.type` should match `entrypoint.command` ({} vs {})",
                    canon, runtime_interpreter
                ),
                instance_path: "/runtime/type".into(),
                schema_path: "".into(),
            });
        }
    }

    let has_error = issues.iter().any(|i| i.level == "error");
    Ok((!has_error, issues))
}

fn canonical_interpreter(cmd: &str) -> String {
    let base = cmd.to_lowercase();

    base.strip_suffix(".exe")
        .or_else(|| base.strip_suffix(".cmd"))
        .or_else(|| base.strip_suffix(".bat"))
        .unwrap_or(&base)
        .to_string()
}

fn is_interpreter_match(runtime: &str, command: &str) -> bool {
    if runtime == command {
        return true;
    }

    // map runtime -> acceptable command names
    let mut aliases: HashMap<&str, &[&str]> = HashMap::new();
    aliases.insert("python", &["python3"]);
    aliases.insert("node", &["nodejs"]);

    if let Some(cmds) = aliases.get(runtime) {
        cmds.contains(&command)
    } else {
        false
    }
}

/// Discover manifest files from CLI args (dirs/globs/files).
pub fn discover_manifest_files(paths: &[String]) -> Result<Vec<PathBuf>> {
    // Default: ./agent.json
    if paths.is_empty() {
        let p = PathBuf::from("agent.json");
        return Ok(if p.exists() { vec![p] } else { vec![] });
    }

    let mut out = Vec::new();
    for raw in paths {
        let p = PathBuf::from(raw);
        if p.is_dir() {
            let candidate = p.join("agent.json");
            if candidate.exists() {
                out.push(candidate);
            }
        } else if p.file_name().map(|f| f == "agent.json").unwrap_or(false) && p.exists() {
            out.push(p);
        } else {
            // Glob-ish: allow users to pass things like "**/agent.json"
            if let Ok(paths) = glob::glob(raw) {
                for entry in paths.flatten() {
                    if entry
                        .file_name()
                        .map(|f| f == "agent.json")
                        .unwrap_or(false)
                        && entry.exists()
                    {
                        out.push(entry);
                    }
                }
            }
        }
    }
    Ok(out)
}

/// Write back a (possibly fixed) manifest in pretty JSON.
pub fn write_manifest_pretty(path: &Path, value: &Value) -> Result<()> {
    let pretty = serde_json::to_string_pretty(value)?;
    fs::write(path, pretty + "\n")
        .with_context(|| format!("failed to write fixed file {}", path.display()))?;
    Ok(())
}

pub fn write_manifest_pretty_atomic(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    // Serialize with trailing newline
    let mut data = serde_json::to_vec_pretty(value)?;
    if !data.ends_with(b"\n") {
        data.push(b'\n');
    }

    // Write to .tmp then rename
    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp)
            .with_context(|| format!("opening temp file {}", tmp.display()))?;
        f.write_all(&data)
            .with_context(|| format!("writing {}", tmp.display()))?;
        let _ = f.sync_all(); // best-effort
    }

    // On Windows, replace existing file safely
    if path.exists() {
        let _ = fs::remove_file(path);
    }
    fs::rename(&tmp, path)
        .with_context(|| format!("renaming {} -> {}", tmp.display(), path.display()))?;

    // Best-effort fsync of the directory
    if let Some(parent) = path.parent()
        && let Ok(dirf) = fs::File::open(parent)
    {
        let _ = dirf.sync_all();
    }

    Ok(())
}

/// Parse the strongly typed Tool manifest; enforce kind = "tool" here if desired.
pub fn parse_tool_manifest(value: &Value) -> Result<ToolManifest> {
    let mf: ToolManifest =
        serde_json::from_value(value.clone()).context("parsing manifest into ToolManifest")?;
    if mf.kind != "tool" {
        return Err(anyhow!(format!(
            "`agentpm publish` currently supports only kind=\"tool\" (got kind=\"{}\")",
            mf.kind
        )));
    }
    Ok(mf)
}

pub fn parse_publish_manifest(value: &Value) -> Result<PublishManifest> {
    let kind = value
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("manifest must include kind"))?;

    match kind {
        "tool" => Ok(PublishManifest::Tool(Box::new(parse_tool_manifest(value)?))),
        "agent" => {
            let mf: AgentManifest = serde_json::from_value(value.clone())
                .context("parsing manifest into AgentManifest")?;
            Ok(PublishManifest::Agent(Box::new(mf)))
        }
        "template" => Ok(PublishManifest::Template(Box::new(
            parse_template_manifest(value)?,
        ))),
        "skill" => Ok(PublishManifest::Skill(Box::new(parse_skill_manifest(
            value,
        )?))),
        other => Err(anyhow!(format!(
            "`agentpm publish` supports kind=\"tool\", kind=\"agent\", kind=\"template\", and kind=\"skill\" manifests (got kind=\"{}\")",
            other
        ))),
    }
}

#[allow(dead_code)]
pub fn parse_template_manifest(value: &Value) -> Result<TemplateManifest> {
    let mf: TemplateManifest =
        serde_json::from_value(value.clone()).context("parsing manifest into TemplateManifest")?;
    if mf.kind != "template" {
        return Err(anyhow!(format!(
            "expected kind=\"template\" manifest (got kind=\"{}\")",
            mf.kind
        )));
    }
    Ok(mf)
}

pub fn parse_skill_manifest(value: &Value) -> Result<SkillManifest> {
    let mf: SkillManifest =
        serde_json::from_value(value.clone()).context("parsing manifest into SkillManifest")?;
    if mf.kind != "skill" {
        return Err(anyhow!(format!(
            "expected kind=\"skill\" manifest (got kind=\"{}\")",
            mf.kind
        )));
    }
    Ok(mf)
}

/// Lock files
pub fn write_lock<P: AsRef<Path>>(dir: P, lock: &Lock) -> Result<()> {
    let path = dir.as_ref().join("agent.lock");
    let v: Value = serde_json::to_value(lock)?; // convert to Value
    write_manifest_pretty_atomic(&path, &v) // reuse atomic writer
}

pub fn read_lock_or_default<P: AsRef<Path>>(dir: P) -> Result<Lock> {
    let path = dir.as_ref().join("agent.lock");
    if path.exists() {
        let data = fs::read(path)?;
        let lock: Lock = serde_json::from_slice(&data)?;
        Ok(lock)
    } else {
        Ok(Lock::empty_v2())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn schema_path() -> String {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../schemas/agentpm.manifest.schema.json")
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn load_schema_value_uses_embedded_default_schema_for_hosted_url() {
        let schema = load_schema_value(DEFAULT_MANIFEST_SCHEMA_URL).unwrap();
        assert_eq!(
            schema.get("$schema").and_then(Value::as_str),
            Some("https://json-schema.org/draft/2020-12/schema")
        );
        assert_eq!(
            schema
                .get("properties")
                .and_then(|props| props.get("kind"))
                .and_then(|kind| kind.get("enum"))
                .and_then(Value::as_array)
                .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>())
                .unwrap(),
            vec!["agent", "tool", "template", "skill"]
        );
    }

    fn assert_manifest_ok(mut manifest: Value) {
        let (ok, issues) =
            validate_manifest_value(&schema_path(), "agent.json", &mut manifest, false).unwrap();
        assert!(ok, "expected manifest to validate, got issues: {issues:#?}");
    }

    fn assert_manifest_invalid(mut manifest: Value) -> Vec<LintIssue> {
        let (ok, issues) =
            validate_manifest_value(&schema_path(), "agent.json", &mut manifest, false).unwrap();
        assert!(!ok, "expected manifest to fail validation");
        issues
    }

    fn temp_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("agentpm-{label}-{nanos}.json"))
    }

    #[test]
    fn valid_tool_manifest_validates() {
        assert_manifest_ok(json!({
            "kind": "tool",
            "name": "capitalize",
            "version": "0.1.0",
            "description": "Uppercase text.",
            "entrypoint": {
                "command": "python",
                "args": ["capitalize.py"]
            },
            "inputs": {
                "type": "object",
                "properties": {
                    "text": { "type": "string" }
                },
                "required": ["text"]
            },
            "outputs": {
                "type": "object",
                "properties": {
                    "upper": { "type": "string" }
                },
                "required": ["upper"]
            },
            "files": ["capitalize.py"],
            "runtime": {
                "type": "python",
                "version": "3.12"
            }
        }));
    }

    #[test]
    fn valid_agent_manifest_validates_with_examples_and_reserved_fields() {
        assert_manifest_ok(json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "description": "Triage support requests using installed tools.",
            "tools": [
                "@zack/slack-post-message@0.1.0",
                {
                    "name": "@zack/github-issues",
                    "version": "0.2.3"
                }
            ],
            "skills": ["@zack/support-triage-skill@0.1.0"],
            "knowledge": [
                {
                    "name": "@zack/internal-playbooks",
                    "version": "0.1.0"
                }
            ],
            "memory": [],
            "profiles": [],
            "examples": [
                {
                    "title": "Triage an incident",
                    "prompt": "Summarize this incident and draft a follow-up issue."
                }
            ]
        }));
    }

    #[test]
    fn valid_procedural_skill_manifest_validates_without_tools() {
        assert_manifest_ok(json!({
            "kind": "skill",
            "name": "incident-commander",
            "version": "0.1.0",
            "description": "Incident response coordination playbook.",
            "skill": {
                "entrypoint": "SKILL.md"
            }
        }));
    }

    #[test]
    fn valid_tool_backed_skill_manifest_validates() {
        assert_manifest_ok(json!({
            "kind": "skill",
            "name": "slack-incident-update",
            "version": "0.1.0",
            "description": "A playbook for posting structured incident updates to Slack.",
            "tools": [
                {
                    "name": "@zack/slack-post-message",
                    "version": "0.1.1"
                }
            ],
            "skill": {
                "entrypoint": "SKILL.md",
                "references": [
                    "references/tool-contract.md",
                    "references/examples.md"
                ],
                "scripts": ["scripts/run.sh"],
                "compatibility": {
                    "runtimes": ["agentpm-run", "shell"]
                }
            }
        }));
    }

    #[test]
    fn skill_manifest_rejects_unknown_compatibility_runtime() {
        let issues = assert_manifest_invalid(json!({
            "kind": "skill",
            "name": "invalid-compatibility-runtime",
            "version": "0.1.0",
            "description": "Unknown compatibility runtime should fail schema validation.",
            "skill": {
                "entrypoint": "SKILL.md",
                "compatibility": {
                    "runtimes": ["whatever"]
                }
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/skill/compatibility/runtimes/0"),
            "expected invalid compatibility runtime failure, got: {issues:#?}"
        );
    }

    #[test]
    fn valid_template_manifest_validates() {
        assert_manifest_ok(json!({
            "kind": "template",
            "name": "research-assistant-python",
            "version": "0.1.0",
            "description": "Python SDK starter for a local research assistant.",
            "template": {
                "display_name": "Python Research Assistant",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "stack": ["python"],
                "files_root": "template",
                "variables": [
                    {
                        "name": "project_name",
                        "description": "Generated project name. Do not use for secrets.",
                        "required": true,
                        "default": "research-assistant"
                    }
                ],
                "dependencies": {
                    "tools": [
                        {
                            "name": "@zack/web-page-extract",
                            "version": "0.1.2"
                        }
                    ],
                    "agents": []
                },
                "entrypoints": [
                    {
                        "label": "Run locally",
                        "command": "python main.py \"AgentPM\""
                    }
                ]
            }
        }));
    }

    #[test]
    fn valid_template_manifest_with_skill_dependencies_validates() {
        assert_manifest_ok(json!({
            "kind": "template",
            "name": "incident-response-workspace",
            "version": "0.1.0",
            "description": "Workspace starter with a first-class skill dependency.",
            "template": {
                "display_name": "Incident Response Workspace",
                "use_case": "incident-response",
                "execution_surfaces": ["multi-agent-workspace"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": [],
                    "skills": [
                        {
                            "name": "@zack/incident-commander",
                            "version": "0.1.0"
                        }
                    ]
                },
                "entrypoints": [
                    {
                        "label": "Open workspace",
                        "command": "agentpm run"
                    }
                ]
            }
        }));
    }

    #[test]
    fn invalid_tool_manifest_missing_single_required_tool_field_fails() {
        let issues = assert_manifest_invalid(json!({
            "kind": "tool",
            "name": "broken-tool",
            "version": "0.1.0",
            "description": "Missing one required tool-only field.",
            "entrypoint": {
                "command": "python",
                "args": ["capitalize.py"]
            },
            "inputs": {
                "type": "object"
            },
            "outputs": {
                "type": "object"
            }
        }));

        assert!(
            issues.iter().any(|issue| issue.schema_path == "/oneOf"),
            "expected oneOf failure caused by the remaining missing tool field, got: {issues:#?}"
        );
    }

    #[test]
    fn invalid_agent_manifest_with_tool_only_fields_fails() {
        let issues = assert_manifest_invalid(json!({
            "kind": "agent",
            "name": "misclassified-agent",
            "version": "0.1.0",
            "description": "Should not carry tool runtime contract.",
            "tools": ["@zack/slack-post-message@0.1.0"],
            "entrypoint": {
                "command": "node",
                "args": ["dist/index.js"]
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/kind" || issue.instance_path.is_empty()),
            "expected kind/oneOf validation error, got: {issues:#?}"
        );
    }

    #[test]
    fn agent_manifest_with_recursive_agents_dependencies_fails() {
        let issues = assert_manifest_invalid(json!({
            "kind": "agent",
            "name": "recursive-agent",
            "version": "0.1.0",
            "description": "Should not accept recursive agent dependencies.",
            "tools": ["@zack/web-page-extract@0.1.0"],
            "agents": ["@zack/another-agent@0.1.0"]
        }));

        assert!(
            issues.iter().any(|issue| issue
                .message
                .contains("Additional properties are not allowed")
                || issue.instance_path.is_empty()),
            "expected recursive agents dependency failure, got: {issues:#?}"
        );
    }

    #[test]
    fn reserved_agent_fields_validate_and_are_preserved_without_warning_on_skills() {
        let mut manifest = json!({
            "kind": "agent",
            "name": "preserved-agent",
            "version": "0.1.0",
            "description": "Reserved references should survive lint validation untouched.",
            "tools": ["@zack/slack-post-message@0.1.0"],
            "skills": ["@zack/support-triage-skill@0.1.0"],
            "knowledge": [{"name": "@zack/internal-playbooks", "version": "0.1.0"}],
            "memory": ["@zack/session-memory@0.1.0"],
            "profiles": ["@zack/escalation-profile@0.1.0"]
        });
        let original = manifest.clone();

        let (ok, issues) =
            validate_manifest_value(&schema_path(), "agent.json", &mut manifest, false).unwrap();

        assert!(ok, "expected manifest to validate, got issues: {issues:#?}");
        assert_eq!(
            manifest, original,
            "validation should preserve reserved fields"
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/knowledge"),
            "expected reserved-field warning for knowledge, got: {issues:#?}"
        );
        assert!(
            issues.iter().all(|issue| issue.instance_path != "/skills"),
            "skills should not emit a reserved-field warning, got: {issues:#?}"
        );
    }

    #[test]
    fn reserved_future_fields_do_not_replace_required_tools() {
        let issues = assert_manifest_invalid(json!({
            "kind": "agent",
            "name": "missing-tools-agent",
            "version": "0.1.0",
            "description": "Reserved refs alone must not imply installable tool dependencies.",
            "skills": ["@zack/support-triage-skill@0.1.0"],
            "knowledge": ["@zack/internal-playbooks@0.1.0"],
            "memory": [],
            "profiles": []
        }));

        assert!(
            issues.iter().any(|issue| issue.instance_path.is_empty()),
            "expected missing-tools validation error, got: {issues:#?}"
        );
    }

    #[test]
    fn template_manifest_rejects_invalid_variable_name() {
        let issues = assert_manifest_invalid(json!({
            "kind": "template",
            "name": "bad-variable-template",
            "version": "0.1.0",
            "description": "Invalid variable name should fail schema validation.",
            "template": {
                "display_name": "Bad Variable Template",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [
                    {
                        "name": "ProjectName",
                        "description": "Bad variable name.",
                        "required": true
                    }
                ],
                "dependencies": {
                    "tools": [],
                    "agents": []
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ]
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/template/variables/0/name"),
            "expected invalid template variable name error, got: {issues:#?}"
        );
    }

    #[test]
    fn template_manifest_rejects_invalid_stack_value() {
        let issues = assert_manifest_invalid(json!({
            "kind": "template",
            "name": "bad-stack-template",
            "version": "0.1.0",
            "description": "Invalid stack values should fail schema validation.",
            "template": {
                "display_name": "Bad Stack Template",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "stack": ["cobol"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": []
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ]
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/template/stack/0"),
            "expected invalid stack enum error, got: {issues:#?}"
        );
    }

    #[test]
    fn template_manifest_rejects_duplicate_variable_names() {
        let issues = assert_manifest_invalid(json!({
            "kind": "template",
            "name": "duplicate-variables-template",
            "version": "0.1.0",
            "description": "Duplicate variable names should fail validation.",
            "template": {
                "display_name": "Duplicate Variables Template",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [
                    {
                        "name": "project_name",
                        "description": "Project name.",
                        "required": true
                    },
                    {
                        "name": "project_name",
                        "description": "Duplicate project name.",
                        "required": false,
                        "default": "duplicate"
                    }
                ],
                "dependencies": {
                    "tools": [],
                    "agents": []
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ]
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.message.contains("duplicate template variable name")),
            "expected duplicate template variable name error, got: {issues:#?}"
        );
    }

    #[test]
    fn template_manifest_requires_template_object() {
        let issues = assert_manifest_invalid(json!({
            "kind": "template",
            "name": "missing-template-object",
            "version": "0.1.0",
            "description": "Missing template metadata should fail."
        }));

        assert!(
            issues.iter().any(|issue| issue.schema_path == "/oneOf"
                || issue.message.contains("required property")),
            "expected missing template object failure, got: {issues:#?}"
        );
    }

    #[test]
    fn template_manifest_rejects_missing_files_root() {
        let issues = assert_manifest_invalid(json!({
            "kind": "template",
            "name": "missing-files-root",
            "version": "0.1.0",
            "description": "Missing files_root should fail.",
            "template": {
                "display_name": "Missing Files Root",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": []
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ]
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.message.contains("files_root")),
            "expected missing files_root validation error, got: {issues:#?}"
        );
    }

    #[test]
    fn template_manifest_rejects_invalid_dependency_shape() {
        let issues = assert_manifest_invalid(json!({
            "kind": "template",
            "name": "invalid-dependency-shape",
            "version": "0.1.0",
            "description": "Template dependency refs must match packageRef shape.",
            "template": {
                "display_name": "Invalid Dependency Shape",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [
                        {
                            "version": "0.1.0"
                        }
                    ],
                    "agents": []
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ]
            }
        }));

        assert!(
            issues.iter().any(|issue| issue
                .instance_path
                .starts_with("/template/dependencies/tools/0")),
            "expected invalid dependency shape failure, got: {issues:#?}"
        );
    }

    #[test]
    fn skill_manifest_rejects_top_level_skill_dependencies() {
        let issues = assert_manifest_invalid(json!({
            "kind": "skill",
            "name": "recursive-skill",
            "version": "0.1.0",
            "description": "Should not accept skill-to-skill dependencies in Phase 6A.",
            "tools": [],
            "skills": ["@zack/other-skill@0.1.0"],
            "skill": {
                "entrypoint": "SKILL.md"
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/kind" || issue.instance_path.is_empty()),
            "expected top-level skills rejection for kind=skill, got: {issues:#?}"
        );
    }

    #[test]
    fn skill_manifest_rejects_missing_entrypoint() {
        let issues = assert_manifest_invalid(json!({
            "kind": "skill",
            "name": "missing-entrypoint",
            "version": "0.1.0",
            "description": "Missing skill.entrypoint should fail.",
            "skill": {}
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.message.contains("entrypoint")),
            "expected missing entrypoint validation error, got: {issues:#?}"
        );
    }

    #[test]
    fn skill_manifest_rejects_unsafe_entrypoint_path() {
        let issues = assert_manifest_invalid(json!({
            "kind": "skill",
            "name": "unsafe-entrypoint",
            "version": "0.1.0",
            "description": "Unsafe entrypoint path should fail.",
            "skill": {
                "entrypoint": "../SKILL.md"
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/skill/entrypoint"),
            "expected unsafe entrypoint path failure, got: {issues:#?}"
        );
    }

    #[test]
    fn skill_manifest_rejects_absolute_entrypoint_path() {
        let issues = assert_manifest_invalid(json!({
            "kind": "skill",
            "name": "absolute-entrypoint",
            "version": "0.1.0",
            "description": "Absolute entrypoint path should fail.",
            "skill": {
                "entrypoint": "/tmp/SKILL.md"
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/skill/entrypoint"),
            "expected absolute entrypoint path failure, got: {issues:#?}"
        );
    }

    #[test]
    fn skill_manifest_rejects_unsafe_reference_path() {
        let issues = assert_manifest_invalid(json!({
            "kind": "skill",
            "name": "unsafe-reference",
            "version": "0.1.0",
            "description": "Unsafe reference path should fail.",
            "skill": {
                "entrypoint": "SKILL.md",
                "references": ["references/../../secret.md"]
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/skill/references/0"),
            "expected unsafe reference path failure, got: {issues:#?}"
        );
    }

    #[test]
    fn skill_manifest_rejects_unsafe_script_path() {
        let issues = assert_manifest_invalid(json!({
            "kind": "skill",
            "name": "unsafe-script",
            "version": "0.1.0",
            "description": "Unsafe script path should fail.",
            "skill": {
                "entrypoint": "SKILL.md",
                "scripts": ["/tmp/run.sh"]
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/skill/scripts/0"),
            "expected unsafe script path failure, got: {issues:#?}"
        );
    }

    #[test]
    fn template_manifest_rejects_unsupported_extra_properties() {
        let issues = assert_manifest_invalid(json!({
            "kind": "template",
            "name": "extra-properties-template",
            "version": "0.1.0",
            "description": "Unsupported template properties should fail.",
            "template": {
                "display_name": "Extra Properties Template",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": []
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ],
                "hooks": ["pre_generate"]
            }
        }));

        assert!(
            issues.iter().any(|issue| issue
                .message
                .contains("Additional properties are not allowed")),
            "expected unsupported extra properties failure, got: {issues:#?}"
        );
    }

    #[test]
    fn agent_template_matches_phase_three_shape() {
        let rendered = include_str!("../assets/templates/agent.json.tpl")
            .replace("{{AGENT_NAME}}", "support-agent")
            .replace(
                "{{AGENT_DESCRIPTION}}",
                "Triage support requests using installed tools.",
            );
        let mut manifest: Value = serde_json::from_str(&rendered).unwrap();
        let (ok, issues) =
            validate_manifest_value(&schema_path(), "agent.json", &mut manifest, false).unwrap();

        assert!(
            ok,
            "expected rendered template to validate, got: {issues:#?}"
        );
        assert_eq!(manifest["kind"], "agent");
        assert_eq!(manifest["tools"], json!([]));
        assert_eq!(manifest["skills"], json!([]));
        assert_eq!(manifest["knowledge"], json!([]));
        assert_eq!(manifest["memory"], json!([]));
        assert_eq!(manifest["profiles"], json!([]));
        assert_eq!(manifest["examples"][0]["title"], "Example prompt");
        assert!(manifest.get("entrypoint").is_none());
    }

    #[test]
    fn lint_fix_can_add_schema_to_agent_template() {
        let path = temp_path("agent-template");
        let raw = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "description": "Triage support requests using installed tools.",
            "tools": [],
            "skills": [],
            "knowledge": [],
            "memory": [],
            "profiles": [],
            "examples": [
                {
                    "title": "Example prompt",
                    "prompt": "Describe the user request this agent should handle."
                }
            ]
        });

        write_manifest_pretty(&path, &raw).unwrap();
        let (mut manifest, _) = load_manifest_value(&path).unwrap();
        let (ok, issues) =
            validate_manifest_value(&schema_path(), "agent.json", &mut manifest, true).unwrap();

        assert!(
            ok,
            "expected lint fix validation to succeed, got: {issues:#?}"
        );
        assert_eq!(
            manifest.get("$schema").and_then(Value::as_str),
            Some(schema_path().as_str())
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn parse_publish_manifest_dispatches_tool_kind() {
        let manifest = json!({
            "kind": "tool",
            "name": "summarize",
            "version": "1.2.3",
            "description": "Summarize text.",
            "runtime": {"type": "python", "version": "3.11"},
            "entrypoint": {
                "command": "python",
                "args": ["main.py"]
            },
            "files": ["main.py"],
            "inputs": {"type": "object"},
            "outputs": {"type": "object"}
        });

        match parse_publish_manifest(&manifest).unwrap() {
            PublishManifest::Tool(mf) => {
                assert_eq!(mf.kind, "tool");
                assert_eq!(mf.name, "summarize");
            }
            PublishManifest::Agent(_)
            | PublishManifest::Template(_)
            | PublishManifest::Skill(_) => {
                panic!("expected tool publish manifest")
            }
        }
    }

    #[test]
    fn parse_publish_manifest_dispatches_agent_kind() {
        let manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "description": "Support agent.",
            "tools": ["@zack/slack-post-message@0.1.0"],
            "skills": [],
            "knowledge": [],
            "memory": [],
            "profiles": [],
            "examples": [{"title": "Example", "prompt": "Help the user."}]
        });

        match parse_publish_manifest(&manifest).unwrap() {
            PublishManifest::Agent(mf) => {
                assert_eq!(mf.kind, "agent");
                assert_eq!(mf.name, "support-agent");
            }
            PublishManifest::Tool(_) | PublishManifest::Template(_) | PublishManifest::Skill(_) => {
                panic!("expected agent publish manifest")
            }
        }
    }

    #[test]
    fn parse_publish_manifest_dispatches_skill_kind() {
        let manifest = json!({
            "kind": "skill",
            "name": "incident-commander",
            "version": "0.1.0",
            "description": "Incident response coordination playbook.",
            "tools": [
                {
                    "name": "@zack/slack-post-message",
                    "version": "0.1.1"
                }
            ],
            "skill": {
                "entrypoint": "SKILL.md",
                "references": ["references/tool-contract.md"],
                "scripts": ["scripts/run.sh"]
            }
        });

        match parse_publish_manifest(&manifest).unwrap() {
            PublishManifest::Skill(mf) => {
                assert_eq!(mf.kind, "skill");
                assert_eq!(mf.name, "incident-commander");
                assert_eq!(mf.skill.entrypoint, "SKILL.md");
                assert_eq!(mf.tools.len(), 1);
            }
            PublishManifest::Tool(_) | PublishManifest::Agent(_) | PublishManifest::Template(_) => {
                panic!("expected skill publish manifest")
            }
        }
    }

    #[test]
    fn parse_publish_manifest_dispatches_template_kind() {
        let manifest = json!({
            "kind": "template",
            "name": "research-assistant",
            "version": "0.1.0",
            "description": "Research starter template.",
            "template": {
                "display_name": "Research Assistant",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": []
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ]
            }
        });

        match parse_publish_manifest(&manifest).unwrap() {
            PublishManifest::Template(mf) => {
                assert_eq!(mf.kind, "template");
                assert_eq!(mf.name, "research-assistant");
                assert_eq!(mf.template.files_root, "template");
                assert!(mf.template.dependencies.skills.is_empty());
            }
            PublishManifest::Tool(_) | PublishManifest::Agent(_) | PublishManifest::Skill(_) => {
                panic!("expected template publish manifest")
            }
        }
    }

    #[test]
    fn parse_publish_manifest_rejects_unknown_kind() {
        let manifest = json!({
            "kind": "workflow",
            "name": "starter",
            "version": "0.1.0"
        });

        let err = parse_publish_manifest(&manifest).unwrap_err().to_string();
        assert!(
            err.contains(
                "supports kind=\"tool\", kind=\"agent\", kind=\"template\", and kind=\"skill\""
            ),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_template_manifest_accepts_template_kind() {
        let manifest = json!({
            "kind": "template",
            "name": "starter-template",
            "version": "0.1.0",
            "description": "Starter template.",
            "template": {
                "display_name": "Starter Template",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": []
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ]
            }
        });

        let parsed = parse_template_manifest(&manifest).unwrap();
        assert_eq!(parsed.kind, "template");
        assert_eq!(parsed.name, "starter-template");
        assert!(parsed.template.dependencies.skills.is_empty());
    }

    #[test]
    fn parse_template_manifest_preserves_skill_dependencies() {
        let manifest = json!({
            "kind": "template",
            "name": "starter-template",
            "version": "0.1.0",
            "description": "Starter template.",
            "template": {
                "display_name": "Starter Template",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": [],
                    "skills": [
                        {
                            "name": "@zack/incident-commander",
                            "version": "0.1.0"
                        }
                    ]
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ]
            }
        });

        let parsed = parse_template_manifest(&manifest).unwrap();
        assert_eq!(parsed.template.dependencies.skills.len(), 1);
        match &parsed.template.dependencies.skills[0] {
            PackageReference::Object { name, version } => {
                assert_eq!(name, "@zack/incident-commander");
                assert_eq!(version.as_deref(), Some("0.1.0"));
            }
            PackageReference::String(_) => panic!("expected object dependency reference"),
        }
    }
}
