//! Real CLI preflight/machine contracts; no Registry or model API calls.
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
const LOOP: &str = "loop:@fixture/workflow@1.0.0";

struct Project(PathBuf);
impl Project {
    fn new(with_loop: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "agentpm-harness-standard-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let this = Self(root);
        let mut agent = json!({"kind":"agent","name":"consumer","version":"1.0.0","description":"Preflight fixture.","standard":{"id":"agentpm","version":"1.0.0"}});
        let mut lock = json!({"lockfile_version":4,"generated":"2026-11-21T00:00:00Z","packages":{},"roots":{"local:agent":{"name":"consumer","version":"1.0.0","standard":{"id":"agentpm","version":"1.0.0"},"standard_origin":"declared"}}});
        if with_loop {
            agent["loop"] = json!("@fixture/workflow@1.0");
            lock["roots"]["local:agent"]["loop"] = json!(LOOP);
            lock["packages"][LOOP] = json!({"kind":"loop","name":"@fixture/workflow","version":"1.0.0","integrity":"fixture","standard":{"id":"agentpm","version":"1.0.0"},"standard_origin":"declared"});
            this.write(".agentpm/loops/fixture/workflow/1.0.0/agent.json", json!({"kind":"loop","name":"workflow","version":"1.0.0","description":"A single phase.","standard":{"id":"agentpm","version":"1.0.0"},"loop":{"entry_phase":"execute","phases":[{"id":"execute","objective":"Answer."}],"transitions":[{"from":"execute","on":"complete","to":"$end"}]}}));
        }
        this.write("agent.json", agent);
        this.write("agent.lock", lock);
        this
    }
    fn write(&self, name: &str, value: Value) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    }
    fn edit(&self, name: &str, edit: impl FnOnce(&mut Value)) {
        let mut value: Value =
            serde_json::from_slice(&fs::read(self.0.join(name)).unwrap()).unwrap();
        edit(&mut value);
        self.write(name, value);
    }
    fn command(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_agentpm"));
        cmd.current_dir(&self.0)
            .env("AGENTPM_TELEMETRY", "0")
            .env_remove("OPENAI_API_KEY")
            .env_remove("ANTHROPIC_API_KEY");
        cmd
    }
    fn report(&self, success: bool) -> Value {
        let output = self.command().args(["harness", "--json"]).output().unwrap();
        assert_eq!(
            output.status.success(),
            success,
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
    fn configured(&self) {
        self.write(
            "agentpm.harness.json",
            json!({"version":1,"model":{"provider":"ollama","model":"fixture"}}),
        );
    }
    fn snapshot(&self) -> Vec<(PathBuf, Vec<u8>)> {
        fn collect(path: &Path, rows: &mut Vec<(PathBuf, Vec<u8>)>) {
            for entry in fs::read_dir(path).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    collect(&path, rows);
                } else {
                    rows.push((path.clone(), fs::read(path).unwrap()));
                }
            }
        }
        let mut rows = Vec::new();
        collect(&self.0, &mut rows);
        rows.sort();
        rows
    }
    fn machine(&self) -> (Output, Vec<Value>) {
        let mut child = self
            .command()
            .args(["harness", "--machine"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        for method in ["initialize", "preflight", "start_run", "shutdown"] {
            writeln!(stdin, "{}", json!({"protocol":"agentpm-harness-machine","version":1,"kind":"request","id":method,"method":method,"payload":{"input":"Never execute on failed preflight."}})).unwrap();
        }
        drop(stdin);
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let frames = String::from_utf8(output.stdout.clone())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        (output, frames)
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn has_code(report: &Value, code: &str) -> bool {
    report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == code)
}

#[test]
fn minimal_declared_agent_is_conformant_but_not_harness_runnable() {
    let p = Project::new(false);
    let before = p.snapshot();
    let report = p.report(false);
    assert_eq!(report["apds"]["status"], "conformant");
    assert_eq!(report["harness_support"], "not_runnable");
    assert_eq!(report["local_readiness"], "not_evaluated");
    assert!(has_code(&report, "agent_missing_loop"));
    let explicit = p
        .command()
        .args(["harness", "consumer", "--json"])
        .output()
        .unwrap();
    let explicit: Value = serde_json::from_slice(&explicit.stdout).unwrap();
    assert_eq!(report, explicit);
    assert_eq!(before, p.snapshot());
}

#[test]
fn configured_supported_agent_is_ready_and_does_not_contact_the_provider() {
    let p = Project::new(true);
    p.configured();
    let before = p.snapshot();
    let report = p.report(true);
    assert_eq!(
        report["supported_standards"],
        json!([{"id":"agentpm","version":"1.0.0"}])
    );
    assert_eq!(report["apds"]["status"], "conformant");
    assert_eq!(
        report["apds"]["checked_levels"],
        json!(["standalone", "resolved"])
    );
    assert_eq!(report["apds"]["runner_evaluated"], false);
    assert_eq!(report["harness_support"], "supported");
    assert_eq!(report["local_readiness"], "ready");
    assert_eq!(report["apds"]["artifacts"].as_array().unwrap().len(), 2);
    assert_eq!(before, p.snapshot());
}

#[test]
fn noninteractive_json_headless_and_machine_do_not_assume_model_prompting() {
    let p = Project::new(true);
    let report = p.report(false);
    assert_eq!(report["apds"]["status"], "conformant");
    assert_eq!(report["harness_support"], "supported");
    assert_eq!(report["local_readiness"], "not_ready");
    assert!(has_code(&report, "model_not_configured"));
    let headless = p
        .command()
        .args(["harness", "--headless", "--input", "hello"])
        .output()
        .unwrap();
    assert!(!headless.status.success());
    assert!(headless.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&headless.stderr);
    assert!(
        stderr.contains("model.provider and model.model"),
        "{stderr}"
    );
    assert!(stderr.contains("Harness supported APDS standards: agentpm / 1.0.0"));
    let (_, frames) = p.machine();
    let init = frames.iter().find(|f| f["id"] == "initialize").unwrap();
    assert_eq!(init["payload"]["preflight"], report);
    assert_eq!(
        init["payload"]["session"],
        json!({"protocol":"agentpm-harness-machine","version":1})
    );
    let preflight = frames.iter().find(|f| f["id"] == "preflight").unwrap();
    assert_eq!(preflight["payload"]["report"], report);
    let start = frames.iter().find(|f| f["id"] == "start_run").unwrap();
    assert_eq!(start["error"]["code"], "preflight_not_ready");
}

#[test]
fn unsupported_root_and_component_standards_never_fall_back_or_start() {
    for path in [
        "agent.json",
        ".agentpm/loops/fixture/workflow/1.0.0/agent.json",
    ] {
        let p = Project::new(true);
        p.configured();
        p.edit(path, |v| v["standard"]["version"] = json!("9.0.0"));
        let before = p.snapshot();
        let report = p.report(false);
        assert_eq!(report["apds"]["status"], "unsupported", "{report}");
        assert_eq!(report["harness_support"], "unsupported");
        assert_eq!(report["local_readiness"], "not_evaluated");
        assert!(has_code(&report, "unsupported_standard"));
        let diagnostic = report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["code"] == "unsupported_standard")
            .unwrap();
        let reported_path = diagnostic["path"].as_str().unwrap();
        assert!(Path::new(reported_path).is_relative());
        let message = diagnostic["message"].as_str().unwrap();
        assert!(
            message.starts_with(&format!("{reported_path}: ")),
            "{message}"
        );
        assert!(
            !message.contains(&p.0.canonicalize().unwrap().to_string_lossy().to_string()),
            "{message}"
        );
        let (_, frames) = p.machine();
        assert!(
            frames
                .iter()
                .any(|f| f["id"] == "start_run" && f["error"]["code"] == "preflight_not_ready")
        );
        assert_eq!(before, p.snapshot());
    }
}

#[test]
fn structural_and_relationship_errors_are_definition_failures() {
    for defect in ["description", "phase", "standard", "identity"] {
        let p = Project::new(true);
        p.configured();
        p.edit("agent.json", |v| match defect {
            "description" => v["description"] = json!(" "),
            "phase" => v["bindings"] = json!({"phases":{"absent":{}}}),
            "standard" => v["standard"] = Value::Null,
            _ => v["version"] = json!("2.0.0"),
        });
        let report = p.report(false);
        assert_eq!(report["apds"]["status"], "nonconformant", "{report}");
        assert_eq!(report["local_readiness"], "not_evaluated");
    }
}

#[test]
fn missing_locked_or_installed_context_is_not_reported_conformant() {
    for missing in ["lock", "manifest"] {
        let p = Project::new(true);
        if missing == "lock" {
            p.edit("agent.lock", |v| {
                v["packages"].as_object_mut().unwrap().remove(LOOP);
            });
        } else {
            fs::remove_file(p.0.join(".agentpm/loops/fixture/workflow/1.0.0/agent.json")).unwrap();
        }
        let report = p.report(false);
        assert_eq!(report["apds"]["status"], "incomplete", "{report}");
        assert_eq!(report["local_readiness"], "not_evaluated");
    }
}

#[test]
fn malformed_selected_json_and_inconsistent_lock_identity_block_preflight() {
    let p = Project::new(true);
    fs::write(p.0.join("agent.json"), "{\"standard\":").unwrap();
    let report = p.report(false);
    assert_eq!(report["apds"]["status"], "nonconformant");
    assert!(has_code(&report, "apds_nonconformant"));
    let p = Project::new(true);
    p.edit("agent.lock", |v| {
        let value = v["packages"].as_object_mut().unwrap().remove(LOOP).unwrap();
        v["packages"]["loop:@fixture/alias@1.0.0"] = value;
        v["roots"]["local:agent"]["loop"] = json!("loop:@fixture/alias@1.0.0");
    });
    let report = p.report(false);
    assert!(has_code(&report, "apds_identity_mismatch"));
}

#[test]
fn legacy_origin_stays_qualified_and_provenance_mismatch_is_rejected() {
    let p = Project::new(true);
    p.configured();
    p.edit("agent.json", |v| {
        v.as_object_mut().unwrap().remove("standard");
    });
    p.edit("agent.lock", |v| {
        v["roots"]["local:agent"]["standard_origin"] = json!("legacy_inferred")
    });
    let before = p.snapshot();
    let report = p.report(true);
    assert_eq!(report["apds"]["status"], "legacy_inferred");
    assert!(has_code(&report, "legacy_standard_inferred"));
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["severity"] != "fatal")
    );
    assert_eq!(before, p.snapshot());
    p.edit("agent.lock", |v| {
        v["roots"]["local:agent"]["standard_origin"] = json!("declared")
    });
    let report = p.report(false);
    assert!(has_code(&report, "apds_provenance_mismatch"));
}

#[test]
fn legacy_component_in_declared_graph_is_qualified_without_rewriting() {
    let p = Project::new(true);
    p.configured();
    p.edit(".agentpm/loops/fixture/workflow/1.0.0/agent.json", |v| {
        v.as_object_mut().unwrap().remove("standard");
    });
    p.edit("agent.lock", |v| {
        v["packages"][LOOP]["standard_origin"] = json!("legacy_inferred")
    });
    let before = p.snapshot();
    let report = p.report(true);
    assert_eq!(report["apds"]["status"], "legacy_inferred");
    assert!(
        report["apds"]["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["artifact_id"] == LOOP
                && a["status"] == "legacy_inferred"
                && a["evaluated_rules"] == json!([])
                && a["unevaluated"] == json!(["APDS-ID-001"]))
    );
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["severity"] != "fatal")
    );
    assert_eq!(before, p.snapshot());
}

#[test]
fn legacy_root_and_component_defects_warn_while_declared_defects_fail() {
    for component in [false, true] {
        for legacy in [false, true] {
            let p = Project::new(true);
            p.configured();
            let path = if component {
                ".agentpm/loops/fixture/workflow/1.0.0/agent.json"
            } else {
                "agent.json"
            };
            p.edit(path, |v| {
                v["description"] = json!("   ");
                if legacy {
                    v.as_object_mut().unwrap().remove("standard");
                }
            });
            if legacy {
                p.edit("agent.lock", |v| {
                    let row = if component {
                        &mut v["packages"][LOOP]
                    } else {
                        &mut v["roots"]["local:agent"]
                    };
                    row["standard_origin"] = json!("legacy_inferred");
                });
            }
            let source = p.0.join(path).canonicalize().unwrap();
            let project = p.0.canonicalize().unwrap();
            let source = source.strip_prefix(project).unwrap().to_string_lossy();
            let before = p.snapshot();
            let report = p.report(legacy);
            assert_eq!(
                report["apds"]["status"],
                if legacy {
                    "legacy_inferred"
                } else {
                    "nonconformant"
                }
            );
            assert_eq!(
                report["status"],
                if legacy {
                    "ready_with_warnings"
                } else {
                    "failed"
                }
            );
            let findings = report["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|d| {
                    d["path"] == format!("{source}#/description")
                        && d["message"]
                            .as_str()
                            .is_some_and(|s| s.contains("APDS-ID-002"))
                })
                .collect::<Vec<_>>();
            assert_eq!(findings.len(), 1, "{report}");
            assert_eq!(
                findings[0]["severity"],
                if legacy { "warning" } else { "fatal" }
            );
            assert_eq!(
                findings[0]["code"],
                if legacy {
                    "legacy_apds_issue"
                } else {
                    "apds_nonconformant"
                }
            );
            if legacy {
                assert!(
                    report["diagnostics"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|d| d["severity"] != "fatal")
                );
                assert!(!report["diagnostics"].as_array().unwrap().iter().any(|d| {
                    d["message"]
                        .as_str()
                        .is_some_and(|m| m.contains("APDS-ID-001 at /standard:"))
                }));
                assert_eq!(
                    report["apds"]["checked_levels"],
                    json!(["standalone", "resolved"])
                );
            }
            assert_eq!(before, p.snapshot());
        }
    }
}

#[test]
fn legacy_resolved_phase_binding_defect_is_a_visible_nonfatal_apds_warning() {
    let p = Project::new(true);
    p.configured();
    p.edit("agent.json", |v| {
        v.as_object_mut().unwrap().remove("standard");
        v["bindings"] = json!({"phases":{"nonexistent":{}}});
    });
    p.edit("agent.lock", |v| {
        v["roots"]["local:agent"]["standard_origin"] = json!("legacy_inferred")
    });
    let before = p.snapshot();
    let report = p.report(true);
    assert_eq!(report["apds"]["status"], "legacy_inferred");
    assert_eq!(report["status"], "ready_with_warnings");
    assert_eq!(
        report["apds"]["checked_levels"],
        json!(["standalone", "resolved"])
    );
    let findings = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| {
            d["code"] == "legacy_apds_issue"
                && d["path"] == "agent.json#/bindings/phases/nonexistent"
                && d["message"]
                    .as_str()
                    .is_some_and(|m| m.contains("APDS-BIND-002"))
        })
        .collect::<Vec<_>>();
    assert_eq!(findings.len(), 1, "{report}");
    assert_eq!(findings[0]["severity"], "warning");
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["severity"] != "fatal")
    );
    assert_eq!(before, p.snapshot());
}

#[test]
fn inherited_tool_standard_is_checked_before_runtime_shape_parsing() {
    let p = Project::new(true);
    p.configured();
    p.edit("agent.json", |v| {
        v["skills"] = json!(["@fixture/instructions@1.0.0"])
    });
    p.edit("agent.lock", |v| {
        v["roots"]["local:agent"]["skills"] = json!(["skill:@fixture/instructions@1.0.0"]);
        v["packages"]["skill:@fixture/instructions@1.0.0"] = json!({"kind":"skill","name":"@fixture/instructions","version":"1.0.0","integrity":"fixture"});
        v["packages"]["tool:@fixture/future@1.0.0"] = json!({"kind":"tool","name":"@fixture/future","version":"1.0.0","integrity":"fixture"});
    });
    p.write(".agentpm/skills/fixture/instructions/1.0.0/agent.json", json!({"kind":"skill","name":"instructions","version":"1.0.0","description":"Instructions.","standard":{"id":"agentpm","version":"1.0.0"},"tools":["@fixture/future@1.0.0"],"skill":{"entrypoint":"SKILL.md"}}));
    // The current Tool parser need not understand a future standard's shape.
    p.write(
        ".agentpm/tools/fixture/future/1.0.0/agent.json",
        json!({"standard":{"id":"agentpm","version":"9.0.0"},"future_execution_contract":{}}),
    );
    let report = p.report(false);
    assert_eq!(report["harness_support"], "unsupported", "{report}");
    assert!(has_code(&report, "unsupported_standard"));
}

#[test]
fn unselected_workspace_components_do_not_determine_standard_support() {
    let p = Project::new(true);
    p.configured();
    p.edit("agent.lock", |v| v["packages"]["tool:@fixture/unrelated@9.0.0"] = json!({"kind":"tool","name":"@fixture/unrelated","version":"9.0.0","integrity":"fixture"}));
    p.write(
        ".agentpm/tools/fixture/unrelated/9.0.0/agent.json",
        json!({"standard":{"id":"agentpm","version":"9.0.0"}}),
    );
    assert_eq!(p.report(true)["apds"]["status"], "conformant");
}

#[test]
fn unsupported_lock_metadata_has_an_explicit_compatibility_result() {
    let p = Project::new(true);
    p.edit("agent.lock", |v| {
        v["packages"][LOOP]["standard"]["version"] = json!("9.0.0")
    });
    let report = p.report(false);
    assert_eq!(report["apds"]["status"], "unsupported");
    assert_eq!(report["harness_support"], "unsupported");
    assert!(has_code(&report, "unsupported_standard"));
}

#[test]
fn dependency_normalization_does_not_hide_invalid_authored_fields() {
    let p = Project::new(true);
    p.edit(
        "agent.json",
        |v| v["loop"] = json!({"name":"@fixture/workflow","version":"1.0.0","unexpected":true}),
    );
    let report = p.report(false);
    assert_eq!(report["apds"]["status"], "nonconformant");
    assert!(has_code(&report, "apds_nonconformant"));
}

#[test]
fn configured_memory_and_knowledge_backend_limitations_are_not_invalid_apds() {
    for kind in ["memory", "knowledge"] {
        let p = Project::new(true);
        p.configured();
        let field = kind;
        let key = format!("{kind}:@fixture/component@1.0.0");
        p.edit("agent.lock", |v| {
            v["packages"][&key] = json!({"kind":kind,"name":"@fixture/component","version":"1.0.0","integrity":"fixture","standard":{"id":"agentpm","version":"1.0.0"},"standard_origin":"declared"});
            v["roots"]["local:agent"][field] = json!([key]);
        });
        p.edit("agent.json", |v| {
            v[field] = json!(["@fixture/component@1.0.0"]);
            v["bindings"] = if kind == "memory" {
                json!({"global":{"memory":[{"package":"@fixture/component","spaces":["profile"]}]}})
            } else {
                json!({"global":{"knowledge":["@fixture/component"]}})
            };
        });
        let manifest = if kind == "memory" {
            let mut value: Value = serde_json::from_str(include_str!(
                "../../../standards/agentpm/1.0.0/conformance/fixtures/valid/memory.json"
            ))
            .unwrap();
            value["name"] = json!("component");
            value["version"] = json!("1.0.0");
            value["memory"]["spaces"]["profile"]["model"] = json!("collection");
            value["memory"]["spaces"]["profile"]["retrieval"]["modes"] = json!(["semantic"]);
            value
        } else {
            p.edit("agentpm.harness.json", |v| v["knowledge"] = json!({"runtimes":{"remote":{"implementation":{"type":"host"}}},"packages":{"@fixture/component":{"runtime":"remote"}}}));
            json!({"kind":"knowledge","name":"component","version":"1.0.0","description":"Context.","standard":{"id":"agentpm","version":"1.0.0"},"knowledge":{"mode":"context","documents":[{"path":"context.md"}]}})
        };
        p.write(
            &format!(".agentpm/{kind}/fixture/component/1.0.0/agent.json"),
            manifest,
        );
        let report = p.report(true);
        assert_eq!(report["apds"]["status"], "conformant", "{report}");
        assert_eq!(report["harness_support"], "limited");
        assert_eq!(report["local_readiness"], "limited");
        assert!(has_code(
            &report,
            if kind == "memory" {
                "unrealizable_memory_space"
            } else {
                "host_implementation_unavailable"
            }
        ));
    }
}

#[test]
fn missing_provider_credentials_are_local_configuration_not_apds_errors() {
    for provider in ["openai", "anthropic"] {
        let p = Project::new(true);
        p.write(
            "agentpm.harness.json",
            json!({"version":1,"model":{"provider":provider,"model":"fixture"}}),
        );
        let report = p.report(false);
        assert_eq!(report["apds"]["status"], "conformant");
        assert_eq!(report["local_readiness"], "not_ready");
        assert!(has_code(&report, "model_provider_not_ready"));
    }
}
