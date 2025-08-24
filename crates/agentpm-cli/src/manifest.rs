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

    let has_error = issues.iter().any(|i| i.level == "error");
    Ok((!has_error, issues))
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
