//! Strict authoring and explicit historical inspection through the real CLI.
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "agentpm-authoring-validation-{:032x}",
            rand::random::<u128>()
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_agentpm"))
            .current_dir(&self.0)
            .env("AGENTPM_TELEMETRY", "0")
            .env("NO_COLOR", "1")
            .env("AGENTPM_TOKEN", "apm_fixture.only")
            .args(["--base-url", "http://127.0.0.1:1"])
            .args(args)
            .output()
            .unwrap()
    }
    fn write(&self, value: &Value) {
        fs::write(
            self.0.join("agent.json"),
            serde_json::to_vec_pretty(value).unwrap(),
        )
        .unwrap();
    }
    fn init(&self, kind: &str) -> Value {
        success(self.run(&["init", "--kind", kind]));
        read(&self.0.join("agent.json"))
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn read(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn success(output: Output) -> Output {
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}
fn report(output: &Output) -> Value {
    let reports: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "missing JSON report: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    assert_eq!(reports.as_array().unwrap().len(), 1);
    reports[0].clone()
}
fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../standards/agentpm/1.0.0/conformance")
}

fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for item in fs::read_dir(source).unwrap() {
        let item = item.unwrap();
        if item.file_type().unwrap().is_dir() {
            copy_tree(&item.path(), &target.join(item.file_name()));
        } else {
            fs::copy(item.path(), target.join(item.file_name())).unwrap();
        }
    }
}

#[test]
fn historical_installed_inspection_and_strict_explicit_lint_are_separate() {
    for kind in ["knowledge", "memory"] {
        let project = Project::new();
        success(project.run(&["init", "--kind", kind, "--out-dir", "source"]));
        let source = project.0.join("source");
        let installed = project
            .0
            .join(format!(".agentpm/{kind}/fixture/my-{kind}/0.1.0"));
        copy_tree(&source, &installed);
        let mut manifest = read(&installed.join("agent.json"));
        manifest.as_object_mut().unwrap().remove("standard");
        fs::write(
            installed.join("agent.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let package = format!("@fixture/my-{kind}");
        let key = format!("{kind}:{package}@0.1.0");
        fs::write(project.0.join("agent.lock"), serde_json::to_vec(&json!({
            "lockfile_version":2,"generated":"2026-11-16T00:00:00Z","roots":{},
            "packages": {key: {"kind":kind,"name":package,"version":"0.1.0","integrity":"historical-test"}}
        })).unwrap()).unwrap();
        let before = fs::read(installed.join("agent.json")).unwrap();
        for target in [
            &package,
            installed.to_str().unwrap(),
            installed.join("agent.json").to_str().unwrap(),
        ] {
            let output = success(project.run(&[kind, "inspect", target, "--json"]));
            let data: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert!(data.get("apds").is_none());
            assert_eq!(fs::read(installed.join("agent.json")).unwrap(), before);
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&installed, project.0.join("installed-link")).unwrap();
            success(project.run(&[kind, "inspect", "installed-link", "--json"]));
        }
        let path = installed.join("agent.json");
        assert_eq!(
            project.run(&["lint", path.to_str().unwrap()]).status.code(),
            Some(1)
        );
        let output = success(project.run(&[
            "lint",
            "--legacy",
            path.to_str().unwrap(),
            "--format",
            "json",
        ]));
        assert_eq!(report(&output)["compatibility"], "legacy-unverified");
        assert_eq!(fs::read(installed.join("agent.json")).unwrap(), before);
    }
}

#[test]
fn default_scope_does_not_claim_missing_package_or_dependency_context() {
    let project = Project::new();
    project.write(&read(
        &corpus().join("fixtures/packaged/knowledge-context-missing/agent.json"),
    ));
    let ordinary = success(project.run(&["lint", "--format", "json"]));
    assert_eq!(report(&ordinary)["apds"]["level"], "standalone");
    assert_eq!(report(&ordinary)["apds"]["runner_evaluated"], false);
    let packaged = project.run(&[
        "lint",
        "--apds",
        "packaged",
        "--package-root",
        ".",
        "--format",
        "json",
    ]);
    assert_eq!(packaged.status.code(), Some(1));
    assert_eq!(report(&packaged)["apds"]["status"], "nonconformant");
    let mut agent = read(&corpus().join("fixtures/valid/agent.json"));
    agent["tools"] = json!(["@fixture/tool@1.0.0"]);
    project.write(&agent);
    let resolved = project.run(&["lint", "--apds", "resolved", "--format", "json"]);
    assert_eq!(resolved.status.code(), Some(1));
    assert_eq!(report(&resolved)["apds"]["status"], "incomplete");
    assert!(
        !report(&resolved)["apds"]["unevaluated"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn authoring_diagnostics_match_human_and_machine_snapshots() {
    let project = Project::new();
    let mut value = project.init("agent");
    for (name, selector) in [
        ("missing-standard", None),
        (
            "unsupported-standard",
            Some(json!({"id":"agentpm","version":"2.0.0"})),
        ),
    ] {
        value.as_object_mut().unwrap().remove("standard");
        if let Some(selector) = selector {
            value["standard"] = selector;
        }
        project.write(&value);
        let pretty = project.run(&["lint"]);
        let json_output = project.run(&["lint", "--format", "json"]);
        let ndjson = project.run(&["lint", "--format", "ndjson"]);
        assert_eq!(pretty.status.code(), Some(1));
        assert_eq!(json_output.status.code(), Some(1));
        assert_eq!(ndjson.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&pretty.stderr).contains("Lint failed"));
        assert!(String::from_utf8_lossy(&json_output.stderr).contains("Lint failed"));
        let actual = report(&json_output);
        assert_eq!(
            serde_json::from_slice::<Value>(&ndjson.stdout).unwrap(),
            actual
        );
        let snapshots = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/authoring");
        assert_eq!(
            String::from_utf8(pretty.stdout).unwrap(),
            fs::read_to_string(snapshots.join(format!("{name}.pretty.txt"))).unwrap()
        );
        assert_eq!(
            actual,
            read(&snapshots.join(format!("{name}.snapshot.json")))
        );
    }
}

#[test]
fn normal_lint_uses_the_pinned_engine_for_kind_specific_rules() {
    for case in [
        "dependency-duplicate-identity",
        "loop-entry-unknown",
        "memory-scope-unknown",
        "template-variable-duplicate",
        "binding-undeclared",
        "tool-runtime-mismatch",
        "unknown-kind",
        "description-whitespace",
    ] {
        let project = Project::new();
        project.write(&read(
            &corpus().join(format!("fixtures/invalid/{case}.json")),
        ));
        let ordinary = project.run(&["lint", "--format", "json"]);
        let explicit = project.run(&["lint", "--apds", "standalone", "--format", "json"]);
        assert_eq!(ordinary.status.code(), Some(1), "{case}");
        let actual = report(&ordinary);
        let reference = report(&explicit);
        assert_eq!(
            actual["apds"]["status"], reference["apds"]["status"],
            "{case}"
        );
        for finding in reference["issues"].as_array().unwrap() {
            assert!(
                actual["issues"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|issue| issue["rule_id"] == finding["rule_id"]
                        && issue["instance_path"] == finding["instance_path"]),
                "{case}: {actual}"
            );
        }
    }
}

#[test]
fn every_new_kind_requires_a_standard_before_lint_or_publication() {
    for kind in [
        "agent",
        "tool",
        "skill",
        "knowledge",
        "memory",
        "profile",
        "loop",
        "template",
    ] {
        let project = Project::new();
        let mut value = project.init(kind);
        value.as_object_mut().unwrap().remove("standard");
        project.write(&value);
        let before = fs::read(project.0.join("agent.json")).unwrap();
        let lint = project.run(&["lint", "--format", "json"]);
        assert_eq!(lint.status.code(), Some(1));
        let actual = report(&lint);
        assert_eq!(actual["apds"]["status"], "nonconformant");
        assert!(
            actual["issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| issue["rule_id"] == "APDS-ID-001"
                    && issue["instance_path"] == "/standard"
                    && issue["message"].as_str().unwrap().contains("agentpm lint"))
        );
        let publish = project.run(&["publish", "--dry-run", "--quiet"]);
        assert_eq!(publish.status.code(), Some(1), "{kind}");
        let error = String::from_utf8_lossy(&publish.stderr);
        assert!(
            error.contains("APDS-ID-001") && error.contains("/standard"),
            "{kind}: {error}"
        );
        assert!(!project.0.join("target/agentpm").exists());
        assert_eq!(fs::read(project.0.join("agent.json")).unwrap(), before);
    }
}

#[test]
fn legacy_inspection_is_read_only_labelled_and_cannot_authorize_publish() {
    let project = Project::new();
    let mut value = project.init("agent");
    value.as_object_mut().unwrap().remove("standard");
    project.write(&value);
    let before = fs::read(project.0.join("agent.json")).unwrap();
    let legacy = success(project.run(&["lint", "--legacy", "--strict", "--format", "json"]));
    let actual = report(&legacy);
    assert_eq!(actual["compatibility"], "legacy-unverified");
    assert!(actual.get("apds").is_none());
    let pretty = success(project.run(&["lint", "--legacy"]));
    assert!(String::from_utf8_lossy(&pretty.stdout).contains("Legacy compatibility only"));
    assert!(
        !project
            .run(&["publish", "--dry-run", "--quiet"])
            .status
            .success()
    );
    for conflict in [
        vec!["lint", "--legacy", "--fix"],
        vec!["lint", "--legacy", "--apds", "standalone"],
        vec!["lint", "--legacy", "--schema", "anything.json"],
    ] {
        assert_eq!(project.run(&conflict).status.code(), Some(2));
    }
    assert_eq!(fs::read(project.0.join("agent.json")).unwrap(), before);
    value["description"] = json!(" ");
    project.write(&value);
    let invalid_legacy = project.run(&["lint", "--legacy", "--format", "json"]);
    assert_eq!(invalid_legacy.status.code(), Some(1));
    assert_eq!(
        report(&invalid_legacy)["compatibility"],
        "legacy-unverified"
    );
    assert_eq!(report(&invalid_legacy)["ok"], false);
    assert!(report(&invalid_legacy).get("apds").is_none());
    value["description"] = json!("Historical local definition");
    for selector in [Value::Null, json!({}), json!("agentpm")] {
        value["standard"] = selector;
        project.write(&value);
        let malformed = project.run(&["lint", "--legacy", "--format", "json"]);
        assert_eq!(malformed.status.code(), Some(1));
        assert!(report(&malformed).get("compatibility").is_none());
        assert_eq!(report(&malformed)["apds"]["status"], "nonconformant");
    }
    for selector in [
        json!({"id":"agentpm","version":"9.0.0"}),
        json!({"id":"other","version":"1.0.0"}),
    ] {
        value["standard"] = selector;
        project.write(&value);
        let output = project.run(&["lint", "--legacy", "--format", "json"]);
        assert_eq!(output.status.code(), Some(1));
        let actual = report(&output);
        assert_eq!(actual["apds"]["status"], "unsupported");
        assert!(actual.get("compatibility").is_none());
        assert!(
            actual["issues"][0]["message"]
                .as_str()
                .unwrap()
                .contains("This CLI supports")
        );
    }
    project.write(&json!([]));
    assert_eq!(project.run(&["lint", "--legacy"]).status.code(), Some(1));
}

#[test]
fn malformed_json_and_missing_inputs_have_nonzero_structured_reports() {
    for args in [
        vec!["lint", "--format", "json"],
        vec!["lint", "not-found.json", "--format", "json"],
    ] {
        let project = Project::new();
        let output = project.run(&args);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(report(&output)["ok"], false);
        assert!(String::from_utf8_lossy(&output.stderr).contains("Lint failed"));
    }
    let project = Project::new();
    fs::write(project.0.join("custom.agent.json"), "{broken").unwrap();
    let output = project.run(&["lint", "custom.agent.json", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(report(&output)["file"], "custom.agent.json");
    assert!(
        report(&output)["issues"][0]["message"]
            .as_str()
            .unwrap()
            .contains("Failed to parse JSON")
    );
    let mut value = read(&corpus().join("fixtures/valid/agent.json"));
    value["$schema"] = json!(
        "https://raw.githubusercontent.com/agentpm-dev/cli/refs/tags/apds-v1.0.0/standards/agentpm/1.0.0/manifest.schema.json"
    );
    fs::write(
        project.0.join("custom.agent.json"),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
    success(project.run(&["lint", "custom.agent.json", "--strict"]));
    let mixed = project.run(&[
        "lint",
        "custom.agent.json",
        "missing.json",
        "--format",
        "json",
    ]);
    assert_eq!(mixed.status.code(), Some(1));
    let reports: Value = serde_json::from_slice(&mixed.stdout).unwrap();
    assert_eq!(reports.as_array().unwrap().len(), 2);
}

#[test]
fn fix_does_not_invent_declarations_or_rewrite_noop_and_unsupported_input() {
    let project = Project::new();
    let mut value = project.init("agent");
    for variant in ["valid", "missing", "unsupported"] {
        if variant == "missing" {
            value.as_object_mut().unwrap().remove("standard");
        }
        if variant == "unsupported" {
            value["standard"] = json!({"id":"agentpm","version":"99.0.0"});
        }
        project.write(&value);
        let before = fs::read(project.0.join("agent.json")).unwrap();
        let result = project.run(&["lint", "--fix"]);
        assert_eq!(result.status.success(), variant == "valid");
        assert_eq!(fs::read(project.0.join("agent.json")).unwrap(), before);
    }
    value["standard"] = json!({"id":"agentpm","version":"1.0.0"});
    value.as_object_mut().unwrap().remove("$schema");
    project.write(&value);
    success(project.run(&["lint", "--fix"]));
    assert!(
        read(&project.0.join("agent.json"))["$schema"]
            .as_str()
            .unwrap()
            .contains("apds-v1.0.0")
    );
    success(project.run(&["lint", "--strict"]));
}

#[test]
fn local_content_commands_reject_before_building_or_inspecting() {
    for kind in ["knowledge", "memory"] {
        let project = Project::new();
        let mut value = project.init(kind);
        for selector in [Value::Null, json!({"id":"agentpm","version":"99.0.0"})] {
            if selector.is_null() {
                value.as_object_mut().unwrap().remove("standard");
            } else {
                value["standard"] = selector;
            }
            project.write(&value);
            let before = fs::read(project.0.join("agent.json")).unwrap();
            for command in [vec![kind, "build"], vec![kind, "inspect", ".", "--json"]] {
                let output = project.run(&command);
                assert_eq!(output.status.code(), Some(1));
                assert!(String::from_utf8_lossy(&output.stderr).contains("APDS-ID-001"));
            }
            assert_eq!(fs::read(project.0.join("agent.json")).unwrap(), before);
            assert!(!project.0.join("memory/build.json").exists());
        }
    }
}
