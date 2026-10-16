use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "agentpm-authoring-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_agentpm"))
            .current_dir(&self.0)
            .env("NO_COLOR", "1")
            .args(args)
            .output()
            .unwrap()
    }

    fn manifest(&self) -> Value {
        serde_json::from_slice(&fs::read(self.0.join("agent.json")).unwrap()).unwrap()
    }

    fn write(&self, value: &Value) {
        fs::write(
            self.0.join("agent.json"),
            serde_json::to_vec_pretty(value).unwrap(),
        )
        .unwrap();
    }

    fn init(&self, kind: &str) {
        success(self.cli(&["init", "--kind", kind, "--name", "starter"]));
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn success(output: Output) -> Output {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn all_eight_init_commands_lint_clean_and_accept_only_the_standard_root_extension() {
    for kind in [
        "tool",
        "agent",
        "template",
        "skill",
        "knowledge",
        "memory",
        "profile",
        "loop",
    ] {
        let project = Project::new();
        project.init(kind);
        let output = success(project.cli(&["lint", "--strict", "--format", "json"]));
        let reports: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(reports[0]["issues"], json!([]), "{kind}: {reports}");
        let original = project.manifest();
        assert!(
            original.get("standard").is_none(),
            "init must not emit standard"
        );
        for standard in [
            json!({"future":"value"}),
            json!("unrecognized"),
            json!(null),
            json!(true),
            json!([1, 2]),
            json!(42),
        ] {
            let mut value = original.clone();
            value["standard"] = standard;
            project.write(&value);
            let output = success(project.cli(&["lint", "--strict", "--format", "json"]));
            let reports: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(reports[0]["issues"], json!([]), "{kind}: {reports}");
        }
        for typo in ["standrad", "tool"] {
            let mut value = original.clone();
            value[typo] = json!({});
            project.write(&value);
            let output = project.cli(&["lint", "--format", "json"]);
            assert_eq!(output.status.code(), Some(1), "{kind} accepted {typo}");
            let reports: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert!(
                reports[0]["issues"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|issue| issue["schema_path"] == "/additionalProperties")
            );
        }
    }
}

#[test]
fn tool_init_creates_a_runnable_minimal_json_echo_and_packages_its_source() {
    let project = Project::new();
    project.init("tool");
    let manifest = project.manifest();
    assert_eq!(manifest["runtime"], json!({"type":"python", "version":"3"}));
    assert_eq!(manifest["files"], json!(["main.py"]));
    assert_eq!(manifest["entrypoint"]["command"], "python3");
    assert_eq!(manifest["entrypoint"]["args"], json!(["main.py"]));
    let python = ["python3", "python"]
        .into_iter()
        .find(|command| {
            Command::new(command)
                .arg("--version")
                .output()
                .is_ok_and(|output| output.status.success())
        })
        .expect("Python required for scaffold execution fixture");
    let mut child = Command::new(python)
        .arg("main.py")
        .current_dir(&project.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let request = json!({"message":"hello", "nested":{"count":2}});
    child
        .stdin
        .take()
        .unwrap()
        .write_all(serde_json::to_string(&request).unwrap().as_bytes())
        .unwrap();
    let result = success(child.wait_with_output().unwrap());
    assert_eq!(
        serde_json::from_slice::<Value>(&result.stdout).unwrap(),
        request
    );
    success(project.cli(&[
        "publish",
        "--dry-run",
        "--token",
        "fixture-token",
        "--quiet",
    ]));
    assert!(
        project
            .0
            .join("target/agentpm/starter-0.1.0-python3.tar.gz")
            .is_file()
    );
}

#[test]
fn tool_init_keeps_existing_source_content() {
    let project = Project::new();
    fs::write(project.0.join("main.py"), "# existing application code\n").unwrap();
    project.init("tool");
    assert_eq!(
        fs::read_to_string(project.0.join("main.py")).unwrap(),
        "# existing application code\n"
    );
}

#[test]
fn lint_and_publish_share_human_context_ordering_and_preserve_machine_report_fields() {
    let project = Project::new();
    project.write(&serde_json::from_str(include_str!("fixtures/lint/invalid-loop.json")).unwrap());
    let pretty = project.cli(&["lint"]);
    assert_eq!(pretty.status.code(), Some(1));
    let human = String::from_utf8(pretty.stdout).unwrap();
    assert!(human.starts_with("✗ agent.json\n"));
    assert!(human.contains("at instance /loop/transitions/0/to"));
    assert!(!human.contains("\"phases\":"), "{human}");
    let publish = project.cli(&[
        "publish",
        "--dry-run",
        "--token",
        "fixture-token",
        "--quiet",
    ]);
    assert_eq!(publish.status.code(), Some(1));
    let stderr = String::from_utf8(publish.stderr).unwrap();
    assert!(
        stderr.contains(&human),
        "publish did not reuse lint's contextual report:\n{stderr}\nexpected:\n{human}"
    );
    for format in ["json", "ndjson"] {
        let output = project.cli(&["lint", "--format", format]);
        assert_eq!(output.status.code(), Some(1));
        let json: Value = serde_json::from_slice(&output.stdout).unwrap();
        let report = if format == "json" { &json[0] } else { &json };
        assert_eq!(report.as_object().unwrap().len(), 3);
        assert_eq!(report["file"], "agent.json");
        assert_eq!(report["ok"], false);
        for issue in report["issues"].as_array().unwrap() {
            assert_eq!(
                issue.as_object().unwrap().len(),
                5,
                "unexpected machine field: {issue}"
            );
            for key in ["file", "level", "message", "instance_path", "schema_path"] {
                assert!(issue[key].is_string(), "missing {key}: {issue}");
            }
        }
    }
}

#[test]
fn missing_literal_tool_files_block_publish_without_a_partial_artifact() {
    for missing in ["missing.txt", "missing-dir", "*.py"] {
        let project = Project::new();
        project.init("tool");
        let mut manifest = project.manifest();
        manifest["files"] = json!(["main.py", missing]);
        project.write(&manifest);
        let output = project.cli(&[
            "publish",
            "--dry-run",
            "--token",
            "fixture-token",
            "--quiet",
        ]);
        assert_eq!(output.status.code(), Some(1));
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains("manifest.files entry") && error.contains(missing),
            "{error}"
        );
        assert!(
            !project
                .0
                .join("target/agentpm/starter-0.1.0-python3.tar.gz")
                .exists()
        );
    }
}

#[test]
fn knowledge_write_preserves_standard_and_unknown_typed_fields_under_a_local_schema() {
    let project = Project::new();
    project.init("knowledge");
    let mut value = project.manifest();
    value["standard"] = json!({"name":"future-standard", "opaque":{"version":7}});
    project.write(&value);
    success(project.cli(&["knowledge", "build"]));
    assert_eq!(project.manifest()["standard"], value["standard"]);
    value = project.manifest();
    value["future_extension"] = json!({"opaque":[3, 2, 1]});
    project.write(&value);
    assert_eq!(
        project.cli(&["lint"]).status.code(),
        Some(1),
        "bundled schema must remain closed"
    );
    // A documented local schema override allows testing forward fields that the
    // typed Knowledge manifest does not know, without relaxing the bundled root.
    let mut schema: Value = serde_json::from_str(include_str!(
        "../../../schemas/agentpm.manifest.schema.json"
    ))
    .unwrap();
    schema["properties"]["future_extension"] = json!({});
    fs::create_dir(project.0.join("schemas")).unwrap();
    fs::write(
        project.0.join("schemas/agentpm.manifest.schema.json"),
        serde_json::to_vec(&schema).unwrap(),
    )
    .unwrap();
    success(project.cli(&["lint", "--strict"]));
    success(project.cli(&["knowledge", "build"]));
    let written = project.manifest();
    assert_eq!(written["standard"], value["standard"]);
    assert_eq!(written["future_extension"], value["future_extension"]);
    assert_eq!(written["knowledge"]["context"]["document_count"], 1);
}
