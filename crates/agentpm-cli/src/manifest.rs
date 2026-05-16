use crate::semver::types::Lock;
use anyhow::{Context, Result, anyhow};
use jsonschema::{Draft, JSONSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

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

#[derive(Debug)]
pub enum PublishManifest {
    Tool(Box<ToolManifest>),
    Agent(Box<AgentManifest>),
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
        "https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json".to_string()
    }
}

/// Load a JSON schema from a file or URL.
pub fn load_schema_value(source: &str) -> Result<Value> {
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
        for field in ["skills", "knowledge", "memory", "profiles"] {
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
        other => Err(anyhow!(format!(
            "`agentpm publish` supports kind=\"tool\" and kind=\"agent\" (got kind=\"{}\")",
            other
        ))),
    }
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
        Ok(Lock {
            lockfile_version: 1,
            generated: chrono::Utc::now(), // or to_rfc3339() if using String
            dependencies: Default::default(),
        })
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
    fn reserved_future_fields_validate_and_are_preserved() {
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
            issues.iter().any(|issue| issue.instance_path == "/skills"),
            "expected reserved-field warning, got: {issues:#?}"
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
            PublishManifest::Agent(_) => panic!("expected tool publish manifest"),
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
            PublishManifest::Tool(_) => panic!("expected agent publish manifest"),
        }
    }

    #[test]
    fn parse_publish_manifest_rejects_unknown_kind() {
        let manifest = json!({
            "kind": "template",
            "name": "starter",
            "version": "0.1.0"
        });

        let err = parse_publish_manifest(&manifest).unwrap_err().to_string();
        assert!(
            err.contains("supports kind=\"tool\" and kind=\"agent\""),
            "unexpected error: {err}"
        );
    }
}
