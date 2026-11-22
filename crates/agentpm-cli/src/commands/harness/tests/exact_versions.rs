use super::*;

fn versioned_workspace(label: &str) -> PathBuf {
    let root = temp_dir(label);
    let standard = json!({"id":"agentpm","version":"1.0.0"});
    let mut packages = json!({});
    let mut roots = json!({});
    for major in [1, 2] {
        let version = format!("{major}.0.0");
        let mut agent = json!({"kind":"agent","name":format!("consumer-{major}"),"version":"1.0.0","description":"Exact selected graph.","standard":standard,"bindings":{"global":{}}});
        let mut selected = json!({"name":format!("consumer-{major}"),"version":"1.0.0","standard":standard,"standard_origin":"declared"});
        for (kind, directory, field) in [
            ("tool", "tools", "tools"),
            ("skill", "skills", "skills"),
            ("knowledge", "knowledge", "knowledge"),
            ("memory", "memory", "memory"),
            ("profile", "profiles", "profiles"),
            ("loop", "loops", "loop"),
        ] {
            let key = format!("{kind}:@fixture/{kind}@{version}");
            let name = format!("@fixture/{kind}");
            let folder = root.join(format!(".agentpm/{directory}/fixture/{kind}/{version}"));
            let mut value = json!({"kind":kind,"name":kind,"version":version,"description":"Exact component fixture.","standard":standard});
            match kind {
                "tool" => {
                    value["entrypoint"] = json!({"command":"node","args":["main.cjs"]});
                    value["files"] = json!(["main.cjs"]);
                    value["inputs"] = json!({"type":"object"});
                    value["outputs"] = json!({"type":"object"});
                }
                "skill" => {
                    value["skill"] = json!({"entrypoint":"SKILL.md"});
                    value["tools"] = json!([format!("@fixture/tool@{version}")]);
                }
                "knowledge" => {
                    value["knowledge"] = json!({"mode":"context","documents":[{"path":"docs/guide.md","content_type":"text/markdown"}]})
                }
                "profile" => {
                    value["profile"] = json!({"identity":{"role":"reviewer","description":"Review carefully."},"communication":{"tone":["clear"],"verbosity":"concise"},"objectives":["Review carefully."]})
                }
                "loop" => {
                    value["loop"] = json!({"entry_phase":"work","phases":[{"id":"work","objective":"Complete."}],"transitions":[{"from":"work","on":"complete","to":"$end"}]})
                }
                "memory" => {
                    value["memory"] = json!({"record_types":{"note":{"schema":"schema.json","version":"1.0.0","description":"A note."}},"scopes":{"user":{"description":"User."}},"spaces":{"notes":{"description":"Notes.","model":"collection","scope":["user"],"record_types":["note"],"retrieval":{"modes":["key"]}}},"operations":{"prune":{"type":"delete","description":"Prune externally.","trigger":{"type":"external"},"targets":[{"space":"notes"}],"cascade_derived_records":false}}})
                }
                _ => unreachable!(),
            }
            write_json(&folder.join("agent.json"), value);
            fs::write(folder.join("main.cjs"), "console.log('{}');").unwrap();
            fs::write(folder.join("SKILL.md"), format!("# Skill {version}\n")).unwrap();
            fs::create_dir_all(folder.join("docs")).unwrap();
            fs::write(folder.join("docs/guide.md"), format!("# Guide {version}\n")).unwrap();
            write_json(&folder.join("schema.json"), json!({"type":"object"}));
            if kind == "memory" {
                crate::commands::memory::execute_memory_build(
                    &folder.join("agent.json"),
                    crate::commands::memory::MemoryBuildMode::Write,
                )
                .unwrap();
            }
            if kind == "knowledge" {
                crate::commands::knowledge::execute_knowledge_build(
                    &folder.join("agent.json"),
                    crate::commands::knowledge::KnowledgeBuildMode::Write,
                )
                .unwrap();
            }
            packages[&key] = json!({"kind":kind,"name":name,"version":version,"integrity":"fixture","standard":standard,"standard_origin":"declared"});
            agent[field] = if kind == "loop" {
                json!(format!("{name}@{version}"))
            } else {
                json!([format!("{name}@{version}")])
            };
            selected[field] = if kind == "loop" {
                json!(key)
            } else {
                json!([key])
            };
            if kind != "loop" {
                agent["bindings"]["global"][field] = if kind == "memory" {
                    json!([{"package":name,"operations":["prune"]}])
                } else {
                    json!([name])
                };
            }
        }
        // Agent 2 gets its Tool only through its selected Skill.
        if major == 2 {
            agent["tools"] = json!([]);
            agent["bindings"]["global"]
                .as_object_mut()
                .unwrap()
                .remove("tools");
            selected["tools"] = json!([]);
        }
        let path = format!("agents/agent-{major}.agent.json");
        write_json(&root.join(&path), agent);
        roots[format!("local:agent:{path}")] = selected;
    }
    write_json(
        &root.join("agent.lock"),
        json!({"lockfile_version":4,"generated":"2026-11-21T00:00:00Z","packages":packages,"roots":roots}),
    );
    write_json(
        &root.join("agentpm.harness.json"),
        json!({"version":1,"model":{"provider":"ollama","model":"fixture"},"scopes":{"user":"fixture-user"}}),
    );
    root
}

#[test]
fn selected_agent_runtime_uses_exact_versions_for_every_component_kind() {
    let root = versioned_workspace("m6b-selected-components");
    for major in [1, 2] {
        let plan = resolve_harness_plan(
            &root,
            &HarnessBootstrapOptions {
                agent_selector: Some(format!("consumer-{major}")),
                ..Default::default()
            },
        )
        .unwrap();
        assert_ne!(
            plan.report.status,
            PreflightStatus::Failed,
            "{:?}",
            plan.report.diagnostics
        );
        let version = format!("{major}.0.0");
        assert_eq!(plan.package_graph.len(), 6);
        assert!(plan.package_graph.values().all(|p| p.version == version));
        let runtime = runtime_snapshot_from_plan(&plan);
        assert_eq!(runtime.tools.len(), 1);
        assert_eq!(runtime.tools[0].version, version);
        assert_eq!(runtime.skills.len(), 1);
        assert_eq!(runtime.skills[0].version, version);
        assert_eq!(runtime.knowledge.len(), 1);
        assert_eq!(runtime.knowledge[0].version, version);
        assert_eq!(runtime.profiles.len(), 1);
        assert_eq!(runtime.profiles[0].version, version);
        assert_eq!(runtime.loop_package.unwrap().version, version);
        assert!(runtime.memory.is_empty());
        assert_eq!(runtime.memory_operations.len(), 1);
        assert_eq!(runtime.memory_operations[0].package_version, version);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn ambiguous_active_versions_are_a_runner_limitation_not_arbitrary_selection() {
    let root = versioned_workspace("m6b-ambiguous-components");
    let skill = root.join(".agentpm/skills/fixture/skill/1.0.0/agent.json");
    let mut value: Value = serde_json::from_slice(&fs::read(&skill).unwrap()).unwrap();
    value["tools"] = json!(["@fixture/tool@2.0.0"]);
    write_json(&skill, value);
    let plan = resolve_harness_plan(
        &root,
        &HarnessBootstrapOptions {
            agent_selector: Some("consumer-1".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        plan.report.apds.status,
        crate::harness_plan::DefinitionStatus::Conformant
    );
    assert_eq!(plan.report.status, PreflightStatus::Failed);
    assert_eq!(
        plan.report.harness_support,
        crate::harness_plan::HarnessSupport::Limited
    );
    assert_eq!(
        plan.report.local_readiness,
        crate::harness_plan::LocalReadiness::NotReady
    );
    let tool_candidates = plan
        .capabilities
        .iter()
        .filter(|candidate| candidate.kind == "tool" && candidate.identity == "@fixture/tool")
        .collect::<Vec<_>>();
    assert!(!tool_candidates.is_empty());
    assert!(
        tool_candidates
            .iter()
            .all(|candidate| candidate.state == CapabilityState::Suppressed),
        "Ambiguous Tool candidates must not advertise arbitrary installed metadata as ready: {tool_candidates:?}"
    );
    assert!(
        plan.report
            .diagnostics
            .iter()
            .any(|d| d.code == "ambiguous_locked_component"
                && d.severity == PreflightDiagnosticSeverity::Fatal)
    );
    fs::remove_dir_all(root).unwrap();

    // Historical duplicate declarations still need safe runtime selection;
    // do not suppress the bound identity before the ambiguity guard sees it.
    let root = versioned_workspace("m6b-ambiguous-memory");
    let agent_path = root.join("agents/agent-1.agent.json");
    let mut agent: Value = serde_json::from_slice(&fs::read(&agent_path).unwrap()).unwrap();
    agent.as_object_mut().unwrap().remove("standard");
    agent["memory"] = json!(["@fixture/memory@1.0.0", "@fixture/memory@2.0.0"]);
    write_json(&agent_path, agent);
    let lock_path = root.join("agent.lock");
    let mut lock: Value = serde_json::from_slice(&fs::read(&lock_path).unwrap()).unwrap();
    let selected = &mut lock["roots"]["local:agent:agents/agent-1.agent.json"];
    selected["standard_origin"] = json!("legacy_inferred");
    selected["memory"] = json!([
        "memory:@fixture/memory@1.0.0",
        "memory:@fixture/memory@2.0.0"
    ]);
    write_json(&lock_path, lock);
    let plan = resolve_harness_plan(
        &root,
        &HarnessBootstrapOptions {
            agent_selector: Some("consumer-1".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        plan.report.apds.status,
        crate::harness_plan::DefinitionStatus::LegacyInferred
    );
    assert_eq!(plan.report.status, PreflightStatus::Failed);
    let memory_candidates = plan
        .capabilities
        .iter()
        .filter(|candidate| candidate.kind == "memory" && candidate.identity == "@fixture/memory")
        .collect::<Vec<_>>();
    assert!(!memory_candidates.is_empty());
    assert!(
        memory_candidates
            .iter()
            .all(|candidate| candidate.state == CapabilityState::Suppressed),
        "Ambiguous Memory candidates must not advertise an arbitrary installed version as pending/ready: {memory_candidates:?}"
    );
    assert!(
        plan.report
            .diagnostics
            .iter()
            .any(|d| d.code == "ambiguous_locked_component" && d.message.contains("memory"))
    );
    assert!(
        !plan
            .report
            .diagnostics
            .iter()
            .any(|d| d.code == "missing_installed_memory")
    );
    fs::remove_dir_all(root).unwrap();
}
