use jsonschema::{Draft, JSONSchema};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

const PIN: &str = "https://raw.githubusercontent.com/agentpm-dev/cli/refs/tags/apds-v1.0.0/standards/agentpm/1.0.0/manifest.schema.json";
const SCHEMA: &str = include_str!("../../../standards/agentpm/1.0.0/manifest.schema.json");
static NEXT: AtomicU64 = AtomicU64::new(0);

fn bundle() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../standards/agentpm/1.0.0")
}

fn minimal() -> Value {
    serde_json::from_str(include_str!(
        "../../../standards/agentpm/1.0.0/conformance/fixtures/valid/minimal-agent.json"
    ))
    .unwrap()
}

fn validator() -> JSONSchema {
    JSONSchema::options()
        .with_draft(Draft::Draft202012)
        .compile(&serde_json::from_str(SCHEMA).unwrap())
        .unwrap()
}

#[test]
fn canonical_schema_validates_all_kinds_and_retains_stage_one_metadata() {
    let validator = validator();
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();
    assert_eq!(schema["$id"], PIN);
    let mut count = 0;
    for path in fs::read_dir(bundle().join("conformance/fixtures/valid")).unwrap() {
        let path = path.unwrap().path();
        let value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(validator.is_valid(&value), "{}", path.display());
        count += 1;
    }
    assert_eq!(count, 9);
    for field in ["standard", "description"] {
        let mut invalid = minimal();
        invalid.as_object_mut().unwrap().remove(field);
        assert!(!validator.is_valid(&invalid));
    }
    for standard in [
        json!(null),
        json!("agentpm/1.0.0"),
        json!({}),
        json!({"id":"other", "version":"1.0.0"}),
        json!({"id":"agentpm", "version":"2.0.0"}),
        json!({"id":"agentpm", "version":"1.0.0", "extra":true}),
    ] {
        let mut value = minimal();
        value["standard"] = standard;
        assert!(!validator.is_valid(&value));
    }
    let mut value = minimal();
    value["name"] = json!("@example/name");
    assert!(!validator.is_valid(&value));
    value = minimal();
    value["standrad"] = json!({});
    assert!(!validator.is_valid(&value));
    let tool: Value = serde_json::from_slice(
        &fs::read(bundle().join("conformance/fixtures/valid/tool.json")).unwrap(),
    )
    .unwrap();
    for field in ["entrypoint", "files", "inputs", "outputs"] {
        let mut value = tool.clone();
        value.as_object_mut().unwrap().remove(field);
        assert!(!validator.is_valid(&value), "Tool must require {field}");
    }
    let mut node = tool;
    node["runtime"]["type"] = json!("node");
    assert!(!validator.is_valid(&node));
}

#[test]
fn description_uses_the_same_explicit_whitespace_cases_as_python() {
    let validator = validator();
    let cases: Value = serde_json::from_str(include_str!(
        "../../../standards/agentpm/1.0.0/conformance/structural-cases.json"
    ))
    .unwrap();
    for case in cases["description_cases"].as_array().unwrap() {
        let mut value = minimal();
        value["description"] = case["value"].clone();
        assert_eq!(
            validator.is_valid(&value),
            case["valid"].as_bool().unwrap(),
            "{case}"
        );
    }
}

struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "agentpm-apds-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("schemas")).unwrap();
        for name in ["config", "cache", "roaming", "local"] {
            fs::create_dir_all(root.join(name)).unwrap();
        }
        let workspace = Self(root);
        workspace.write(minimal());
        fs::write(
            workspace.0.join("schemas/agentpm.manifest.schema.json"),
            "{}",
        )
        .unwrap();
        workspace
    }

    fn write(&self, value: Value) {
        fs::write(
            self.0.join("agent.json"),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_agentpm"))
            .current_dir(&self.0)
            .env("AGENTPM_TELEMETRY", "0")
            .env("AGENTPM_BASE_URL", "http://127.0.0.1:1")
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .env("XDG_CACHE_HOME", self.0.join("cache"))
            .env("APPDATA", self.0.join("roaming"))
            .env("LOCALAPPDATA", self.0.join("local"))
            .env("HTTP_PROXY", "http://127.0.0.1:1")
            .env("HTTPS_PROXY", "http://127.0.0.1:1")
            .args(args)
            .output()
            .unwrap()
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn offline_default_and_legacy_editor_uri_ignore_permissive_cwd_schema() {
    let workspace = Workspace::new();
    let mut value = minimal();
    value["$schema"] = json!(
        "https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json"
    );
    workspace.write(value.clone());
    let output = workspace.run(&["lint", "--strict", "--format", "json"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    value["name"] = json!("@example/name");
    workspace.write(value);
    let output = workspace.run(&["lint", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("/name"));
}

#[test]
fn development_override_is_labeled_and_cannot_replace_declared_apds_or_publish() {
    let workspace = Workspace::new();
    let mut legacy = minimal();
    legacy.as_object_mut().unwrap().remove("standard");
    legacy["$schema"] = json!(PIN);
    legacy["future_extension"] = json!({"opaque":true});
    workspace.write(legacy);
    let args = [
        "lint",
        "--schema",
        "schemas/agentpm.manifest.schema.json",
        "--format",
        "json",
    ];
    let output = workspace.run(&args);
    assert_eq!(
        output.status.code(),
        Some(1),
        "A development schema must not bypass the required declaration or unknown-field checks"
    );
    let reports: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        reports[0]["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["rule_id"] == "schema.override"
                && issue["message"]
                    .as_str()
                    .unwrap()
                    .contains("non-authoritative"))
    );
    assert!(
        !workspace
            .run(&[
                "lint",
                "--strict",
                "--schema",
                "schemas/agentpm.manifest.schema.json"
            ])
            .status
            .success()
    );
    let mut declared = minimal();
    declared["name"] = json!("@example/name");
    workspace.write(declared);
    assert!(!workspace.run(&args).status.success());
    let publish = workspace.run(&["publish", "--dry-run", "--token", "fixture-only"]);
    assert!(!publish.status.success());
    assert!(String::from_utf8_lossy(&publish.stderr).contains("Manifest validation failed"));
    assert!(!workspace.0.join("target/agentpm").exists());
    for source in [
        "schemas/agentpm.manifest.schema.json",
        "http://127.0.0.1:1/schema",
        "https://example.invalid/schema",
    ] {
        let output = workspace.run(&[
            "publish",
            "--dry-run",
            "--token",
            "fixture-only",
            "--schema",
            source,
        ]);
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Schema overrides cannot be used")
        );
        let package = workspace.run(&["package", "--schema", source]);
        assert!(!package.status.success());
        assert!(
            String::from_utf8_lossy(&package.stderr).contains("Schema overrides cannot be used")
        );
        let fan_in = workspace.run(&[
            "publish",
            "--build",
            "missing-build.json",
            "--schema",
            source,
        ]);
        assert!(!fan_in.status.success());
        assert!(
            String::from_utf8_lossy(&fan_in.stderr).contains("Schema overrides cannot be used")
        );
    }
    let output = workspace.run(&["lint", "--schema", "https://example.invalid/schema"]);
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Remote schema overrides are not supported")
    );
}

#[test]
fn blank_description_has_schema_errors_without_the_legacy_warning() {
    let workspace = Workspace::new();
    let mut value = minimal();
    value["description"] = json!("\u{00a0}\t\u{3000}");
    workspace.write(value);
    let output = workspace.run(&["lint", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    let reports: Value = serde_json::from_slice(&output.stdout).unwrap();
    let description: Vec<_> = reports[0]["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|issue| issue["instance_path"] == "/description")
        .collect();
    assert!(!description.is_empty());
    assert!(description.iter().all(|issue| {
        issue["level"] == "error"
            && issue["schema_path"]
                .as_str()
                .unwrap()
                .starts_with("/properties/description/")
    }));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("should not be empty"));
}
