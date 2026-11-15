//! Run the actual Rust CLI against the shared corpus. Expectations stay in the bundle.
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "agentpm-apds-validation-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn run(&self, args: &[String]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_agentpm"));
        command
            .current_dir(&self.0)
            .args(args)
            .env("AGENTPM_TELEMETRY", "0")
            .env("AGENTPM_BASE_URL", "http://127.0.0.1:1");
        for key in [
            "XDG_CONFIG_HOME",
            "XDG_CACHE_HOME",
            "APPDATA",
            "LOCALAPPDATA",
        ] {
            command.env(key, self.0.join(key));
        }
        command
            .env("HTTP_PROXY", "http://127.0.0.1:1")
            .env("HTTPS_PROXY", "http://127.0.0.1:1");
        command.output().unwrap()
    }
    fn write(&self, name: &str, value: &Value) -> PathBuf {
        let path = self.0.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
        path
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../standards/agentpm/1.0.0/conformance")
        .canonicalize()
        .unwrap()
}
fn read(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn args(level: &str, path: &Path) -> Vec<String> {
    vec![
        "lint".into(),
        "--apds".into(),
        level.into(),
        "--format".into(),
        "json".into(),
        path.to_string_lossy().into_owned(),
    ]
}
fn report(output: &Output) -> Value {
    serde_json::from_slice::<Value>(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "Missing actual CLI JSON report: {e}\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })[0]
        .clone()
}

#[test]
fn shared_corpus_matches_actual_rust_validation() {
    let workspace = Workspace::new();
    let directory = directory();
    let index = read(&directory.join("fixture-index.json"));
    let mut rows = Vec::new();
    let mut failures = Vec::new();
    for case in index["cases"].as_array().unwrap() {
        let id = case["case_id"].as_str().unwrap();
        let level = case["level"].as_str().unwrap();
        let root = directory.join(case["root"].as_str().unwrap());
        let mut arguments = args(level, &root);
        if level == "resolved"
            && let Some(graph) = case.get("graph")
        {
            let mut graph = graph.clone();
            for node in graph["artifacts"].as_array_mut().unwrap() {
                node["manifest"] = json!(directory.join(node["manifest"].as_str().unwrap()));
            }
            let path = workspace.write("selected-graph.json", &graph);
            arguments.extend(["--graph".into(), path.to_string_lossy().into_owned()]);
        }
        if case["package"]["availability"] == "complete" {
            arguments.extend([
                "--package-root".into(),
                directory
                    .join(case["package"]["root"].as_str().unwrap())
                    .to_string_lossy()
                    .into_owned(),
            ]);
        }
        let output = workspace.run(&arguments);
        let actual = report(&output);
        let expected = &case["expected"];
        if actual["apds"]["status"] != expected["status"]
            || output.status.success() != (expected["status"] == "conformant")
            || actual["apds"]["unevaluated"] != expected["unevaluated"]
            || actual["apds"]["level"] != case["level"]
        {
            failures.push(format!("{id}: unexpected evaluation\n{actual}"));
        }
        let mut findings = Vec::new();
        // The public comparator accepts focused projections of native diagnostics.
        // Check every required finding against actual output; never manufacture one.
        for finding in expected["findings"].as_array().unwrap() {
            let wanted = directory
                .join(finding["document"].as_str().unwrap())
                .canonicalize()
                .unwrap();
            let matched = actual["issues"].as_array().unwrap().iter().find(|i| {
                i["rule_id"] == finding["rule_id"]
                    && i["instance_path"] == finding["path"]
                    && i["level"] == finding["severity"]
                    && Path::new(i["file"].as_str().unwrap()).canonicalize().ok()
                        == Some(wanted.clone())
            });
            if let Some(issue) = matched {
                let path = Path::new(issue["file"].as_str().unwrap())
                    .canonicalize()
                    .unwrap();
                findings.push(json!({"rule_id":issue["rule_id"],"path":issue["instance_path"],"severity":issue["level"],"document":path.strip_prefix(&directory).unwrap().to_string_lossy().replace('\\',"/")}));
            } else {
                failures.push(format!(
                    "{id}: expected finding {finding} absent from actual report\n{actual}"
                ));
            }
        }
        rows.push(json!({"case_id":id,"level":actual["apds"]["level"],"status":actual["apds"]["status"],"findings":findings,"unevaluated":actual["apds"]["unevaluated"]}));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    if let Some(path) = std::env::var_os("AGENTPM_APDS_RESULTS") {
        let path = PathBuf::from(path);
        let path = if path.is_relative() {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(path)
        } else {
            path
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
    }
    println!(
        "{} shared cases evaluated by the actual Rust CLI; no Runner behavior executed.",
        rows.len()
    );
}

#[test]
fn context_scope_distinguishes_missing_evidence_and_missing_content() {
    let workspace = Workspace::new();
    let root = directory().join("fixtures/packaged/knowledge-context-missing/agent.json");
    let plain = workspace.run(&["lint".into(), root.to_string_lossy().into_owned()]);
    assert!(
        plain.status.success(),
        "M3B owns ordinary authoring rollout"
    );
    let standalone = report(&workspace.run(&args("standalone", &root)));
    assert_eq!(standalone["apds"]["status"], "conformant");
    let unavailable = report(&workspace.run(&args("packaged", &root)));
    assert_eq!(unavailable["apds"]["status"], "incomplete");
    let mut complete = args("packaged", &root);
    complete.extend([
        "--package-root".into(),
        root.parent().unwrap().to_string_lossy().into_owned(),
    ]);
    let output = workspace.run(&complete);
    assert!(!output.status.success());
    assert_eq!(report(&output)["apds"]["status"], "nonconformant");
}

#[test]
fn offline_pinned_validation_cannot_be_forged_by_schema_overrides() {
    let workspace = Workspace::new();
    workspace.write("schemas/agentpm.manifest.schema.json", &json!({}));
    let mut value = read(&directory().join("fixtures/valid/minimal-agent.json"));
    value["description"] = json!("   ");
    let root = workspace.write("manifest.json", &value);
    let output = workspace.run(&args("standalone", &root));
    assert!(!output.status.success());
    assert!(
        report(&output)["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["instance_path"] == "/description" && i["rule_id"] == "APDS-ID-002")
    );
    for source in [
        "schemas/agentpm.manifest.schema.json",
        "https://example.invalid/schema",
    ] {
        let mut command = args("standalone", &root);
        command.extend(["--schema".into(), source.into()]);
        assert!(!workspace.run(&command).status.success());
    }
    value["standard"] = json!({"id":"agentpm","version":"999.0.0"});
    let root = workspace.write("manifest.json", &value);
    let unsupported = report(&workspace.run(&args("standalone", &root)));
    assert_eq!(unsupported["apds"]["status"], "unsupported");
    assert_eq!(unsupported["issues"], json!([]));
}

#[test]
fn explicit_files_require_real_reports_and_diagnostics_are_deterministic() {
    let workspace = Workspace::new();
    let root = workspace.write(
        "not-agent-name.json",
        &read(&directory().join("fixtures/valid/minimal-agent.json")),
    );
    let output = workspace.run(&args("standalone", &root));
    assert!(output.status.success());
    assert_eq!(report(&output)["apds"]["status"], "conformant");
    assert!(
        !workspace
            .run(&args("standalone", &workspace.0.join("missing.json")))
            .status
            .success()
    );
    let invalid = directory().join("fixtures/invalid/loop-unreachable.json");
    let first = workspace.run(&args("standalone", &invalid));
    for _ in 0..3 {
        assert_eq!(
            workspace.run(&args("standalone", &invalid)).stdout,
            first.stdout
        );
    }
}

#[test]
fn source_schema_references_are_offline_and_governance_visits_only_schema_locations() {
    let workspace = Workspace::new();
    let mut manifest = read(&directory().join("fixtures/valid/memory.json"));
    manifest.as_object_mut().unwrap().remove("readme");
    let root = workspace.write("agent.json", &manifest);
    let schema = json!({"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"x-agentpm-custom-user-field":{"type":"string"}},"examples":[{"$ref":"https://example.invalid/not-a-schema","x-agentpm-unknown":"ordinary example data"}]});
    workspace.write("schemas/user-preference.schema.json", &schema);
    let mut command = args("packaged", &root);
    command.extend([
        "--package-root".into(),
        workspace.0.to_string_lossy().into_owned(),
    ]);
    assert_eq!(
        report(&workspace.run(&command))["apds"]["status"],
        "conformant"
    );
    let schema = json!({"$id":"https://example.invalid/rebased","type":"object","properties":{"value":{"$ref":"https://127.0.0.1:1/not-fetched"}}});
    workspace.write("schemas/user-preference.schema.json", &schema);
    let output = workspace.run(&command);
    assert!(!output.status.success());
    assert_eq!(report(&output)["apds"]["status"], "nonconformant");
}

#[cfg(unix)]
#[test]
fn packaged_content_cannot_escape_through_a_symbolic_link() {
    let workspace = Workspace::new();
    let package = workspace.0.join("package");
    fs::create_dir_all(package.join("docs")).unwrap();
    let root = workspace.write(
        "package/agent.json",
        &read(&directory().join("fixtures/packaged/knowledge-context-files/agent.json")),
    );
    fs::write(workspace.0.join("outside.md"), "Hello\n").unwrap();
    std::os::unix::fs::symlink(
        workspace.0.join("outside.md"),
        package.join("docs/context.md"),
    )
    .unwrap();
    let mut command = args("packaged", &root);
    command.extend([
        "--package-root".into(),
        package.to_string_lossy().into_owned(),
    ]);
    let output = workspace.run(&command);
    assert!(!output.status.success());
    assert_eq!(report(&output)["apds"]["status"], "nonconformant");
}

#[test]
fn selected_nested_agents_receive_resolved_checks_and_missing_edges_do_not_pass() {
    let workspace = Workspace::new();
    let directory = directory();
    let index = read(&directory.join("fixture-index.json"));
    let case = index["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["case_id"] == "template-selected-dependencies")
        .unwrap();
    let mut graph = case["graph"].clone();
    for node in graph["artifacts"].as_array_mut().unwrap() {
        node["manifest"] = json!(directory.join(node["manifest"].as_str().unwrap()));
    }
    let node = graph["artifacts"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|n| n["kind"] == "agent")
        .unwrap();
    let mut agent = read(Path::new(node["manifest"].as_str().unwrap()));
    agent["bindings"]["phases"]["unknown-phase"] = agent["bindings"]["phases"]["review"].clone();
    let nested = workspace.write("nested-agent.json", &agent);
    node["manifest"] = json!(nested);
    let graph_path = workspace.write("graph.json", &graph);
    let root = directory.join(case["root"].as_str().unwrap());
    let mut command = args("resolved", &root);
    command.extend(["--graph".into(), graph_path.to_string_lossy().into_owned()]);
    let output = workspace.run(&command);
    assert!(!output.status.success());
    assert!(
        report(&output)["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["rule_id"] == "APDS-BIND-002"
                && i["instance_path"] == "/bindings/phases/unknown-phase"
                && i["file"] == json!(nested))
    );
    graph["edges"]
        .as_array_mut()
        .unwrap()
        .retain(|e| !(e["from"] == "root" && e["field"] == "/template/dependencies/agents/0"));
    workspace.write("graph.json", &graph);
    let missing = report(&workspace.run(&command));
    assert_eq!(missing["apds"]["status"], "nonconformant");
    assert!(
        missing["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["rule_id"] == "APDS-GRAPH-002")
    );
}

#[test]
fn packaged_tool_literal_directories_are_valid_and_missing_license_files_are_not() {
    let workspace = Workspace::new();
    let mut value = read(&directory().join("fixtures/valid/tool.json"));
    value["files"] = json!(["src"]);
    let root = workspace.write("agent.json", &value);
    fs::create_dir_all(workspace.0.join("src")).unwrap();
    fs::write(workspace.0.join("src/main.py"), "print('{}')\n").unwrap();
    let mut command = args("packaged", &root);
    command.extend([
        "--package-root".into(),
        workspace.0.to_string_lossy().into_owned(),
    ]);
    assert_eq!(
        report(&workspace.run(&command))["apds"]["status"],
        "conformant"
    );
    value["license"] = json!({"file":"missing-license.txt"});
    workspace.write("agent.json", &value);
    let invalid = report(&workspace.run(&command));
    assert_eq!(invalid["apds"]["status"], "nonconformant");
    assert!(
        invalid["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["instance_path"] == "/license/file")
    );
}

#[test]
fn unsupported_dependencies_are_not_validated_using_the_supported_contract() {
    let workspace = Workspace::new();
    let directory = directory();
    let mut value = read(&directory.join("fixtures/valid/minimal-agent.json"));
    value["tools"] = json!(["@example/foreign@1.0.0"]);
    let root = workspace.write("agent.json", &value);
    let foreign = workspace.write("foreign.json", &json!({"kind":"tool","name":"foreign","version":"1.0.0","standard":{"id":"agentpm","version":"999.0.0"}}));
    let graph = workspace.write("graph.json", &json!({"complete":true,"artifacts":[{"id":"foreign","identity":"@example/foreign","kind":"tool","version":"1.0.0","manifest":foreign}],"edges":[{"from":"root","field":"/tools/0","to":"foreign"}]}));
    let mut command = args("resolved", &root);
    command.extend(["--graph".into(), graph.to_string_lossy().into_owned()]);
    let actual = report(&workspace.run(&command));
    assert_eq!(actual["apds"]["status"], "unsupported");
    assert_eq!(
        actual["issues"],
        json!([]),
        "Do not apply v1 required Tool fields to a foreign contract"
    );
}

#[test]
fn unsupported_vector_encoding_is_not_an_authored_violation_but_missing_files_are() {
    let workspace = Workspace::new();
    let mut value = read(&directory().join("fixtures/packaged/knowledge-vector-files/agent.json"));
    value["knowledge"]
        .as_object_mut()
        .unwrap()
        .remove("indexes");
    value["knowledge"]["embedding"]["vectors_path"] = json!("vectors.opaque");
    let root = workspace.write("agent.json", &value);
    for path in ["chunks.jsonl", "sources.jsonl", "vectors.opaque"] {
        fs::write(workspace.0.join(path), "opaque\n").unwrap();
    }
    let mut command = args("packaged", &root);
    command.extend([
        "--package-root".into(),
        workspace.0.to_string_lossy().into_owned(),
    ]);
    let unknown = report(&workspace.run(&command));
    assert_eq!(unknown["apds"]["status"], "unsupported");
    assert_eq!(unknown["issues"], json!([]));
    value["knowledge"]["provenance"] = json!({"sources_manifest_path":"missing.json"});
    workspace.write("agent.json", &value);
    assert_eq!(
        report(&workspace.run(&command))["apds"]["status"],
        "nonconformant"
    );
}

#[test]
fn composed_governance_is_reported_unevaluated_without_a_false_shallow_verdict() {
    let workspace = Workspace::new();
    let mut value = read(&directory().join("fixtures/valid/memory.json"));
    value.as_object_mut().unwrap().remove("readme");
    let root = workspace.write("agent.json", &value);
    workspace.write("schemas/user-preference.schema.json", &json!({"type":"object","anyOf":[{"properties":{"secret":{"type":"string","x-agentpm-persist":false}},"required":["secret"]},{"properties":{"public":{"type":"string"}},"required":["public"]}]}));
    let mut command = args("packaged", &root);
    command.extend([
        "--package-root".into(),
        workspace.0.to_string_lossy().into_owned(),
    ]);
    let actual = report(&workspace.run(&command));
    assert_eq!(actual["apds"]["status"], "unsupported");
    assert_eq!(actual["issues"], json!([]));
    assert_eq!(actual["apds"]["unevaluated"], json!(["APDS-MEM-006"]));
}

#[test]
fn malformed_selectors_and_python_requirements_have_field_diagnostics() {
    let workspace = Workspace::new();
    let mut tool = read(&directory().join("fixtures/valid/tool.json"));
    tool["runtime"]["dependencies"] = json!(["not a valid requirement !!!"]);
    let root = workspace.write("agent.json", &tool);
    let invalid = report(&workspace.run(&args("standalone", &root)));
    assert_eq!(invalid["apds"]["status"], "nonconformant");
    assert!(
        invalid["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["instance_path"] == "/runtime/dependencies"
                && i["rule_id"] == "APDS-TOOL-002")
    );
    tool["runtime"]["dependencies"] = json!(["valid-distribution>=1"]);
    tool["standard"]["version"] = json!("");
    workspace.write("agent.json", &tool);
    let malformed = report(&workspace.run(&args("standalone", &root)));
    assert_eq!(malformed["apds"]["status"], "nonconformant");
    assert!(
        malformed["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["rule_id"] == "APDS-ID-001")
    );
}

#[test]
fn unavailable_graph_documents_remain_incomplete_and_do_not_hide_other_violations() {
    let workspace = Workspace::new();
    let directory = directory();
    let mut value = read(&directory.join("fixtures/resolved/composition-complete.json"));
    let root = workspace.write("agent.json", &value);
    let index = read(&directory.join("fixture-index.json"));
    let mut graph = index["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["case_id"] == "composition-complete")
        .unwrap()["graph"]
        .clone();
    for node in graph["artifacts"].as_array_mut().unwrap() {
        node["manifest"] = if node["kind"] == "loop" {
            json!(workspace.0.join("unavailable-loop.json"))
        } else {
            json!(directory.join(node["manifest"].as_str().unwrap()))
        };
    }
    let graph_path = workspace.write("graph.json", &graph);
    let mut command = args("resolved", &root);
    command.extend(["--graph".into(), graph_path.to_string_lossy().into_owned()]);
    let incomplete = report(&workspace.run(&command));
    assert_eq!(incomplete["apds"]["status"], "incomplete");
    assert!(incomplete["issues"].as_array().unwrap().is_empty());
    value["bindings"]["global"]["memory"][0]["spaces"] = json!(["not-a-space"]);
    workspace.write("agent.json", &value);
    let invalid = report(&workspace.run(&command));
    assert_eq!(invalid["apds"]["status"], "nonconformant");
    assert!(
        invalid["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["rule_id"] == "APDS-BIND-001")
    );
}

#[test]
fn known_index_associations_and_corpus_hashes_match_actual_payloads() {
    let workspace = Workspace::new();
    let source = directory().join("fixtures/packaged/knowledge-vector-files");
    for entry in walkdir::WalkDir::new(&source) {
        let entry = entry.unwrap();
        let destination = workspace
            .0
            .join(entry.path().strip_prefix(&source).unwrap());
        if entry.file_type().is_dir() {
            fs::create_dir_all(destination).unwrap();
        } else {
            fs::copy(entry.path(), destination).unwrap();
        }
    }
    let root = workspace.0.join("agent.json");
    let mut command = args("packaged", &root);
    command.extend([
        "--package-root".into(),
        workspace.0.to_string_lossy().into_owned(),
    ]);
    assert_eq!(
        report(&workspace.run(&command))["apds"]["status"],
        "conformant"
    );
    let metadata_path = "knowledge/indexes/default/metadata.json";
    let original = read(&workspace.0.join(metadata_path));
    for (field, bad) in [
        ("vectors_path", json!("other.f32")),
        ("metric", json!("other-metric")),
        ("normalized", json!(true)),
    ] {
        let mut changed = original.clone();
        changed[field] = bad;
        workspace.write(metadata_path, &changed);
        assert_eq!(
            report(&workspace.run(&command))["apds"]["status"],
            "nonconformant",
            "{field}"
        );
    }
    workspace.write(metadata_path, &original);
    let mut manifest = read(&root);
    manifest["knowledge"]["corpus"]["content_hash"] = json!(format!("sha256:{}", "0".repeat(64)));
    workspace.write("agent.json", &manifest);
    let invalid = report(&workspace.run(&command));
    assert!(
        invalid["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["instance_path"] == "/knowledge/corpus/content_hash"
                && i["rule_id"] == "APDS-KNOW-002")
    );
}
