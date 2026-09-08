use super::*;

fn write_m14f_projected_memory_package(root: &std::path::Path) -> MemoryRecordTypeRuntimeSnapshot {
    std::fs::create_dir_all(root.join("schemas")).unwrap();
    std::fs::write(
        root.join("agent.json"),
        r#"{
  "kind": "memory",
  "name": "m14f-projected-memory-test",
  "version": "0.1.0",
  "description": "M14f hook projection test package.",
  "memory": {
    "scopes": {
      "user": { "description": "User scope." }
    },
    "record_types": {
      "note": {
        "version": "1.0.0",
        "description": "Durable note.",
        "schema": "schemas/note.schema.json"
      }
    },
    "spaces": {
      "notes": {
        "description": "Direct notes.",
        "model": "collection",
        "record_types": ["note"],
        "scope": ["user"],
        "retrieval": { "modes": ["key", "filter", "chronological"] }
      }
    }
  }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("schemas/note.schema.json"),
        r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "body": { "type": "string", "minLength": 1 },
    "scratch": {
      "type": "object",
      "properties": {
        "public": { "type": "string" },
        "private": {
          "type": "string",
          "x-agentpm-persist": false
        }
      },
      "additionalProperties": false
    }
  },
  "required": ["body"],
  "additionalProperties": false
}
"#,
    )
    .unwrap();
    crate::commands::memory::execute_memory_build(
        &root.join("agent.json"),
        crate::commands::memory::MemoryBuildMode::Write,
    )
    .unwrap();
    let contracts =
        crate::harness_runtime::memory::validate_and_load_memory_contracts(root).unwrap();
    MemoryRecordTypeRuntimeSnapshot {
        name: "note".into(),
        schema_version: "1.0.0".into(),
        content_schema: crate::harness_runtime::memory::generated_memory_content_schema(
            &contracts, "notes", "note",
        )
        .unwrap(),
    }
}

fn runtime_with_m14f_projected_memory(
    workspace: &std::path::Path,
    package_root: &std::path::Path,
    memory_runtime: &str,
) -> RuntimeSnapshot {
    let record_type = write_m14f_projected_memory_package(package_root);
    let mut runtime = RuntimeSnapshot::empty("session-test".into());
    runtime.workspace_root = workspace.to_path_buf();
    runtime.state_dir = workspace.join(".agentpm-state");
    runtime
        .runtime_scopes
        .insert("user".into(), "user-123".into());
    runtime.memory.push(MemorySpaceRuntimeSnapshot {
        package: "m14f-projected-memory-test".into(),
        package_version: "0.1.0".into(),
        space: "notes".into(),
        model: MemorySpaceModel::Collection,
        description: "Direct notes.".into(),
        root: Some(package_root.to_path_buf()),
        runtime: memory_runtime.into(),
        source: "agent_binding".into(),
        state: "available".into(),
        readiness_reason: None,
        binding_scope: "global".into(),
        scope_keys: vec!["user".into()],
        retrieval_modes: vec![
            MemoryRetrievalMode::Key,
            MemoryRetrievalMode::Filter,
            MemoryRetrievalMode::Chronological,
        ],
        semantic: None,
        append_only: false,
        record_types: vec![record_type],
    });
    runtime
}

fn write_m14f_engine_memory_package(
    root: &std::path::Path,
) -> Vec<MemoryRecordTypeRuntimeSnapshot> {
    std::fs::create_dir_all(root.join("schemas")).unwrap();
    std::fs::write(
        root.join("agent.json"),
        r#"{
  "kind": "memory",
  "name": "m14f-engine-memory-test",
  "version": "0.1.0",
  "description": "M14f Engine integration Memory test package.",
  "memory": {
    "scopes": {
      "user": { "description": "User scope." }
    },
    "record_types": {
      "note": {
        "version": "1.0.0",
        "description": "Durable note.",
        "schema": "schemas/note.schema.json"
      },
      "profile": {
        "version": "1.0.0",
        "description": "Current profile.",
        "schema": "schemas/profile.schema.json"
      },
      "event": {
        "version": "1.0.0",
        "description": "Timeline event.",
        "schema": "schemas/event.schema.json"
      }
    },
    "spaces": {
      "notes": {
        "description": "Direct notes.",
        "model": "collection",
        "record_types": ["note"],
        "scope": ["user"],
        "retrieval": { "modes": ["key", "filter", "chronological", "full_text"] }
      },
      "profile": {
        "description": "Current profile.",
        "model": "document",
        "record_types": ["profile"],
        "scope": ["user"],
        "retrieval": { "modes": ["key"] }
      },
      "timeline": {
        "description": "Ordered timeline.",
        "model": "sequence",
        "record_types": ["event"],
        "scope": ["user"],
        "retrieval": { "modes": ["key", "chronological"] }
      }
    }
  }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("schemas/note.schema.json"),
        r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "body": { "type": "string", "minLength": 1 }
  },
  "required": ["body"],
  "additionalProperties": false
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("schemas/profile.schema.json"),
        r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "name": { "type": "string", "minLength": 1 }
  },
  "required": ["name"],
  "additionalProperties": false
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("schemas/event.schema.json"),
        r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "body": { "type": "string", "minLength": 1 }
  },
  "required": ["body"],
  "additionalProperties": false
}
"#,
    )
    .unwrap();
    crate::commands::memory::execute_memory_build(
        &root.join("agent.json"),
        crate::commands::memory::MemoryBuildMode::Write,
    )
    .unwrap();
    let contracts =
        crate::harness_runtime::memory::validate_and_load_memory_contracts(root).unwrap();
    ["note", "profile", "event"]
        .into_iter()
        .map(|record_type| MemoryRecordTypeRuntimeSnapshot {
            name: record_type.into(),
            schema_version: "1.0.0".into(),
            content_schema: crate::harness_runtime::memory::generated_memory_content_schema(
                &contracts,
                match record_type {
                    "profile" => "profile",
                    "event" => "timeline",
                    _ => "notes",
                },
                record_type,
            )
            .unwrap(),
        })
        .collect()
}

fn runtime_with_m14f_engine_memory(
    workspace: &std::path::Path,
    package_root: &std::path::Path,
) -> RuntimeSnapshot {
    let mut record_types = write_m14f_engine_memory_package(package_root)
        .into_iter()
        .map(|record_type| (record_type.name.clone(), record_type))
        .collect::<BTreeMap<_, _>>();
    let mut runtime = RuntimeSnapshot::empty("session-test".into());
    runtime.workspace_root = workspace.to_path_buf();
    runtime.state_dir = workspace.join(".agentpm-state");
    runtime
        .runtime_scopes
        .insert("user".into(), "user-123".into());
    runtime.memory = vec![
        MemorySpaceRuntimeSnapshot {
            package: "m14f-engine-memory-test".into(),
            package_version: "0.1.0".into(),
            space: "notes".into(),
            model: MemorySpaceModel::Collection,
            description: "Direct notes.".into(),
            root: Some(package_root.to_path_buf()),
            runtime: "local".into(),
            source: "agent_binding".into(),
            state: "available".into(),
            readiness_reason: None,
            binding_scope: "global".into(),
            scope_keys: vec!["user".into()],
            retrieval_modes: vec![
                MemoryRetrievalMode::Key,
                MemoryRetrievalMode::Filter,
                MemoryRetrievalMode::Chronological,
                MemoryRetrievalMode::FullText,
            ],
            semantic: None,
            append_only: false,
            record_types: vec![record_types.remove("note").unwrap()],
        },
        MemorySpaceRuntimeSnapshot {
            package: "m14f-engine-memory-test".into(),
            package_version: "0.1.0".into(),
            space: "profile".into(),
            model: MemorySpaceModel::Document,
            description: "Current profile.".into(),
            root: Some(package_root.to_path_buf()),
            runtime: "local".into(),
            source: "agent_binding".into(),
            state: "available".into(),
            readiness_reason: None,
            binding_scope: "global".into(),
            scope_keys: vec!["user".into()],
            retrieval_modes: vec![MemoryRetrievalMode::Key],
            semantic: None,
            append_only: false,
            record_types: vec![record_types.remove("profile").unwrap()],
        },
        MemorySpaceRuntimeSnapshot {
            package: "m14f-engine-memory-test".into(),
            package_version: "0.1.0".into(),
            space: "timeline".into(),
            model: MemorySpaceModel::Sequence,
            description: "Ordered timeline.".into(),
            root: Some(package_root.to_path_buf()),
            runtime: "local".into(),
            source: "agent_binding".into(),
            state: "available".into(),
            readiness_reason: None,
            binding_scope: "global".into(),
            scope_keys: vec!["user".into()],
            retrieval_modes: vec![MemoryRetrievalMode::Key, MemoryRetrievalMode::Chronological],
            semantic: None,
            append_only: false,
            record_types: vec![record_types.remove("event").unwrap()],
        },
    ];
    runtime
}

fn snapshot_file_tree(root: &std::path::Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(
        base: &std::path::Path,
        path: &std::path::Path,
        snapshot: &mut BTreeMap<String, Vec<u8>>,
    ) {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let file_type = entry.file_type().unwrap();
            if file_type.is_dir() {
                visit(base, &path, snapshot);
            } else if file_type.is_file() {
                let relative = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                snapshot.insert(relative, std::fs::read(&path).unwrap());
            }
        }
    }

    let mut snapshot = BTreeMap::new();
    if root.exists() {
        visit(root, root, &mut snapshot);
    }
    snapshot
}

#[test]
fn memory_descriptors_require_bound_ready_space_and_trusted_scope() {
    let temp = temp_workspace_dir("m14c-descriptors");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let effective =
        EffectivePhase::from_phase(&one_phase_memory_loop(None).r#loop.phases[0], &runtime);
    let notes_read = effective
        .capability_catalog
        .iter()
        .find(|descriptor| {
            descriptor.action_kind == "memory_read"
                && descriptor.identity == "m14c-memory-test/notes"
        })
        .expect("notes read descriptor");
    assert!(notes_read.description.contains("For collection spaces, key requires record_id; use filter, chronological, or full_text to find/list records when available."));
    assert!(effective.capability_catalog.iter().any(|descriptor| {
        descriptor.action_kind == "memory_write" && descriptor.identity == "m14c-memory-test/notes"
    }));

    let missing_scope_runtime =
        runtime_with_m14c_memory(&temp, &package_root, "global", "available", false);
    let missing_scope = EffectivePhase::from_phase(
        &one_phase_memory_loop(None).r#loop.phases[0],
        &missing_scope_runtime,
    );
    assert!(
        !missing_scope
            .capability_catalog
            .iter()
            .any(|descriptor| descriptor.action_kind == "memory_read")
    );
    assert!(
        missing_scope
            .suppressed_capabilities
            .iter()
            .any(|suppressed| {
                suppressed.kind == "memory_read"
                    && suppressed.reason.contains("unresolved Memory scope keys")
            })
    );

    let read_disabled_loop = one_phase_memory_loop(Some(LoopPhaseAccess {
        tools: None,
        knowledge: None,
        memory: Some(LoopAccessMemory {
            read: Some(false),
            write: Some(true),
        }),
    }));
    let write_only = EffectivePhase::from_phase(&read_disabled_loop.r#loop.phases[0], &runtime);
    assert!(
        !write_only
            .capability_catalog
            .iter()
            .any(|descriptor| descriptor.action_kind == "memory_read")
    );
    assert!(
        write_only
            .capability_catalog
            .iter()
            .any(|descriptor| descriptor.action_kind == "memory_write")
    );
}

#[test]
fn memory_descriptors_union_global_and_phase_scoped_bindings() {
    let temp = temp_workspace_dir("m14c-memory-union");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let mut runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut phase_memory = runtime.memory[0].clone();
    phase_memory.space = "phase_notes".into();
    phase_memory.description = "Phase notes.".into();
    phase_memory.binding_scope = "phase:remember".into();
    let mut other_phase_memory = runtime.memory[0].clone();
    other_phase_memory.space = "other_phase_notes".into();
    other_phase_memory.description = "Other phase notes.".into();
    other_phase_memory.binding_scope = "phase:other".into();
    runtime.memory.push(phase_memory);
    runtime.memory.push(other_phase_memory);

    let effective =
        EffectivePhase::from_phase(&one_phase_memory_loop(None).r#loop.phases[0], &runtime);
    let identities = effective
        .capability_catalog
        .iter()
        .filter(|descriptor| descriptor.action_kind == "memory_read")
        .map(|descriptor| descriptor.identity.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        identities,
        vec!["m14c-memory-test/notes", "m14c-memory-test/phase_notes"]
    );
    assert!(effective.active_memory.iter().any(|memory| {
        memory.package == "m14c-memory-test"
            && memory.space == "notes"
            && memory.binding_scope == "global"
    }));
    assert!(effective.active_memory.iter().any(|memory| {
        memory.package == "m14c-memory-test"
            && memory.space == "phase_notes"
            && memory.binding_scope == "phase:remember"
    }));
    assert!(
        !effective
            .active_memory
            .iter()
            .any(|memory| memory.space == "other_phase_notes")
    );
}

#[test]
fn memory_read_descriptors_explain_key_mode_by_space_model() {
    let temp = temp_workspace_dir("m14c-read-key-descriptors");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let record_type = write_m14c_memory_package(&package_root);
    let mut runtime = RuntimeSnapshot::empty("session-test".into());
    runtime.workspace_root = temp.to_path_buf();
    runtime.state_dir = temp.join(".agentpm-state");
    runtime
        .runtime_scopes
        .insert("user".into(), "user-123".into());
    runtime.memory = vec![
        MemorySpaceRuntimeSnapshot {
            package: "m14c-memory-test".into(),
            package_version: "0.1.0".into(),
            space: "session".into(),
            model: MemorySpaceModel::Document,
            description: "Current session.".into(),
            root: Some(package_root.to_path_buf()),
            runtime: "local".into(),
            source: "agent_binding".into(),
            state: "available".into(),
            readiness_reason: None,
            binding_scope: "global".into(),
            scope_keys: vec!["user".into()],
            retrieval_modes: vec![MemoryRetrievalMode::Key],
            semantic: None,
            append_only: false,
            record_types: vec![record_type.clone()],
        },
        MemorySpaceRuntimeSnapshot {
            package: "m14c-memory-test".into(),
            package_version: "0.1.0".into(),
            space: "log".into(),
            model: MemorySpaceModel::Sequence,
            description: "Ordered log.".into(),
            root: Some(package_root.to_path_buf()),
            runtime: "local".into(),
            source: "agent_binding".into(),
            state: "available".into(),
            readiness_reason: None,
            binding_scope: "global".into(),
            scope_keys: vec!["user".into()],
            retrieval_modes: vec![MemoryRetrievalMode::Key, MemoryRetrievalMode::Chronological],
            semantic: None,
            append_only: false,
            record_types: vec![record_type],
        },
    ];
    let effective =
        EffectivePhase::from_phase(&one_phase_memory_loop(None).r#loop.phases[0], &runtime);

    let document_read = effective
        .capability_catalog
        .iter()
        .find(|descriptor| {
            descriptor.action_kind == "memory_read"
                && descriptor.identity == "m14c-memory-test/session"
        })
        .expect("document read descriptor");
    assert!(document_read.description.contains(
        "For document spaces, key reads the current scoped document and does not require record_id."
    ));
    let sequence_read = effective
        .capability_catalog
        .iter()
        .find(|descriptor| {
            descriptor.action_kind == "memory_read" && descriptor.identity == "m14c-memory-test/log"
        })
        .expect("sequence read descriptor");
    assert!(sequence_read.description.contains(
        "For sequence spaces, key requires record_id; use chronological to find/list records when available."
    ));
    assert!(!sequence_read.description.contains("filter"));
    assert!(!sequence_read.description.contains("full_text"));
}

#[test]
fn direct_memory_actions_route_to_local_runtime_and_phase_transcript() {
    let temp = temp_workspace_dir("m14c-direct");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "write",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "note".into(),
                    record_id: None,
                    content: Some(json!({
                        "body": "Alpha launch checklist",
                        "labels": ["alpha", "release"],
                        "assignee": { "team": "platform" }
                    })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "read",
                SemanticAction::MemoryRead {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    mode: MemoryReadMode::Filter,
                    record_id: None,
                    record_type: Some("note".into()),
                    filter: BTreeMap::from([("labels".into(), json!("release"))]),
                    query: None,
                    limit: Some(1),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let result = engine
        .execute_run(
            &mut session,
            "remember this",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 2);
    assert_eq!(result.report.memory_summaries.len(), 2);
    assert!(
        dispatcher.dispatched.is_empty(),
        "Memory must not use fake dispatcher path"
    );

    let request_after_write = &model.requests[1];
    let transcript_text = request_after_write.prompt.render_text();
    assert!(transcript_text.contains("ActionResult [memory_write m14c-memory-test/notes]"));
    assert!(transcript_text.contains(SUCCESSFUL_ACTION_RESULT_CONTROL));
    let request_after_read = &model.requests[2];
    let transcript_text = request_after_read.prompt.render_text();
    assert!(transcript_text.contains("ActionResult [memory_read m14c-memory-test/notes]"));
    assert!(transcript_text.contains("Alpha launch checklist"));

    let events = handle.events();
    assert!(
        events
            .iter()
            .any(|event| event.event_type == HarnessEventType::MemoryWriteStarted)
    );
    assert!(
        events
            .iter()
            .any(|event| event.event_type == HarnessEventType::MemoryWriteCompleted)
    );
    assert!(
        events
            .iter()
            .any(|event| event.event_type == HarnessEventType::MemoryReadStarted)
    );
    assert!(
        events
            .iter()
            .any(|event| event.event_type == HarnessEventType::MemoryReadCompleted)
    );
    let write_completed = events
        .iter()
        .find(|event| event.event_type == HarnessEventType::MemoryWriteCompleted)
        .expect("memory write completed event");
    let HarnessEventPayload::Action { fields, .. } = &write_completed.payload else {
        panic!("expected action payload");
    };
    let provenance = &fields["result"]["record"]["provenance"]["harness"];
    assert_eq!(provenance["kind"], json!("harness_direct_memory_write"));
    assert_eq!(provenance["run_id"], json!(result.report.run_id));
    assert_eq!(provenance["phase_execution_id"], json!("phase-exec-1"));
    assert_eq!(provenance["phase_id"], json!("remember"));
    assert_eq!(provenance["action_kind"], json!("memory_write"));
    assert_eq!(provenance["operation"], json!("create"));
    assert_eq!(provenance["source"], json!("agent_binding"));
}

#[test]
fn engine_memory_persists_across_session_restart_and_readback() {
    let temp = temp_workspace_dir("m14f-engine-restart-readback");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14f_engine_memory(&temp, &package_root);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let mut write_model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "write",
                SemanticAction::MemoryWrite {
                    package: "m14f-engine-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "note".into(),
                    record_id: None,
                    content: Some(json!({ "body": "Persistent note across restart" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let write_result = engine
        .execute_run(
            &mut session,
            "write persistent memory",
            &mut write_model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(write_result) = write_result else {
        panic!("expected terminal write result");
    };
    assert_eq!(
        write_result.report.terminal_status,
        HarnessTerminalStatus::Ended
    );

    let restarted_runtime = runtime_with_m14f_engine_memory(&temp, &package_root);
    let mut restarted_session = HarnessSession::with_runtime_snapshot(restarted_runtime);
    let mut read_model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "read",
                SemanticAction::MemoryRead {
                    package: "m14f-engine-memory-test".into(),
                    space: "notes".into(),
                    mode: MemoryReadMode::Filter,
                    record_id: None,
                    record_type: Some("note".into()),
                    filter: BTreeMap::from([(
                        "body".into(),
                        json!("Persistent note across restart"),
                    )]),
                    query: None,
                    limit: Some(1),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let read_result = engine
        .execute_run(
            &mut restarted_session,
            "read persistent memory",
            &mut read_model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(read_result) = read_result else {
        panic!("expected terminal read result");
    };
    assert_eq!(
        read_result.report.terminal_status,
        HarnessTerminalStatus::Ended
    );
    assert_eq!(read_result.report.usage.memory_requests, 1);
    assert!(
        read_model.requests[1]
            .prompt
            .render_text()
            .contains("Persistent note across restart")
    );
}

#[test]
fn engine_memory_run_keeps_installed_agentpm_package_tree_immutable() {
    let temp = temp_workspace_dir("m14f-engine-agentpm-immutability");
    let package_root = temp
        .join(".agentpm")
        .join("memory")
        .join("m14f-engine-memory-test")
        .join("0.1.0");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14f_engine_memory(&temp, &package_root);
    let installed_package_snapshot = snapshot_file_tree(&temp.join(".agentpm"));
    assert!(
        installed_package_snapshot.contains_key("memory/m14f-engine-memory-test/0.1.0/agent.json")
    );

    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "write",
                SemanticAction::MemoryWrite {
                    package: "m14f-engine-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "note".into(),
                    record_id: None,
                    content: Some(json!({ "body": "State belongs outside installed packages" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "read",
                SemanticAction::MemoryRead {
                    package: "m14f-engine-memory-test".into(),
                    space: "notes".into(),
                    mode: MemoryReadMode::Filter,
                    record_id: None,
                    record_type: Some("note".into()),
                    filter: BTreeMap::from([(
                        "body".into(),
                        json!("State belongs outside installed packages"),
                    )]),
                    query: None,
                    limit: Some(1),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let result = engine
        .execute_run(
            &mut session,
            "write and read installed memory package",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 2);
    assert_eq!(
        snapshot_file_tree(&temp.join(".agentpm")),
        installed_package_snapshot
    );
    assert!(temp.join(".agentpm-state").join("memory.sqlite3").exists());
    assert!(
        !snapshot_file_tree(&temp.join(".agentpm"))
            .keys()
            .any(|path| path.ends_with("memory.sqlite3"))
    );
}

#[test]
fn engine_memory_document_and_sequence_spaces_preserve_semantics() {
    let temp = temp_workspace_dir("m14f-engine-document-sequence");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14f_engine_memory(&temp, &package_root);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![
                SemanticActionProposal::new(
                    "profile-initial",
                    SemanticAction::MemoryWrite {
                        package: "m14f-engine-memory-test".into(),
                        space: "profile".into(),
                        operation: MemoryWriteOperation::Upsert,
                        record_type: "profile".into(),
                        record_id: None,
                        content: Some(json!({ "name": "Initial profile" })),
                    },
                ),
                SemanticActionProposal::new(
                    "profile-replacement",
                    SemanticAction::MemoryWrite {
                        package: "m14f-engine-memory-test".into(),
                        space: "profile".into(),
                        operation: MemoryWriteOperation::Upsert,
                        record_type: "profile".into(),
                        record_id: None,
                        content: Some(json!({ "name": "Replacement profile" })),
                    },
                ),
                SemanticActionProposal::new(
                    "profile-read",
                    SemanticAction::MemoryRead {
                        package: "m14f-engine-memory-test".into(),
                        space: "profile".into(),
                        mode: MemoryReadMode::Key,
                        record_id: None,
                        record_type: Some("profile".into()),
                        filter: BTreeMap::new(),
                        query: None,
                        limit: None,
                    },
                ),
                SemanticActionProposal::new(
                    "timeline-a",
                    SemanticAction::MemoryWrite {
                        package: "m14f-engine-memory-test".into(),
                        space: "timeline".into(),
                        operation: MemoryWriteOperation::Create,
                        record_type: "event".into(),
                        record_id: None,
                        content: Some(json!({ "body": "Timeline A" })),
                    },
                ),
                SemanticActionProposal::new(
                    "timeline-b",
                    SemanticAction::MemoryWrite {
                        package: "m14f-engine-memory-test".into(),
                        space: "timeline".into(),
                        operation: MemoryWriteOperation::Create,
                        record_type: "event".into(),
                        record_id: None,
                        content: Some(json!({ "body": "Timeline B" })),
                    },
                ),
                SemanticActionProposal::new(
                    "timeline-read",
                    SemanticAction::MemoryRead {
                        package: "m14f-engine-memory-test".into(),
                        space: "timeline".into(),
                        mode: MemoryReadMode::Chronological,
                        record_id: None,
                        record_type: Some("event".into()),
                        filter: BTreeMap::new(),
                        query: None,
                        limit: Some(2),
                    },
                ),
            ],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let result = engine
        .execute_run(
            &mut session,
            "exercise document and sequence memory",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 6);
    assert!(dispatcher.dispatched.is_empty());

    let events = handle.events();
    let profile_read = events
        .iter()
        .find(|event| {
            if event.event_type != HarnessEventType::MemoryReadCompleted {
                return false;
            }
            let HarnessEventPayload::Action { fields, .. } = &event.payload else {
                return false;
            };
            fields.get("space").and_then(Value::as_str) == Some("profile")
        })
        .expect("profile read completed");
    let HarnessEventPayload::Action { fields, .. } = &profile_read.payload else {
        panic!("expected profile read action payload");
    };
    assert_eq!(
        fields["result"]["records"][0]["content"]["name"],
        json!("Replacement profile")
    );

    let timeline_read = events
        .iter()
        .find(|event| {
            if event.event_type != HarnessEventType::MemoryReadCompleted {
                return false;
            }
            let HarnessEventPayload::Action { fields, .. } = &event.payload else {
                return false;
            };
            fields.get("space").and_then(Value::as_str) == Some("timeline")
        })
        .expect("timeline read completed");
    let HarnessEventPayload::Action { fields, .. } = &timeline_read.payload else {
        panic!("expected timeline read action payload");
    };
    assert_eq!(
        fields["result"]["records"][0]["content"]["body"],
        json!("Timeline A")
    );
    assert_eq!(
        fields["result"]["records"][1]["content"]["body"],
        json!("Timeline B")
    );
}

#[test]
fn engine_memory_archive_and_delete_remove_records_from_active_reads() {
    let temp = temp_workspace_dir("m14f-engine-archive-delete");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14f_engine_memory(&temp, &package_root);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut create_model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![
                SemanticActionProposal::new(
                    "archive-target",
                    SemanticAction::MemoryWrite {
                        package: "m14f-engine-memory-test".into(),
                        space: "notes".into(),
                        operation: MemoryWriteOperation::Create,
                        record_type: "note".into(),
                        record_id: None,
                        content: Some(json!({ "body": "Archive target" })),
                    },
                ),
                SemanticActionProposal::new(
                    "delete-target",
                    SemanticAction::MemoryWrite {
                        package: "m14f-engine-memory-test".into(),
                        space: "notes".into(),
                        operation: MemoryWriteOperation::Create,
                        record_type: "note".into(),
                        record_id: None,
                        content: Some(json!({ "body": "Delete target" })),
                    },
                ),
            ],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let create_result = engine
        .execute_run(
            &mut session,
            "create lifecycle targets",
            &mut create_model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(create_result) = create_result else {
        panic!("expected terminal create result");
    };
    assert_eq!(
        create_result.report.terminal_status,
        HarnessTerminalStatus::Ended
    );

    let created_ids = handle
        .events()
        .iter()
        .filter(|event| event.event_type == HarnessEventType::MemoryWriteCompleted)
        .map(|event| {
            let HarnessEventPayload::Action { fields, .. } = &event.payload else {
                panic!("expected memory write action payload");
            };
            fields["result"]["record_id"]
                .as_str()
                .expect("record_id")
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(created_ids.len(), 2);

    let mut lifecycle_model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![
                SemanticActionProposal::new(
                    "archive",
                    SemanticAction::MemoryWrite {
                        package: "m14f-engine-memory-test".into(),
                        space: "notes".into(),
                        operation: MemoryWriteOperation::Archive,
                        record_type: "note".into(),
                        record_id: Some(created_ids[0].clone()),
                        content: None,
                    },
                ),
                SemanticActionProposal::new(
                    "delete",
                    SemanticAction::MemoryWrite {
                        package: "m14f-engine-memory-test".into(),
                        space: "notes".into(),
                        operation: MemoryWriteOperation::Delete,
                        record_type: "note".into(),
                        record_id: Some(created_ids[1].clone()),
                        content: None,
                    },
                ),
                SemanticActionProposal::new(
                    "read-active",
                    SemanticAction::MemoryRead {
                        package: "m14f-engine-memory-test".into(),
                        space: "notes".into(),
                        mode: MemoryReadMode::Chronological,
                        record_id: None,
                        record_type: Some("note".into()),
                        filter: BTreeMap::new(),
                        query: None,
                        limit: Some(5),
                    },
                ),
            ],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let lifecycle_result = engine
        .execute_run(
            &mut session,
            "archive delete and read active memory",
            &mut lifecycle_model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(lifecycle_result) = lifecycle_result else {
        panic!("expected terminal lifecycle result");
    };
    assert_eq!(
        lifecycle_result.report.terminal_status,
        HarnessTerminalStatus::Ended
    );
    assert_eq!(lifecycle_result.report.usage.memory_requests, 3);

    let events = handle.events();
    assert!(events.iter().any(|event| {
        if event.event_type != HarnessEventType::MemoryWriteCompleted {
            return false;
        }
        let HarnessEventPayload::Action { fields, .. } = &event.payload else {
            return false;
        };
        fields["operation"] == "archive"
    }));
    assert!(events.iter().any(|event| {
        if event.event_type != HarnessEventType::MemoryWriteCompleted {
            return false;
        }
        let HarnessEventPayload::Action { fields, .. } = &event.payload else {
            return false;
        };
        fields["operation"] == "delete"
    }));
    let active_read = events
        .iter()
        .rev()
        .find(|event| event.event_type == HarnessEventType::MemoryReadCompleted)
        .expect("active read completed");
    let HarnessEventPayload::Action { fields, .. } = &active_read.payload else {
        panic!("expected active read action payload");
    };
    assert_eq!(fields["result"]["count"], json!(0));
    assert_eq!(
        fields["result"]["records"]
            .as_array()
            .expect("records")
            .len(),
        0
    );
}

#[test]
fn before_memory_write_hook_patches_content_before_runtime_dispatch() {
    let temp = temp_workspace_dir("m14f-before-memory-write");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "write",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "note".into(),
                    record_id: None,
                    content: Some(json!({ "body": "Original note" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "read",
                SemanticAction::MemoryRead {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    mode: MemoryReadMode::Filter,
                    record_id: None,
                    record_type: Some("note".into()),
                    filter: BTreeMap::from([("body".into(), json!("Hooked note"))]),
                    query: None,
                    limit: Some(1),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut knowledge = NoopKnowledgeRuntime;
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        active_hooks: vec![HarnessHookId::BeforeMemoryWrite],
        memory_write: Some(BeforeMemoryWriteDecision {
            content: Some(json!({ "body": "Hooked note", "labels": ["m14f"] })),
        }),
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let mut services = HarnessRuntimeServices {
        model: &mut model,
        dispatcher: &mut dispatcher,
        knowledge: &mut knowledge,
        memory: None,
        embedding_provider: None,
        approvals: &mut approvals,
        hooks: &mut hooks,
        service_events: None,
    };

    let result = engine
        .execute_run_with_id(
            &mut session,
            "run-m14f-write-hook".into(),
            "write memory",
            &mut services,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(hooks.memory_write_hooks.len(), 1);
    let hook_input = &hooks.memory_write_hooks[0];
    assert_eq!(hook_input.operation, "create");
    assert_eq!(hook_input.record_type, "note");
    assert_eq!(hook_input.scope["user"], json!("user-123"));
    assert_eq!(hook_input.content["body"], json!("Original note"));
    assert!(
        model.requests[2]
            .prompt
            .render_text()
            .contains("Hooked note")
    );
    let events = handle.events();
    assert!(
        events
            .iter()
            .any(|event| event.event_type == HarnessEventType::HookStarted)
    );
    assert!(events.iter().any(|event| {
        if event.event_type != HarnessEventType::HookCompleted {
            return false;
        }
        let HarnessEventPayload::Lifecycle { fields, .. } = &event.payload else {
            return false;
        };
        fields.get("hook").and_then(Value::as_str) == Some("before_memory_write")
            && fields.get("patched").and_then(Value::as_bool) == Some(true)
    }));
}

#[test]
fn before_memory_write_hook_patch_is_projected_before_custom_runtime_dispatch() {
    #[derive(Clone)]
    struct RecordingMemoryRuntime {
        requests: std::rc::Rc<std::cell::RefCell<Vec<Value>>>,
    }

    impl HostServiceInvoker for RecordingMemoryRuntime {
        fn invoke_host_service(
            &mut self,
            _role: &str,
            _registry_id: &str,
            _method: &str,
            payload: Value,
            _timeout_ms: u64,
        ) -> Result<Value> {
            self.requests.borrow_mut().push(payload.clone());
            Ok(json!({
                "ok": true,
                "package": payload["request"]["package"].clone(),
                "package_version": payload["request"]["package_version"].clone(),
                "space": payload["request"]["space"].clone(),
                "operation": payload["request"]["operation"].clone(),
                "record_id": "remote-1"
            }))
        }
    }

    let temp = temp_workspace_dir("m14f-memory-hook-custom-runtime-projection");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14f_projected_memory(&temp, &package_root, "remote-memory");
    let requests = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let custom_memory = CustomMemoryRuntime::new(
        runtime.memory.clone(),
        HashMap::from([(
            "remote-memory".into(),
            ServiceRuntime::host(
                Box::new(RecordingMemoryRuntime {
                    requests: requests.clone(),
                }),
                1_000,
            ),
        )]),
    );
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "write",
                SemanticAction::MemoryWrite {
                    package: "m14f-projected-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "note".into(),
                    record_id: None,
                    content: Some(json!({
                        "body": "Original note",
                        "scratch": {
                            "public": "original public scratch",
                            "private": "original private scratch"
                        }
                    })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut knowledge = NoopKnowledgeRuntime;
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        active_hooks: vec![HarnessHookId::BeforeMemoryWrite],
        memory_write: Some(BeforeMemoryWriteDecision {
            content: Some(json!({
                "body": "Hooked note",
                "scratch": {
                    "public": "hook public scratch",
                    "private": "hook private scratch"
                }
            })),
        }),
        ..TestHookRuntime::default()
    };
    let mut services = HarnessRuntimeServices {
        model: &mut model,
        dispatcher: &mut dispatcher,
        knowledge: &mut knowledge,
        memory: Some(custom_memory),
        embedding_provider: None,
        approvals: &mut approvals,
        hooks: &mut hooks,
        service_events: None,
    };
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );

    let result = engine
        .execute_run_with_id(
            &mut session,
            "run-m14f-memory-hook-custom-runtime-projection".into(),
            "write memory",
            &mut services,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(hooks.memory_write_hooks.len(), 1);
    assert_eq!(
        hooks.memory_write_hooks[0].content["scratch"]["private"],
        json!("original private scratch")
    );

    let requests = requests.borrow();
    assert_eq!(requests.len(), 1);
    let request_content = &requests[0]["request"]["content"];
    assert_eq!(request_content["body"], json!("Hooked note"));
    assert_eq!(
        request_content["scratch"]["public"],
        json!("hook public scratch")
    );
    assert!(request_content["scratch"].get("private").is_none());
    assert!(requests[0].to_string().contains("Hooked note"));
    assert!(!requests[0].to_string().contains("hook private scratch"));
    assert!(!requests[0].to_string().contains("original private scratch"));
}

#[test]
fn before_memory_read_hook_patches_request_within_active_space() {
    let temp = temp_workspace_dir("m14f-before-memory-read");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "write-beta",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "note".into(),
                    record_id: None,
                    content: Some(json!({ "body": "Beta note" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "read",
                SemanticAction::MemoryRead {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    mode: MemoryReadMode::Chronological,
                    record_id: None,
                    record_type: Some("note".into()),
                    filter: BTreeMap::new(),
                    query: None,
                    limit: Some(2),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut knowledge = NoopKnowledgeRuntime;
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        active_hooks: vec![HarnessHookId::BeforeMemoryRead],
        memory_read: Some(BeforeMemoryReadDecision {
            mode: Some("filter".into()),
            filter: Some(json!({ "body": "Beta note" })),
            limit: Some(1),
            ..BeforeMemoryReadDecision::default()
        }),
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let mut services = HarnessRuntimeServices {
        model: &mut model,
        dispatcher: &mut dispatcher,
        knowledge: &mut knowledge,
        memory: None,
        embedding_provider: None,
        approvals: &mut approvals,
        hooks: &mut hooks,
        service_events: None,
    };

    let result = engine
        .execute_run_with_id(
            &mut session,
            "run-m14f-read-hook".into(),
            "read memory",
            &mut services,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(hooks.memory_read_hooks.len(), 1);
    let hook_input = &hooks.memory_read_hooks[0];
    assert_eq!(hook_input.mode.as_deref(), Some("chronological"));
    assert_eq!(
        hook_input.retrieval_modes,
        vec!["key", "filter", "chronological", "full_text"]
    );
    let prompt = model.requests[2].prompt.render_text();
    assert!(prompt.contains("Beta note"));
}

#[test]
fn before_memory_read_hook_rejection_fails_closed_before_runtime_dispatch() {
    let temp = temp_workspace_dir("m14f-memory-read-hook-reject");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![ModelTurn {
        assistant_content: None,
        actions: vec![SemanticActionProposal::new(
            "read",
            SemanticAction::MemoryRead {
                package: "m14c-memory-test".into(),
                space: "notes".into(),
                mode: MemoryReadMode::Filter,
                record_id: None,
                record_type: Some("note".into()),
                filter: BTreeMap::from([("body".into(), json!("Must not read"))]),
                query: None,
                limit: Some(1),
            },
        )],
        usage: RunUsage::default(),
        finish_reason: Some("tool_calls".into()),
        provider_metadata: BTreeMap::new(),
    }]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut knowledge = NoopKnowledgeRuntime;
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        active_hooks: vec![HarnessHookId::BeforeMemoryRead],
        reject_before_memory_read: Some("policy denied memory read".into()),
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let mut services = HarnessRuntimeServices {
        model: &mut model,
        dispatcher: &mut dispatcher,
        knowledge: &mut knowledge,
        memory: None,
        embedding_provider: None,
        approvals: &mut approvals,
        hooks: &mut hooks,
        service_events: None,
    };

    let result = engine
        .execute_run_with_id(
            &mut session,
            "run-m14f-read-hook-reject".into(),
            "read memory",
            &mut services,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Failed);
    assert_eq!(hooks.memory_read_hooks.len(), 1);
    let events = handle.events();
    assert!(events.iter().any(|event| {
        if event.event_type != HarnessEventType::HookRejected {
            return false;
        }
        let HarnessEventPayload::Lifecycle { fields, message } = &event.payload else {
            return false;
        };
        fields.get("hook").and_then(Value::as_str) == Some("before_memory_read")
            && message == "policy denied memory read"
    }));
    assert!(
        !events
            .iter()
            .any(|event| event.event_type == HarnessEventType::MemoryReadStarted)
    );
}

#[test]
fn before_memory_read_hook_rejection_emits_queued_nonfatal_failure_first() {
    let temp = temp_workspace_dir("m14f-memory-read-hook-nonfatal-before-reject");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![ModelTurn {
        assistant_content: None,
        actions: vec![SemanticActionProposal::new(
            "read",
            SemanticAction::MemoryRead {
                package: "m14c-memory-test".into(),
                space: "notes".into(),
                mode: MemoryReadMode::Filter,
                record_id: None,
                record_type: Some("note".into()),
                filter: BTreeMap::from([("body".into(), json!("Must not read"))]),
                query: None,
                limit: Some(1),
            },
        )],
        usage: RunUsage::default(),
        finish_reason: Some("tool_calls".into()),
        provider_metadata: BTreeMap::new(),
    }]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut knowledge = NoopKnowledgeRuntime;
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        active_hooks: vec![HarnessHookId::BeforeMemoryRead],
        nonfatal_before_memory_read: Some("nonfatal memory read hook warning".into()),
        reject_before_memory_read: Some("policy denied memory read".into()),
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let mut services = HarnessRuntimeServices {
        model: &mut model,
        dispatcher: &mut dispatcher,
        knowledge: &mut knowledge,
        memory: None,
        embedding_provider: None,
        approvals: &mut approvals,
        hooks: &mut hooks,
        service_events: None,
    };

    let result = engine
        .execute_run_with_id(
            &mut session,
            "run-m14f-read-hook-nonfatal-before-reject".into(),
            "read memory",
            &mut services,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Failed);
    let events = handle.events();
    let event_types = events
        .iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    let hook_failed_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookFailed)
        .expect("queued nonfatal hook failed event");
    let hook_rejected_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookRejected)
        .expect("terminal hook rejected event");
    assert!(hook_failed_position < hook_rejected_position);
    assert_eq!(
        hook_event_fields_for(&events, HarnessEventType::HookFailed, "before_memory_read")["nonfatal"],
        json!(true)
    );
    assert!(
        !event_types
            .iter()
            .any(|event_type| *event_type == HarnessEventType::MemoryReadStarted)
    );
}

#[test]
fn before_memory_read_hook_undeclared_mode_patch_is_revalidated_before_dispatch() {
    let temp = temp_workspace_dir("m14f-memory-read-hook-mode-revalidation");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![ModelTurn {
        assistant_content: None,
        actions: vec![SemanticActionProposal::new(
            "read",
            SemanticAction::MemoryRead {
                package: "m14c-memory-test".into(),
                space: "notes".into(),
                mode: MemoryReadMode::Chronological,
                record_id: None,
                record_type: Some("note".into()),
                filter: BTreeMap::new(),
                query: None,
                limit: Some(1),
            },
        )],
        usage: RunUsage::default(),
        finish_reason: Some("tool_calls".into()),
        provider_metadata: BTreeMap::new(),
    }]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut knowledge = NoopKnowledgeRuntime;
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        active_hooks: vec![HarnessHookId::BeforeMemoryRead],
        memory_read: Some(BeforeMemoryReadDecision {
            mode: Some("semantic".into()),
            query: Some("alpha semantic".into()),
            limit: Some(1),
            ..BeforeMemoryReadDecision::default()
        }),
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let mut services = HarnessRuntimeServices {
        model: &mut model,
        dispatcher: &mut dispatcher,
        knowledge: &mut knowledge,
        memory: None,
        embedding_provider: None,
        approvals: &mut approvals,
        hooks: &mut hooks,
        service_events: None,
    };

    let result = engine
        .execute_run_with_id(
            &mut session,
            "run-m14f-read-hook-mode-revalidation".into(),
            "read memory",
            &mut services,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Failed);
    assert_eq!(hooks.memory_read_hooks.len(), 1);
    let events = handle.events();
    assert!(events.iter().any(|event| {
        if event.event_type != HarnessEventType::HookFailed {
            return false;
        }
        let HarnessEventPayload::Lifecycle { message, .. } = &event.payload else {
            return false;
        };
        message.contains("semantic")
    }));
    assert!(
        !events
            .iter()
            .any(|event| event.event_type == HarnessEventType::MemoryReadStarted)
    );
}

#[test]
fn before_memory_read_hook_malformed_filter_patch_fails_before_dispatch() {
    let temp = temp_workspace_dir("m14f-memory-read-hook-filter-revalidation");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![ModelTurn {
        assistant_content: None,
        actions: vec![SemanticActionProposal::new(
            "read",
            SemanticAction::MemoryRead {
                package: "m14c-memory-test".into(),
                space: "notes".into(),
                mode: MemoryReadMode::Chronological,
                record_id: None,
                record_type: Some("note".into()),
                filter: BTreeMap::new(),
                query: None,
                limit: Some(1),
            },
        )],
        usage: RunUsage::default(),
        finish_reason: Some("tool_calls".into()),
        provider_metadata: BTreeMap::new(),
    }]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut knowledge = NoopKnowledgeRuntime;
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        active_hooks: vec![HarnessHookId::BeforeMemoryRead],
        memory_read: Some(BeforeMemoryReadDecision {
            mode: Some("filter".into()),
            filter: Some(json!("not an object")),
            limit: Some(1),
            ..BeforeMemoryReadDecision::default()
        }),
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let mut services = HarnessRuntimeServices {
        model: &mut model,
        dispatcher: &mut dispatcher,
        knowledge: &mut knowledge,
        memory: None,
        embedding_provider: None,
        approvals: &mut approvals,
        hooks: &mut hooks,
        service_events: None,
    };

    let result = engine
        .execute_run_with_id(
            &mut session,
            "run-m14f-read-hook-filter-revalidation".into(),
            "read memory",
            &mut services,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Failed);
    assert_eq!(hooks.memory_read_hooks.len(), 1);
    let events = handle.events();
    assert!(events.iter().any(|event| {
        if event.event_type != HarnessEventType::HookFailed {
            return false;
        }
        let HarnessEventPayload::Lifecycle { message, .. } = &event.payload else {
            return false;
        };
        message.contains("invalid filter")
    }));
    assert!(
        !events
            .iter()
            .any(|event| event.event_type == HarnessEventType::MemoryReadStarted)
    );
}

#[test]
fn before_memory_write_hook_rejection_fails_closed_before_mutation() {
    let temp = temp_workspace_dir("m14f-memory-hook-reject");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![ModelTurn {
        assistant_content: None,
        actions: vec![SemanticActionProposal::new(
            "write",
            SemanticAction::MemoryWrite {
                package: "m14c-memory-test".into(),
                space: "notes".into(),
                operation: MemoryWriteOperation::Create,
                record_type: "note".into(),
                record_id: None,
                content: Some(json!({ "body": "Must not persist" })),
            },
        )],
        usage: RunUsage::default(),
        finish_reason: Some("tool_calls".into()),
        provider_metadata: BTreeMap::new(),
    }]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut knowledge = NoopKnowledgeRuntime;
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        active_hooks: vec![HarnessHookId::BeforeMemoryWrite],
        reject_before_memory_write: Some("policy denied memory write".into()),
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let mut services = HarnessRuntimeServices {
        model: &mut model,
        dispatcher: &mut dispatcher,
        knowledge: &mut knowledge,
        memory: None,
        embedding_provider: None,
        approvals: &mut approvals,
        hooks: &mut hooks,
        service_events: None,
    };

    let result = engine
        .execute_run_with_id(
            &mut session,
            "run-m14f-hook-reject".into(),
            "write memory",
            &mut services,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Failed);
    let events = handle.events();
    assert!(
        events
            .iter()
            .any(|event| event.event_type == HarnessEventType::HookRejected)
    );
    assert!(
        !events
            .iter()
            .any(|event| event.event_type == HarnessEventType::MemoryWriteStarted)
    );
}

#[test]
fn before_memory_write_hook_patch_is_revalidated_before_mutation() {
    let temp = temp_workspace_dir("m14f-memory-hook-revalidation");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![ModelTurn {
        assistant_content: None,
        actions: vec![SemanticActionProposal::new(
            "write",
            SemanticAction::MemoryWrite {
                package: "m14c-memory-test".into(),
                space: "notes".into(),
                operation: MemoryWriteOperation::Create,
                record_type: "note".into(),
                record_id: None,
                content: Some(json!({ "body": "Valid before hook" })),
            },
        )],
        usage: RunUsage::default(),
        finish_reason: Some("tool_calls".into()),
        provider_metadata: BTreeMap::new(),
    }]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut knowledge = NoopKnowledgeRuntime;
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        active_hooks: vec![HarnessHookId::BeforeMemoryWrite],
        memory_write: Some(BeforeMemoryWriteDecision {
            content: Some(json!({ "labels": ["missing required body"] })),
        }),
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let mut services = HarnessRuntimeServices {
        model: &mut model,
        dispatcher: &mut dispatcher,
        knowledge: &mut knowledge,
        memory: None,
        embedding_provider: None,
        approvals: &mut approvals,
        hooks: &mut hooks,
        service_events: None,
    };

    let result = engine
        .execute_run_with_id(
            &mut session,
            "run-m14f-hook-revalidate".into(),
            "write memory",
            &mut services,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Failed);
    let events = handle.events();
    assert!(events.iter().any(|event| {
        if event.event_type != HarnessEventType::HookFailed {
            return false;
        }
        let HarnessEventPayload::Lifecycle { message, .. } = &event.payload else {
            return false;
        };
        message.contains("Memory content")
    }));
    assert!(
        !events
            .iter()
            .any(|event| event.event_type == HarnessEventType::MemoryWriteStarted)
    );
}

#[test]
fn semantic_memory_read_reports_embedding_usage_and_events() {
    let temp = temp_workspace_dir("m14d-semantic-memory-usage");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let mut runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let record_type = runtime.memory[0].record_types[0].clone();
    enable_m14c_memory_package_semantic(&package_root);
    runtime.memory[0]
        .retrieval_modes
        .push(MemoryRetrievalMode::Semantic);
    runtime.memory[0].semantic = Some(KnowledgeEmbeddingSnapshot {
        id: "memory.local.semantic".into(),
        provider: "test".into(),
        model: "toy-2d".into(),
        dimensions: 2,
        metric: "cosine".into(),
        normalized: true,
    });
    runtime.memory[0].record_types = vec![record_type];
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "write",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "note".into(),
                    record_id: None,
                    content: Some(json!({ "body": "Alpha semantic note" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "semantic-read",
                SemanticAction::MemoryRead {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    mode: MemoryReadMode::Semantic,
                    record_id: None,
                    record_type: Some("note".into()),
                    filter: BTreeMap::new(),
                    query: Some("alpha semantic".into()),
                    limit: Some(1),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut knowledge = NoopKnowledgeRuntime;
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = NoopHookRuntime;
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let mut services = HarnessRuntimeServices {
        model: &mut model,
        dispatcher: &mut dispatcher,
        knowledge: &mut knowledge,
        memory: None,
        embedding_provider: Some(Box::new(TestMemoryEmbeddingProvider::default())),
        approvals: &mut approvals,
        hooks: &mut hooks,
        service_events: None,
    };
    let result = engine
        .execute_run_with_id(
            &mut session,
            "run-semantic-memory".into(),
            "remember semantic note",
            &mut services,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };

    assert_eq!(result.report.usage.memory_requests, 2);
    assert_eq!(result.report.usage.embedding_requests, 2);
    assert_eq!(session.usage.embedding_requests, 2);
    let events = handle.events();
    assert!(
        events
            .iter()
            .any(|event| event.event_type == HarnessEventType::EmbeddingRequestStarted)
    );
    assert!(
        events
            .iter()
            .any(|event| event.event_type == HarnessEventType::EmbeddingRequestCompleted)
    );
    let completed_fields = events
        .iter()
        .filter(|event| event.event_type == HarnessEventType::EmbeddingRequestCompleted)
        .map(|event| {
            let HarnessEventPayload::Action { fields, .. } = &event.payload else {
                panic!("expected embedding action payload");
            };
            fields
        })
        .collect::<Vec<_>>();
    assert_eq!(completed_fields.len(), 2);
    assert!(completed_fields.iter().all(|fields| {
        fields["package"] == "m14c-memory-test"
            && fields["space"] == "notes"
            && fields["provider"] == "test"
            && fields["model"] == "toy-2d"
            && fields["embedding_requests"] == 1
            && fields["duration_ms"].as_u64().is_some()
    }));
}

#[test]
fn simplified_provider_memory_content_still_receives_authoritative_validation() {
    let temp = temp_workspace_dir("m14c-schema-simplification-validation");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let mut runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    runtime.memory[0]
        .record_types
        .push(m14c_task_record_type_snapshot(&package_root));
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "provider-compatible-but-contract-invalid",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "note".into(),
                    record_id: None,
                    content: Some(json!({
                        "title": "valid for task, invalid for note"
                    })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let result = engine
        .execute_run(
            &mut session,
            "invalid memory",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 0);
    assert!(dispatcher.dispatched.is_empty());
    assert!(
        !handle
            .events()
            .iter()
            .any(|event| event.event_type == HarnessEventType::MemoryWriteStarted)
    );
    assert!(handle.events().iter().any(|event| {
        event.event_type == HarnessEventType::SemanticActionRejected
            && matches!(
                &event.payload,
                HarnessEventPayload::Action { status, .. } if status == "invalid_arguments"
            )
    }));
    assert!(
        model.requests[1]
            .prompt
            .render_text()
            .contains("Memory content")
    );
}

#[test]
fn memory_capacity_overflow_returns_typed_structured_failure() {
    let temp = temp_workspace_dir("m14c-memory-capacity");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "write-1",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "note".into(),
                    record_id: None,
                    content: Some(json!({ "body": "first note" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "write-2",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "note".into(),
                    record_id: None,
                    content: Some(json!({ "body": "second note" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let result = engine
        .execute_run(
            &mut session,
            "overflow memory capacity",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 2);
    assert!(dispatcher.dispatched.is_empty());
    assert!(
        !handle
            .events()
            .iter()
            .any(|event| event.event_type == HarnessEventType::SemanticActionRejected)
    );
    assert!(handle.events().iter().any(|event| {
        if event.event_type != HarnessEventType::MemoryWriteFailed {
            return false;
        }
        let HarnessEventPayload::Action { fields, .. } = &event.payload else {
            return false;
        };
        fields
            .get("result")
            .and_then(|result| result.get("error"))
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str)
            == Some("capacity_exceeded")
    }));
    assert!(
        model.requests[2]
            .prompt
            .render_text()
            .contains("\"code\":\"capacity_exceeded\"")
    );
}

#[test]
fn memory_actions_count_against_action_limit_not_tool_limit() {
    let temp = temp_workspace_dir("m14c-memory-action-limit-tool-limit");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "write",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "note".into(),
                    record_id: None,
                    content: Some(json!({ "body": "allowed despite zero tool calls" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut runtime_limits = limits();
    runtime_limits.max_tool_calls_per_phase = 0;
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(runtime_limits),
    );
    let result = engine
        .execute_run(
            &mut session,
            "write memory",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 1);
    assert_eq!(result.report.usage.tool_calls, 0);
    assert!(dispatcher.dispatched.is_empty());

    let temp = temp_workspace_dir("m14c-memory-action-limit-exhaustion");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![ModelTurn {
        assistant_content: None,
        actions: vec![
            SemanticActionProposal::new(
                "write",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "note".into(),
                    record_id: None,
                    content: Some(json!({ "body": "first memory action" })),
                },
            ),
            SemanticActionProposal::new(
                "read",
                SemanticAction::MemoryRead {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    mode: MemoryReadMode::Filter,
                    record_id: None,
                    record_type: Some("note".into()),
                    filter: BTreeMap::from([("body".into(), json!("first memory action"))]),
                    query: None,
                    limit: Some(1),
                },
            ),
        ],
        usage: RunUsage::default(),
        finish_reason: Some("tool_calls".into()),
        provider_metadata: BTreeMap::new(),
    }]);
    let mut runtime_limits = limits();
    runtime_limits.max_actions_per_phase = 1;
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(runtime_limits),
    );
    let result = engine
        .execute_run(
            &mut session,
            "write then read memory",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal limit result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::LimitReached);
    assert_eq!(result.report.usage.memory_requests, 1);
    assert_eq!(result.report.usage.tool_calls, 0);
    assert_eq!(
        handle
            .events()
            .iter()
            .filter(|event| event.event_type == HarnessEventType::MemoryWriteCompleted)
            .count(),
        1
    );
    assert!(
        !handle
            .events()
            .iter()
            .any(|event| event.event_type == HarnessEventType::MemoryReadStarted)
    );
}

#[test]
fn invalid_memory_write_and_unknown_filter_path_request_repair_before_dispatch() {
    let temp = temp_workspace_dir("m14c-repair");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "bad-write",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "note".into(),
                    record_id: None,
                    content: Some(json!({ "labels": ["missing-body"] })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "bad-read",
                SemanticAction::MemoryRead {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    mode: MemoryReadMode::Filter,
                    record_id: None,
                    record_type: Some("note".into()),
                    filter: BTreeMap::from([("unknown.path".into(), json!("x"))]),
                    query: None,
                    limit: None,
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let result = engine
        .execute_run(
            &mut session,
            "invalid memory",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.usage.memory_requests, 0);
    assert!(dispatcher.dispatched.is_empty());
    let rejected = handle
        .events()
        .into_iter()
        .filter(|event| event.event_type == HarnessEventType::SemanticActionRejected)
        .count();
    assert_eq!(rejected, 2);
    assert!(
        model.requests[1]
            .prompt
            .render_text()
            .contains("Memory content")
    );
    assert!(
        model.requests[2]
            .prompt
            .render_text()
            .contains("Memory filter path `unknown.path`")
    );
}

#[test]
fn append_only_memory_write_rejects_mutation_before_dispatch() {
    let temp = temp_workspace_dir("m14c-append-only-repair");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let mut runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    runtime.memory[0].append_only = true;
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "bad-update",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Update,
                    record_type: "note".into(),
                    record_id: Some("mem_existing".into()),
                    content: Some(json!({ "body": "updated body" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let result = engine
        .execute_run(
            &mut session,
            "invalid append-only memory",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.usage.memory_requests, 0);
    assert!(dispatcher.dispatched.is_empty());
    assert!(handle.events().iter().any(|event| {
        if event.event_type != HarnessEventType::SemanticActionRejected {
            return false;
        }
        let HarnessEventPayload::Action { fields, .. } = &event.payload else {
            return false;
        };
        fields
            .get("error")
            .and_then(Value::as_str)
            .is_some_and(|error| error.contains("append-only"))
    }));
    assert!(
        model.requests[1]
            .prompt
            .render_text()
            .contains("append-only")
    );
}

#[test]
fn duplicate_document_create_requests_repair_after_runtime_lookup() {
    let temp = temp_workspace_dir("m14c-duplicate-document-create-repair");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let mut runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    runtime.memory.push(MemorySpaceRuntimeSnapshot {
        package: "m14c-memory-test".into(),
        package_version: "0.1.0".into(),
        space: "profile".into(),
        model: MemorySpaceModel::Document,
        description: "Single current profile.".into(),
        root: Some(package_root.to_path_buf()),
        runtime: "local".into(),
        source: "agent_binding".into(),
        state: "available".into(),
        readiness_reason: None,
        binding_scope: "global".into(),
        scope_keys: vec!["user".into()],
        retrieval_modes: vec![MemoryRetrievalMode::Key],
        semantic: None,
        append_only: false,
        record_types: vec![
            m14c_profile_record_type_snapshot(&package_root, "profile_a"),
            m14c_profile_record_type_snapshot(&package_root, "profile_b"),
        ],
    });
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "create-profile-a",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "profile".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "profile_a".into(),
                    record_id: None,
                    content: Some(json!({ "name": "A" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "duplicate-create-profile-b",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "profile".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "profile_b".into(),
                    record_id: None,
                    content: Some(json!({ "display": "B" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let result = engine
        .execute_run(
            &mut session,
            "duplicate document create",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 2);
    assert_eq!(result.report.repair_count, 1);
    assert!(dispatcher.dispatched.is_empty());
    let expected = "Memory document create for space `profile` requires no current document for the resolved scope";
    assert!(handle.events().iter().any(|event| {
        if event.event_type != HarnessEventType::SemanticActionRejected {
            return false;
        }
        let HarnessEventPayload::Action { fields, .. } = &event.payload else {
            return false;
        };
        fields
            .get("error")
            .and_then(Value::as_str)
            .is_some_and(|error| error.contains(expected))
    }));
    assert!(model.requests[2].prompt.render_text().contains(expected));

    let (manifest_value, _) = load_manifest_value(&package_root.join("agent.json")).unwrap();
    let manifest = parse_memory_manifest(&manifest_value).unwrap();
    let records = session
        .local_memory_runtime()
        .unwrap()
        .read_records(LocalMemoryReadRequest {
            package: "m14c-memory-test",
            package_version: "0.1.0",
            manifest: &manifest,
            space: "profile",
            scope: BTreeMap::from([("user".into(), "user-123".into())]),
            mode: LocalMemoryReadMode::Key,
            record_id: None,
            record_type: None,
            filter: BTreeMap::new(),
            query: None,
            limit: None,
            now: Utc::now(),
        })
        .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].record_type, "profile_a");
    assert_eq!(records[0].content, json!({ "name": "A" }));
}

#[test]
fn missing_memory_write_target_requests_repair_after_runtime_lookup() {
    let temp = temp_workspace_dir("m14c-missing-memory-target-repair");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "missing-update",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Update,
                    record_type: "note".into(),
                    record_id: Some("mem_missing".into()),
                    content: Some(json!({ "body": "updated body" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let result = engine
        .execute_run(
            &mut session,
            "missing memory target",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 1);
    assert!(dispatcher.dispatched.is_empty());
    assert!(handle.events().iter().any(|event| {
        if event.event_type != HarnessEventType::SemanticActionRejected {
            return false;
        }
        let HarnessEventPayload::Action { fields, .. } = &event.payload else {
            return false;
        };
        fields
            .get("error")
            .and_then(Value::as_str)
            .is_some_and(|error| error.contains("Memory record `mem_missing` was not found"))
    }));
    assert!(
        model.requests[1]
            .prompt
            .render_text()
            .contains("Memory record `mem_missing` was not found")
    );
}

#[test]
fn custom_memory_not_found_requests_repair_after_runtime_lookup() {
    #[derive(Clone)]
    struct NotFoundMemoryRuntime;

    impl HostServiceInvoker for NotFoundMemoryRuntime {
        fn invoke_host_service(
            &mut self,
            _role: &str,
            _registry_id: &str,
            _method: &str,
            _payload: Value,
            _timeout_ms: u64,
        ) -> Result<Value> {
            Ok(json!({
                "ok": false,
                "package": "m14c-memory-test",
                "package_version": "0.1.0",
                "space": "notes",
                "error": {
                    "code": "not_found",
                    "message": "Memory record `mem_missing` was not found"
                }
            }))
        }
    }

    let temp = temp_workspace_dir("m14e-custom-memory-not-found-repair");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let mut runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    runtime.memory[0].runtime = "remote-memory".into();
    let custom_memory = CustomMemoryRuntime::new(
        runtime.memory.clone(),
        HashMap::from([(
            "remote-memory".into(),
            ServiceRuntime::host(Box::new(NotFoundMemoryRuntime), 1_000),
        )]),
    );
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "missing-update",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Update,
                    record_type: "note".into(),
                    record_id: Some("mem_missing".into()),
                    content: Some(json!({ "body": "updated body" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut knowledge = NoopKnowledgeRuntime;
    let mut hooks = NoopHookRuntime;
    let mut services = HarnessRuntimeServices {
        model: &mut model,
        dispatcher: &mut dispatcher,
        knowledge: &mut knowledge,
        memory: Some(custom_memory),
        embedding_provider: None,
        approvals: &mut approvals,
        hooks: &mut hooks,
        service_events: None,
    };
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let result = engine
        .execute_run_with_id(
            &mut session,
            allocate_harness_run_id(),
            "missing custom memory target",
            &mut services,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 1);
    assert_eq!(result.report.repair_count, 1);
    assert!(dispatcher.dispatched.is_empty());
    assert!(handle.events().iter().any(|event| {
        if event.event_type != HarnessEventType::SemanticActionRejected {
            return false;
        }
        let HarnessEventPayload::Action { fields, .. } = &event.payload else {
            return false;
        };
        fields
            .get("error")
            .and_then(Value::as_str)
            .is_some_and(|error| error.contains("Memory record `mem_missing` was not found"))
    }));
    assert!(
        model.requests[1]
            .prompt
            .render_text()
            .contains("Memory record `mem_missing` was not found")
    );
}

#[test]
fn memory_write_target_record_type_mismatch_requests_repair_after_runtime_lookup() {
    let temp = temp_workspace_dir("m14c-memory-target-type-repair");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let mut runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    runtime.memory[0]
        .record_types
        .push(m14c_task_record_type_snapshot(&package_root));
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));

    let (manifest_value, _) = load_manifest_value(&package_root.join("agent.json")).unwrap();
    let manifest = parse_memory_manifest(&manifest_value).unwrap();
    let contracts =
        crate::harness_runtime::memory::validate_and_load_memory_contracts(&package_root).unwrap();
    let seeded = session
        .local_memory_runtime()
        .unwrap()
        .write_record(LocalMemoryWriteRequest {
            package: "m14c-memory-test",
            package_version: "0.1.0",
            manifest: &manifest,
            contracts: &contracts,
            space: "notes",
            record_type: "note",
            scope: BTreeMap::from([("user".into(), "user-123".into())]),
            operation: LocalMemoryWriteOperation::Create,
            record_id: None,
            content: Some(json!({ "body": "seed note" })),
            provenance: json!({}),
            now: Utc::now(),
        })
        .unwrap();
    let seeded_id = seeded.affected_record_id.unwrap();

    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "wrong-type-update",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Update,
                    record_type: "task".into(),
                    record_id: Some(seeded_id.clone()),
                    content: Some(json!({ "title": "retitled as task" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let result = engine
        .execute_run(
            &mut session,
            "wrong memory target type",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 1);
    assert!(dispatcher.dispatched.is_empty());
    let expected = format!("Memory update target `{seeded_id}` has record type `note` not `task`");
    assert!(handle.events().iter().any(|event| {
        if event.event_type != HarnessEventType::SemanticActionRejected {
            return false;
        }
        let HarnessEventPayload::Action { fields, .. } = &event.payload else {
            return false;
        };
        fields
            .get("error")
            .and_then(Value::as_str)
            .is_some_and(|error| error == expected)
    }));
    assert!(model.requests[1].prompt.render_text().contains(&expected));
}

#[test]
fn memory_runtime_failure_returns_structured_action_result_without_fake_dispatch() {
    let temp = temp_workspace_dir("m14c-runtime-failure");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let mut runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    runtime.memory[0].runtime = "process-memory-fixture".into();
    runtime.memory[0].readiness_reason = Some("M14e custom runtime dispatch is not active".into());
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "read",
                SemanticAction::MemoryRead {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    mode: MemoryReadMode::Key,
                    record_id: Some("mem_missing".into()),
                    record_type: Some("note".into()),
                    filter: BTreeMap::new(),
                    query: None,
                    limit: None,
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let result = engine
        .execute_run(
            &mut session,
            "read unavailable memory",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 1);
    assert!(dispatcher.dispatched.is_empty());
    assert_eq!(result.report.memory_summaries.len(), 1);
    assert_eq!(result.report.memory_summaries[0].status, "failed");
    assert_eq!(result.report.action_summaries.len(), 1);
    assert_eq!(
        result.report.action_summaries[0].error.as_deref(),
        Some("M14e custom runtime dispatch is not active")
    );
    assert!(
        model.requests[1]
            .prompt
            .render_text()
            .contains("memory_runtime_unavailable")
    );
    let events = handle.events();
    assert!(
        events
            .iter()
            .any(|event| event.event_type == HarnessEventType::MemoryReadStarted)
    );
    assert!(
        events
            .iter()
            .any(|event| event.event_type == HarnessEventType::MemoryReadFailed)
    );
}

fn review_complete_turn() -> ModelTurn {
    ModelTurn {
        assistant_content: Some("review complete".into()),
        actions: vec![SemanticActionProposal::new(
            "review-complete",
            SemanticAction::PersistenceReviewComplete,
        )],
        usage: RunUsage::default(),
        finish_reason: None,
        provider_metadata: BTreeMap::new(),
    }
}

fn review_write_turn(body: &str) -> ModelTurn {
    review_write_turn_for_package("m14c-memory-test", body)
}

fn review_write_turn_for_package(package: &str, body: &str) -> ModelTurn {
    ModelTurn {
        assistant_content: None,
        actions: vec![SemanticActionProposal::new(
            "review-write",
            SemanticAction::MemoryWrite {
                package: package.into(),
                space: "notes".into(),
                operation: MemoryWriteOperation::Create,
                record_type: "note".into(),
                record_id: None,
                content: Some(json!({ "body": body })),
            },
        )],
        usage: RunUsage::default(),
        finish_reason: Some("tool_calls".into()),
        provider_metadata: BTreeMap::new(),
    }
}

fn review_write_turn_for_space(space: &str, body: &str) -> ModelTurn {
    ModelTurn {
        assistant_content: None,
        actions: vec![SemanticActionProposal::new(
            "review-write",
            SemanticAction::MemoryWrite {
                package: "m14c-memory-test".into(),
                space: space.into(),
                operation: MemoryWriteOperation::Create,
                record_type: "note".into(),
                record_id: None,
                content: Some(json!({ "body": body })),
            },
        )],
        usage: RunUsage::default(),
        finish_reason: Some("tool_calls".into()),
        provider_metadata: BTreeMap::new(),
    }
}

fn review_read_by_body_turn(body: &str) -> ModelTurn {
    ModelTurn {
        assistant_content: None,
        actions: vec![SemanticActionProposal::new(
            "review-read",
            SemanticAction::MemoryRead {
                package: "m14c-memory-test".into(),
                space: "notes".into(),
                mode: MemoryReadMode::Filter,
                record_id: None,
                record_type: Some("note".into()),
                filter: BTreeMap::from([("body".into(), json!(body))]),
                query: None,
                limit: Some(1),
            },
        )],
        usage: RunUsage::default(),
        finish_reason: Some("tool_calls".into()),
        provider_metadata: BTreeMap::new(),
    }
}

fn add_conversation_state_memory_surface(
    runtime: &mut RuntimeSnapshot,
    package_root: &std::path::Path,
) {
    let mut conversation_state = runtime.memory[0].clone();
    conversation_state.space = "conversation_state".into();
    conversation_state.model = MemorySpaceModel::Document;
    conversation_state.description = "Conversation state.".into();
    conversation_state.retrieval_modes = vec![MemoryRetrievalMode::Key];
    conversation_state.record_types =
        vec![m14c_profile_record_type_snapshot(package_root, "profile_a")];
    runtime.memory.insert(0, conversation_state);

    let mut hidden_notes = runtime.memory[1].clone();
    hidden_notes.space = "hidden_notes".into();
    hidden_notes.description = "Hidden notes.".into();
    hidden_notes.binding_scope = "phase:other".into();
    runtime.memory.push(hidden_notes);
}

fn review_update_turn(record_id: &str, body: &str) -> ModelTurn {
    ModelTurn {
        assistant_content: None,
        actions: vec![SemanticActionProposal::new(
            "review-update",
            SemanticAction::MemoryWrite {
                package: "m14c-memory-test".into(),
                space: "notes".into(),
                operation: MemoryWriteOperation::Update,
                record_type: "note".into(),
                record_id: Some(record_id.into()),
                content: Some(json!({ "body": body })),
            },
        )],
        usage: RunUsage::default(),
        finish_reason: Some("tool_calls".into()),
        provider_metadata: BTreeMap::new(),
    }
}

fn review_action_turn(id: &str, action: SemanticAction) -> ModelTurn {
    ModelTurn {
        assistant_content: None,
        actions: vec![SemanticActionProposal::new(id, action)],
        usage: RunUsage::default(),
        finish_reason: Some("tool_calls".into()),
        provider_metadata: BTreeMap::new(),
    }
}

fn empty_review_turn() -> ModelTurn {
    ModelTurn {
        assistant_content: Some("no action".into()),
        actions: Vec::new(),
        usage: RunUsage::default(),
        finish_reason: None,
        provider_metadata: BTreeMap::new(),
    }
}

fn memory_review_options(
    points: Vec<crate::harness_config::HarnessMemoryWriteReviewPoint>,
) -> HarnessEngineOptions {
    HarnessEngineOptions::new(limits()).with_memory_write_review_points(points)
}

fn lifecycle_fields_for(
    events: &[HarnessEventEnvelope],
    event_type: HarnessEventType,
) -> Vec<BTreeMap<String, Value>> {
    events
        .iter()
        .filter_map(|event| {
            if event.event_type != event_type {
                return None;
            }
            let HarnessEventPayload::Lifecycle { fields, .. } = &event.payload else {
                return None;
            };
            Some(fields.clone())
        })
        .collect()
}

fn two_phase_memory_loop() -> LoopManifest {
    LoopManifest {
        kind: "loop".into(),
        name: "m14g-two-phase-memory-loop".into(),
        version: "0.1.0".into(),
        description: None,
        readme: None,
        license: None,
        r#loop: LoopMetadata {
            archetype: None,
            entry_phase: "remember".into(),
            limits: None,
            phases: vec![
                LoopPhase {
                    id: "remember".into(),
                    objective: "Use direct Memory when useful.".into(),
                    access: None,
                    outcomes: vec![LoopOutcome {
                        id: "next".into(),
                        description: "Next.".into(),
                    }],
                },
                LoopPhase {
                    id: "finish".into(),
                    objective: "Finish.".into(),
                    access: None,
                    outcomes: vec![LoopOutcome {
                        id: "done".into(),
                        description: "Done.".into(),
                    }],
                },
            ],
            transitions: vec![
                LoopTransition {
                    from: "remember".into(),
                    on: "next".into(),
                    to: "finish".into(),
                },
                LoopTransition {
                    from: "finish".into(),
                    on: "done".into(),
                    to: "$end".into(),
                },
            ],
            checkpoints: Vec::new(),
            error_policy: None,
        },
    }
}

fn seed_m14c_note(
    session: &mut HarnessSession,
    package_root: &std::path::Path,
    body: &str,
) -> String {
    let (manifest_value, _) = load_manifest_value(&package_root.join("agent.json")).unwrap();
    let manifest = parse_memory_manifest(&manifest_value).unwrap();
    let contracts =
        crate::harness_runtime::memory::validate_and_load_memory_contracts(package_root).unwrap();
    session
        .local_memory_runtime()
        .unwrap()
        .write_record(LocalMemoryWriteRequest {
            package: "m14c-memory-test",
            package_version: "0.1.0",
            manifest: &manifest,
            contracts: &contracts,
            space: "notes",
            record_type: "note",
            scope: BTreeMap::from([("user".into(), "user-123".into())]),
            operation: LocalMemoryWriteOperation::Create,
            record_id: None,
            content: Some(json!({ "body": body })),
            provenance: json!({}),
            now: Utc::now(),
        })
        .unwrap()
        .affected_record_id
        .unwrap()
}

#[test]
fn omitted_memory_write_review_preserves_existing_behavior() {
    let temp = temp_workspace_dir("m14g-omitted-review");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![completion("done", "done")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );

    let result = engine
        .execute_run(
            &mut session,
            "no review configured",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };

    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.model_calls, 1);
    assert_eq!(model.requests.len(), 1);
    assert!(result.report.memory_write_review_summaries.is_empty());
    assert!(!handle.events().iter().any(|event| {
        matches!(
            event.event_type,
            HarnessEventType::MemoryWriteReviewStarted
                | HarnessEventType::MemoryWriteReviewCompleted
                | HarnessEventType::MemoryWriteReviewSkipped
                | HarnessEventType::MemoryWriteReviewFailed
        )
    }));
}

#[test]
fn memory_write_review_run_end_supersedes_phase_end_and_uses_memory_pipeline() {
    let temp = temp_workspace_dir("m14g-run-end-review");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        completion("done", "done"),
        review_write_turn("M14g reviewed terminal write"),
        review_complete_turn(),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        memory_review_options(vec![
            crate::harness_config::HarnessMemoryWriteReviewPoint::PhaseEnd,
            crate::harness_config::HarnessMemoryWriteReviewPoint::RunEnd,
        ]),
    );
    let result = engine
        .execute_run(
            &mut session,
            "review terminal memory",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.model_calls, 3);
    assert_eq!(result.report.usage.memory_requests, 1);
    assert_eq!(result.report.usage.accepted_semantic_actions, 2);
    assert!(dispatcher.dispatched.is_empty());

    let review_request = &model.requests[1];
    let review_actions = review_request
        .effective_phase
        .capability_catalog
        .iter()
        .map(|descriptor| descriptor.action_kind.as_str())
        .collect::<Vec<_>>();
    assert!(review_actions.contains(&"memory_read"));
    assert!(review_actions.contains(&"memory_write"));
    assert!(review_actions.contains(&"persistence_review_complete"));
    assert!(!review_actions.contains(&"phase_completion"));
    assert!(
        review_request
            .prompt
            .render_text()
            .contains("bounded Memory write review at `run_end`")
    );

    let events = handle.events();
    let starts = lifecycle_fields_for(&events, HarnessEventType::MemoryWriteReviewStarted);
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0]["point"], json!("run_end"));
    let completed = lifecycle_fields_for(&events, HarnessEventType::MemoryWriteReviewCompleted);
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0]["memory_writes_attempted"], json!(1));
    assert_eq!(completed[0]["memory_writes_completed"], json!(1));
    assert_eq!(result.report.memory_write_review_summaries.len(), 1);
    assert_eq!(
        result.report.memory_write_review_summaries[0].point,
        "run_end"
    );
    assert_eq!(
        result.report.memory_write_review_summaries[0].memory_writes_completed,
        1
    );
    assert!(lifecycle_fields_for(&events, HarnessEventType::MemoryWriteReviewSkipped).is_empty());
    assert!(lifecycle_fields_for(&events, HarnessEventType::MemoryWriteReviewFailed).is_empty());

    let event_types = events
        .iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    let outcome = event_types
        .iter()
        .position(|event| *event == HarnessEventType::OutcomeSelected)
        .unwrap();
    let review_started = event_types
        .iter()
        .position(|event| *event == HarnessEventType::MemoryWriteReviewStarted)
        .unwrap();
    let memory_started = event_types
        .iter()
        .position(|event| *event == HarnessEventType::MemoryWriteStarted)
        .unwrap();
    let review_completed = event_types
        .iter()
        .position(|event| *event == HarnessEventType::MemoryWriteReviewCompleted)
        .unwrap();
    let phase_result = event_types
        .iter()
        .position(|event| *event == HarnessEventType::PhaseResultReady)
        .unwrap();
    let transition = event_types
        .iter()
        .position(|event| *event == HarnessEventType::TransitionSelected)
        .unwrap();
    assert!(outcome < review_started);
    assert!(review_started < memory_started);
    assert!(memory_started < review_completed);
    assert!(review_completed < phase_result);
    assert!(phase_result < transition);
}

#[test]
fn memory_write_review_completion_does_not_reenter_review() {
    let temp = temp_workspace_dir("m14g-no-recursive-review");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model =
        ScriptedModelRuntime::new(vec![completion("done", "done"), review_complete_turn()]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        memory_review_options(vec![
            crate::harness_config::HarnessMemoryWriteReviewPoint::RunEnd,
        ]),
    );

    let result = engine
        .execute_run(
            &mut session,
            "no recursive review",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };

    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(model.requests.len(), 2);
    assert_eq!(
        lifecycle_fields_for(&handle.events(), HarnessEventType::MemoryWriteReviewStarted).len(),
        1
    );
    assert_eq!(
        lifecycle_fields_for(
            &handle.events(),
            HarnessEventType::MemoryWriteReviewCompleted
        )
        .len(),
        1
    );
    assert!(
        lifecycle_fields_for(&handle.events(), HarnessEventType::MemoryWriteReviewFailed)
            .is_empty()
    );
    assert_eq!(result.report.memory_write_review_summaries.len(), 1);
}

#[test]
fn memory_write_review_phase_end_runs_before_transition_without_leaking_transcript() {
    let temp = temp_workspace_dir("m14g-phase-end-review");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let review_sentinel = "M14g phase review write sentinel";
    let mut model = ScriptedModelRuntime::new(vec![
        completion("next", "next"),
        review_write_turn(review_sentinel),
        review_complete_turn(),
        completion("done", "done"),
        review_complete_turn(),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        two_phase_memory_loop(),
        memory_review_options(vec![
            crate::harness_config::HarnessMemoryWriteReviewPoint::PhaseEnd,
        ]),
    );

    let result = engine
        .execute_run(
            &mut session,
            "phase end review",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };

    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.phase_summaries.len(), 2);
    assert_eq!(
        result.report.phase_summaries[0].outcome.as_deref(),
        Some("next")
    );
    assert_eq!(result.report.memory_write_review_summaries.len(), 2);
    assert_eq!(
        result.report.memory_write_review_summaries[0].point,
        "phase_end"
    );

    let second_phase_request = &model.requests[3];
    assert_eq!(second_phase_request.prior_phase_results.len(), 1);
    assert_eq!(second_phase_request.prior_phase_results[0].outcome, "next");
    let second_phase_prompt = second_phase_request.prompt.render_text();
    assert!(!second_phase_prompt.contains("ActionResult [memory_write"));
    assert!(!second_phase_prompt.contains("PersistenceReviewComplete"));
    assert!(!second_phase_prompt.contains("persistence_review_complete"));
    assert!(!second_phase_prompt.contains(review_sentinel));
    assert!(!second_phase_prompt.contains("review complete"));

    let event_types = handle
        .events()
        .iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    let review_started = event_types
        .iter()
        .position(|event| *event == HarnessEventType::MemoryWriteReviewStarted)
        .unwrap();
    let transition = event_types
        .iter()
        .position(|event| *event == HarnessEventType::TransitionSelected)
        .unwrap();
    assert!(review_started < transition);
}

#[test]
fn memory_write_review_run_end_covers_handoff_and_abort_terminals() {
    for (target, status) in [
        ("$handoff", HarnessTerminalStatus::HandedOff),
        ("$abort", HarnessTerminalStatus::Aborted),
    ] {
        let temp = temp_workspace_dir(&format!("m14g-run-end-{target}").replace('$', ""));
        let package_root = temp.join("memory-package");
        std::fs::create_dir_all(&package_root).unwrap();
        let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
        let mut session = HarnessSession::with_runtime_snapshot(runtime);
        let memory = InMemoryEventSink::default();
        let handle = memory.clone();
        session.emitter.add_sink(Box::new(memory));
        let mut loop_manifest = one_phase_memory_loop(None);
        loop_manifest.r#loop.transitions[0].to = target.into();
        let mut model =
            ScriptedModelRuntime::new(vec![completion("done", "done"), review_complete_turn()]);
        let mut dispatcher = ScriptedActionDispatcher::default();
        let mut approvals = ScriptedApprovalController::default();
        let mut engine = HarnessEngine::new(
            loop_manifest,
            memory_review_options(vec![
                crate::harness_config::HarnessMemoryWriteReviewPoint::RunEnd,
            ]),
        );

        let result = engine
            .execute_run(
                &mut session,
                "terminal review",
                &mut model,
                &mut dispatcher,
                &mut approvals,
            )
            .unwrap();
        let HarnessRunResult::Terminal(result) = result else {
            panic!("expected terminal result");
        };
        assert_eq!(result.report.terminal_status, status);
        assert_eq!(result.report.memory_write_review_summaries.len(), 1);
        assert_eq!(
            result.report.memory_write_review_summaries[0].point,
            "run_end"
        );
        assert_eq!(
            lifecycle_fields_for(&handle.events(), HarnessEventType::MemoryWriteReviewStarted)
                .len(),
            1
        );
    }
}

#[test]
fn memory_write_review_skips_without_writable_surface() {
    let temp = temp_workspace_dir("m14g-skip-review");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![completion("done", "done")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let write_disabled_loop = one_phase_memory_loop(Some(LoopPhaseAccess {
        tools: None,
        knowledge: None,
        memory: Some(LoopAccessMemory {
            read: Some(true),
            write: Some(false),
        }),
    }));
    let mut engine = HarnessEngine::new(
        write_disabled_loop,
        memory_review_options(vec![
            crate::harness_config::HarnessMemoryWriteReviewPoint::RunEnd,
        ]),
    );
    let result = engine
        .execute_run(
            &mut session,
            "review terminal memory",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.model_calls, 1);
    assert_eq!(result.report.usage.memory_requests, 0);
    assert_eq!(model.requests.len(), 1);
    let skipped =
        lifecycle_fields_for(&handle.events(), HarnessEventType::MemoryWriteReviewSkipped);
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0]["reason"], json!("no_writable_memory_surface"));
    assert_eq!(skipped[0]["point"], json!("run_end"));
    assert_eq!(result.report.memory_write_review_summaries.len(), 1);
    assert_eq!(
        result.report.memory_write_review_summaries[0].status,
        "skipped"
    );
}

#[test]
fn memory_write_review_failure_preserves_pending_completion() {
    let temp = temp_workspace_dir("m14g-review-failure");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::with_results(vec![
        Ok(completion("done", "done")),
        Err(ModelRuntimeFailure::new("review model down")),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        memory_review_options(vec![
            crate::harness_config::HarnessMemoryWriteReviewPoint::RunEnd,
        ]),
    );
    let result = engine
        .execute_run(
            &mut session,
            "review terminal memory",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.phase_summaries.len(), 1);
    assert_eq!(
        result.report.phase_summaries[0].outcome.as_deref(),
        Some("done")
    );
    assert_eq!(result.report.usage.model_calls, 2);
    assert_eq!(result.report.usage.memory_requests, 0);
    let failed = lifecycle_fields_for(&handle.events(), HarnessEventType::MemoryWriteReviewFailed);
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["reason"], json!("model_request_failed"));
    assert_eq!(result.report.memory_write_review_summaries.len(), 1);
    assert_eq!(
        result.report.memory_write_review_summaries[0].status,
        "failed"
    );
    assert!(
        handle
            .events()
            .iter()
            .any(|event| event.event_type == HarnessEventType::PhaseResultReady)
    );
    assert!(
        handle
            .events()
            .iter()
            .any(|event| event.event_type == HarnessEventType::RunCompleted)
    );
}

#[test]
fn memory_write_review_mismatch_feedback_suggests_authorized_record_type_surface() {
    let temp = temp_workspace_dir("m14h-review-mismatch-repair");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let mut runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    add_conversation_state_memory_surface(&mut runtime, &package_root);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        completion("done", "done"),
        review_write_turn_for_space("conversation_state", "wrong target"),
        review_write_turn("corrected target"),
        review_complete_turn(),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        memory_review_options(vec![
            crate::harness_config::HarnessMemoryWriteReviewPoint::RunEnd,
        ]),
    );
    let result = engine
        .execute_run(
            &mut session,
            "review target mismatch recovery",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.repair_count, 1);
    assert_eq!(result.report.usage.memory_requests, 1);
    assert_eq!(result.report.memory_write_review_summaries.len(), 1);
    assert_eq!(
        result.report.memory_write_review_summaries[0].status,
        "completed"
    );
    assert_eq!(
        result.report.memory_write_review_summaries[0].memory_writes_completed,
        1
    );

    let expected = "Memory record type `note` is not declared for selected Memory space `conversation_state` in package `m14c-memory-test`. Authorized alternative Memory write action(s) for record type `note`: `m14c-memory-test/notes`.";
    assert!(handle.events().iter().any(|event| {
        if event.event_type != HarnessEventType::SemanticActionRejected {
            return false;
        }
        let HarnessEventPayload::Action { fields, .. } = &event.payload else {
            return false;
        };
        fields
            .get("error")
            .and_then(Value::as_str)
            .is_some_and(|error| error == expected)
    }));
    let repair_prompt = model.requests[2].prompt.render_text();
    assert!(repair_prompt.contains(expected));
    assert!(!repair_prompt.contains("m14c-memory-test/hidden_notes"));
    let post_write_prompt = model.requests[3].prompt.render_text();
    assert!(post_write_prompt.contains("ActionResult [memory_write m14c-memory-test/notes]"));
    assert!(post_write_prompt.contains("corrected target"));

    assert!(handle.events().iter().any(|event| {
        if event.event_type != HarnessEventType::SemanticActionProposed {
            return false;
        }
        let HarnessEventPayload::Action {
            identity, fields, ..
        } = &event.payload
        else {
            return false;
        };
        identity == "m14c-memory-test/notes"
            && fields.get("status").is_none()
            && fields.get("space") == Some(&json!("notes"))
            && fields.get("record_type") == Some(&json!("note"))
            && fields
                .get("content")
                .and_then(|content| content.get("body"))
                == Some(&json!("corrected target"))
    }));
    assert!(handle.events().iter().any(|event| {
        if event.event_type != HarnessEventType::MemoryWriteCompleted {
            return false;
        }
        let HarnessEventPayload::Action {
            identity, fields, ..
        } = &event.payload
        else {
            return false;
        };
        identity == "m14c-memory-test/notes"
            && fields.get("space") == Some(&json!("notes"))
            && fields
                .get("result")
                .and_then(|result| result.get("record"))
                .and_then(|record| record.get("content"))
                .and_then(|content| content.get("body"))
                == Some(&json!("corrected target"))
    }));
}

#[test]
fn memory_write_review_mismatch_repair_exhaustion_stays_nonfatal_and_nonmutating() {
    let temp = temp_workspace_dir("m14h-review-mismatch-exhaustion");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let mut runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    add_conversation_state_memory_surface(&mut runtime, &package_root);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        completion("done", "done"),
        review_write_turn_for_space("conversation_state", "wrong target 1"),
        review_write_turn_for_space("conversation_state", "wrong target 2"),
        review_write_turn_for_space("conversation_state", "wrong target 3"),
        review_write_turn_for_space("conversation_state", "wrong target 4"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        memory_review_options(vec![
            crate::harness_config::HarnessMemoryWriteReviewPoint::RunEnd,
        ]),
    );
    let result = engine
        .execute_run(
            &mut session,
            "review target mismatch exhaustion",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 0);
    assert_eq!(result.report.memory_write_review_summaries.len(), 1);
    let summary = &result.report.memory_write_review_summaries[0];
    assert_eq!(summary.status, "failed");
    assert_eq!(summary.reason, "structured_output_repair_limit");
    assert_eq!(summary.memory_writes_attempted, 0);
    assert_eq!(summary.memory_writes_completed, 0);
    assert!(
        handle
            .events()
            .iter()
            .any(|event| { event.event_type == HarnessEventType::MemoryWriteReviewFailed })
    );
    assert!(
        handle
            .events()
            .iter()
            .any(|event| { event.event_type == HarnessEventType::PhaseResultReady })
    );
    assert!(
        handle
            .events()
            .iter()
            .any(|event| { event.event_type == HarnessEventType::RunCompleted })
    );
}

#[test]
fn memory_write_review_can_read_then_write_using_review_transcript() {
    let temp = temp_workspace_dir("m14g-read-write-review");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let seeded_id = seed_m14c_note(&mut session, &package_root, "seed note");
    let mut model = ScriptedModelRuntime::new(vec![
        completion("done", "done"),
        review_read_by_body_turn("seed note"),
        review_update_turn(&seeded_id, "updated by review"),
        review_complete_turn(),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        memory_review_options(vec![
            crate::harness_config::HarnessMemoryWriteReviewPoint::RunEnd,
        ]),
    );

    let result = engine
        .execute_run(
            &mut session,
            "read then write review",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };

    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 2);
    assert_eq!(result.report.memory_summaries.len(), 2);
    assert_eq!(result.report.memory_write_review_summaries.len(), 1);
    let review = &result.report.memory_write_review_summaries[0];
    assert_eq!(review.memory_reads_completed, 1);
    assert_eq!(review.memory_writes_completed, 1);
    assert!(model.requests[2].prompt.render_text().contains("seed note"));

    let (manifest_value, _) = load_manifest_value(&package_root.join("agent.json")).unwrap();
    let manifest = parse_memory_manifest(&manifest_value).unwrap();
    let records = session
        .local_memory_runtime()
        .unwrap()
        .read_records(LocalMemoryReadRequest {
            package: "m14c-memory-test",
            package_version: "0.1.0",
            manifest: &manifest,
            space: "notes",
            scope: BTreeMap::from([("user".into(), "user-123".into())]),
            mode: LocalMemoryReadMode::Key,
            record_id: Some(seeded_id.clone()),
            record_type: Some("note".into()),
            filter: BTreeMap::new(),
            query: None,
            limit: None,
            now: Utc::now(),
        })
        .unwrap();
    assert_eq!(records[0].content, json!({ "body": "updated by review" }));
}

#[test]
fn memory_write_review_write_only_catalog_allows_writes_without_reads() {
    let temp = temp_workspace_dir("m14g-write-only-review");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let mut model = ScriptedModelRuntime::new(vec![
        completion("done", "done"),
        review_write_turn("write-only review note"),
        review_complete_turn(),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let loop_manifest = one_phase_memory_loop(Some(LoopPhaseAccess {
        tools: None,
        knowledge: None,
        memory: Some(LoopAccessMemory {
            read: Some(false),
            write: Some(true),
        }),
    }));
    let mut engine = HarnessEngine::new(
        loop_manifest,
        memory_review_options(vec![
            crate::harness_config::HarnessMemoryWriteReviewPoint::RunEnd,
        ]),
    );

    let result = engine
        .execute_run(
            &mut session,
            "write-only review",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };

    let review_actions = model.requests[1]
        .effective_phase
        .capability_catalog
        .iter()
        .map(|descriptor| descriptor.action_kind.as_str())
        .collect::<Vec<_>>();
    assert!(!review_actions.contains(&"memory_read"));
    assert!(review_actions.contains(&"memory_write"));
    assert_eq!(result.report.usage.memory_requests, 1);
    assert_eq!(
        result.report.memory_write_review_summaries[0].memory_writes_completed,
        1
    );
}

#[test]
fn memory_write_review_rejects_non_review_catalog_actions_before_execution() {
    let prohibited_actions = vec![
        (
            "tool",
            SemanticAction::AgentPmTool {
                tool: "@zack/search".into(),
                arguments: json!({ "query": "x" }),
            },
        ),
        (
            "knowledge",
            SemanticAction::KnowledgeRequest {
                package: "@zack/kb".into(),
                mode: None,
                document: None,
                query: Some("x".into()),
                top_k: Some(1),
                score_threshold: None,
                return_citations: None,
            },
        ),
        (
            "skill",
            SemanticAction::SkillResourceRead {
                skill: "@zack/skill".into(),
                resource: "SKILL.md".into(),
            },
        ),
        (
            "phase-completion",
            SemanticAction::PhaseCompletion {
                outcome: Some("done".into()),
                output: None,
            },
        ),
    ];

    for (label, action) in prohibited_actions {
        let temp = temp_workspace_dir(&format!("m14g-review-reject-{label}"));
        let package_root = temp.join("memory-package");
        std::fs::create_dir_all(&package_root).unwrap();
        let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
        let mut session = HarnessSession::with_runtime_snapshot(runtime);
        let memory = InMemoryEventSink::default();
        let handle = memory.clone();
        session.emitter.add_sink(Box::new(memory));
        let mut model = ScriptedModelRuntime::new(vec![
            completion("done", "done"),
            review_action_turn(label, action),
            review_complete_turn(),
        ]);
        let mut dispatcher = ScriptedActionDispatcher::default();
        let mut approvals = ScriptedApprovalController::default();
        let mut engine = HarnessEngine::new(
            one_phase_memory_loop(None),
            memory_review_options(vec![
                crate::harness_config::HarnessMemoryWriteReviewPoint::RunEnd,
            ]),
        );

        let result = engine
            .execute_run(
                &mut session,
                "reject non-review action",
                &mut model,
                &mut dispatcher,
                &mut approvals,
            )
            .unwrap();
        let HarnessRunResult::Terminal(result) = result else {
            panic!("expected terminal result");
        };
        assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
        assert_eq!(result.report.usage.memory_requests, 0);
        assert_eq!(result.report.usage.accepted_semantic_actions, 1);
        assert_eq!(result.report.repair_count, 1);
        assert!(dispatcher.dispatched.is_empty());
        assert!(
            !handle
                .events()
                .iter()
                .any(|event| event.event_type == HarnessEventType::MemoryWriteStarted)
        );
        assert!(handle.events().iter().any(|event| {
            if event.event_type != HarnessEventType::SemanticActionRejected {
                return false;
            }
            matches!(
                &event.payload,
                HarnessEventPayload::Action { status, .. } if status == "prohibited_by_review"
            )
        }));
    }
}

#[test]
fn memory_write_review_write_uses_hooks_and_durable_projection() {
    let temp = temp_workspace_dir("m14g-hook-projection-review");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14f_projected_memory(&temp, &package_root, "local");
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        completion("done", "done"),
        review_write_turn_for_package("m14f-projected-memory-test", "original review note"),
        review_complete_turn(),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut knowledge = NoopKnowledgeRuntime;
    let mut hooks = TestHookRuntime {
        active_hooks: vec![HarnessHookId::BeforeMemoryWrite],
        memory_write: Some(BeforeMemoryWriteDecision {
            content: Some(json!({
                "body": "hooked review note",
                "scratch": {
                    "public": "visible review scratch",
                    "private": "hidden review scratch"
                }
            })),
            ..BeforeMemoryWriteDecision::default()
        }),
        ..TestHookRuntime::default()
    };
    let mut services = HarnessRuntimeServices {
        model: &mut model,
        dispatcher: &mut dispatcher,
        knowledge: &mut knowledge,
        memory: None,
        embedding_provider: None,
        approvals: &mut approvals,
        hooks: &mut hooks,
        service_events: None,
    };
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        memory_review_options(vec![
            crate::harness_config::HarnessMemoryWriteReviewPoint::RunEnd,
        ]),
    );

    let result = engine
        .execute_run_with_id(
            &mut session,
            allocate_harness_run_id(),
            "review hook projection",
            &mut services,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };

    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(hooks.memory_write_hooks.len(), 1);
    assert!(
        handle
            .events()
            .iter()
            .any(|event| event.event_type == HarnessEventType::HookCompleted)
    );
    let events = handle.events();
    let write_completed = events
        .iter()
        .find(|event| event.event_type == HarnessEventType::MemoryWriteCompleted)
        .expect("memory write completed");
    let HarnessEventPayload::Action { fields, .. } = &write_completed.payload else {
        panic!("expected action payload");
    };
    let content = &fields["result"]["record"]["content"];
    assert_eq!(content["body"], json!("hooked review note"));
    assert_eq!(
        content["scratch"]["public"],
        json!("visible review scratch")
    );
    assert!(content["scratch"].get("private").is_none());
}

#[test]
fn memory_write_review_limit_exhaustion_preserves_pending_completion() {
    let temp = temp_workspace_dir("m14g-review-limit");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        completion("done", "done"),
        empty_review_turn(),
        empty_review_turn(),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut runtime_limits = limits();
    runtime_limits.max_model_calls_per_phase = 2;
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(runtime_limits).with_memory_write_review_points(vec![
            crate::harness_config::HarnessMemoryWriteReviewPoint::RunEnd,
        ]),
    );

    let result = engine
        .execute_run(
            &mut session,
            "review limit",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };

    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.model_calls, 3);
    assert_eq!(model.requests.len(), 3);
    assert_eq!(
        result.report.phase_summaries[0].outcome.as_deref(),
        Some("done")
    );
    assert_eq!(
        result.report.memory_write_review_summaries[0].reason,
        "max_model_calls_per_phase"
    );
    assert_eq!(
        result.report.memory_write_review_summaries[0].status,
        "failed"
    );
    assert!(
        lifecycle_fields_for(&handle.events(), HarnessEventType::MemoryWriteReviewFailed)
            .iter()
            .any(|fields| fields["reason"] == json!("max_model_calls_per_phase"))
    );
}

#[test]
fn memory_write_review_committed_writes_survive_later_review_failure() {
    let temp = temp_workspace_dir("m14g-review-write-before-failure");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let mut model = ScriptedModelRuntime::with_results(vec![
        Ok(completion("done", "done")),
        Ok(review_write_turn("committed before review failure")),
        Err(ModelRuntimeFailure::new("review failed after write")),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        memory_review_options(vec![
            crate::harness_config::HarnessMemoryWriteReviewPoint::RunEnd,
        ]),
    );

    let result = engine
        .execute_run(
            &mut session,
            "review write then failure",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };

    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 1);
    assert_eq!(result.report.memory_summaries.len(), 1);
    assert_eq!(result.report.memory_summaries[0].status, "completed");
    assert_eq!(result.report.memory_write_review_summaries.len(), 1);
    assert_eq!(
        result.report.memory_write_review_summaries[0].memory_writes_completed,
        1
    );
    assert_eq!(
        result.report.memory_write_review_summaries[0].reason,
        "model_request_failed"
    );
}

#[test]
fn memory_write_review_routes_custom_host_memory_without_fake_dispatch() {
    #[derive(Clone)]
    struct RecordingReviewMemoryRuntime {
        calls: std::sync::Arc<std::sync::Mutex<Vec<(String, String, String, Value)>>>,
    }

    impl HostServiceInvoker for RecordingReviewMemoryRuntime {
        fn invoke_host_service(
            &mut self,
            role: &str,
            registry_id: &str,
            method: &str,
            payload: Value,
            _timeout_ms: u64,
        ) -> Result<Value> {
            self.calls.lock().unwrap().push((
                role.into(),
                registry_id.into(),
                method.into(),
                payload.clone(),
            ));
            Ok(json!({
                "ok": true,
                "package": payload["request"]["package"].clone(),
                "package_version": payload["request"]["package_version"].clone(),
                "space": payload["request"]["space"].clone(),
                "operation": payload["request"]["operation"].clone(),
                "record_id": "remote-review-1"
            }))
        }
    }

    let temp = temp_workspace_dir("m14g-custom-host-review");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let mut runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    runtime.memory[0].runtime = "remote-memory".into();
    let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let custom_memory = CustomMemoryRuntime::new(
        runtime.memory.clone(),
        HashMap::from([(
            "remote-memory".into(),
            ServiceRuntime::host(
                Box::new(RecordingReviewMemoryRuntime {
                    calls: calls.clone(),
                }),
                1_000,
            ),
        )]),
    );
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let mut model = ScriptedModelRuntime::new(vec![
        completion("done", "done"),
        review_write_turn("custom host review write"),
        review_complete_turn(),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut knowledge = NoopKnowledgeRuntime;
    let mut hooks = NoopHookRuntime;
    let mut services = HarnessRuntimeServices {
        model: &mut model,
        dispatcher: &mut dispatcher,
        knowledge: &mut knowledge,
        memory: Some(custom_memory),
        embedding_provider: None,
        approvals: &mut approvals,
        hooks: &mut hooks,
        service_events: None,
    };
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        memory_review_options(vec![
            crate::harness_config::HarnessMemoryWriteReviewPoint::RunEnd,
        ]),
    );

    let result = engine
        .execute_run_with_id(
            &mut session,
            allocate_harness_run_id(),
            "custom review write",
            &mut services,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };

    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 1);
    assert!(dispatcher.dispatched.is_empty());
    assert_eq!(
        result.report.memory_write_review_summaries[0].memory_writes_completed,
        1
    );
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "memory");
    assert_eq!(calls[0].1, "remote-memory");
    assert_eq!(calls[0].2, "write");
    assert_eq!(
        calls[0].3["request"]["content"]["body"],
        "custom host review write"
    );
}

#[test]
fn memory_write_review_routes_custom_process_memory_without_fake_dispatch() {
    let temp = temp_workspace_dir("m14g-custom-process-review");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let mut runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    runtime.memory[0].runtime = "process-memory".into();

    let script = temp.join("process_memory_service.py");
    std::fs::write(
        &script,
        r#"
import json, sys
for line in sys.stdin:
    msg = json.loads(line)
    if msg["kind"] == "initialize":
        result = {
            "ready": True,
            "registry_id": "process-memory",
            "protocol_version": 1,
            "capabilities": {
                "space_models": ["collection"],
                "retrieval_modes": ["key", "filter", "chronological", "full_text"],
                "retention_actions": [],
                "constraints": [],
                "capacity": False,
                "durable_trigger_state": False,
                "atomic_batches": False,
                "packages": [
                    { "package": "m14c-memory-test", "version": "0.1.0", "ready": True }
                ]
            }
        }
        print(json.dumps({
            "protocol": "agentpm-service",
            "version": 1,
            "kind": "initialized",
            "id": msg.get("id"),
            "service": "memory",
            "result": result
        }), flush=True)
    elif msg["kind"] == "request" and msg.get("method") == "write":
        req = msg["payload"]["request"]
        result = {
            "ok": True,
            "package": req["package"],
            "package_version": req["package_version"],
            "space": req["space"],
            "operation": req["operation"],
            "record_id": "process-review-1"
        }
        print(json.dumps({
            "protocol": "agentpm-service",
            "version": 1,
            "kind": "response",
            "id": msg.get("id"),
            "service": "memory",
            "result": result
        }), flush=True)
"#,
    )
    .unwrap();
    let entry = crate::harness_config::HarnessImplementationEntry {
        implementation: crate::harness_config::HarnessImplementation::Process {
            command: "python3".into(),
            args: vec![script.display().to_string()],
            cwd: None,
            env: Vec::new(),
            startup_timeout_ms: 1_000,
            request_timeout_ms: 1_000,
            restart: Default::default(),
        },
    };
    let (process_runtime, _capabilities) =
        crate::harness_runtime::memory::process_memory_runtime_service(
            &temp,
            "process-memory",
            &entry,
            &runtime.memory,
            None,
        )
        .unwrap();
    let custom_memory = CustomMemoryRuntime::new(
        runtime.memory.clone(),
        HashMap::from([("process-memory".into(), process_runtime)]),
    );
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        completion("done", "done"),
        review_write_turn("custom process review write"),
        review_complete_turn(),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut knowledge = NoopKnowledgeRuntime;
    let mut hooks = NoopHookRuntime;
    let mut services = HarnessRuntimeServices {
        model: &mut model,
        dispatcher: &mut dispatcher,
        knowledge: &mut knowledge,
        memory: Some(custom_memory),
        embedding_provider: None,
        approvals: &mut approvals,
        hooks: &mut hooks,
        service_events: None,
    };
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        memory_review_options(vec![
            crate::harness_config::HarnessMemoryWriteReviewPoint::RunEnd,
        ]),
    );

    let result = engine
        .execute_run_with_id(
            &mut session,
            allocate_harness_run_id(),
            "custom process review write",
            &mut services,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };

    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 1);
    assert!(dispatcher.dispatched.is_empty());
    assert_eq!(
        result.report.memory_write_review_summaries[0].memory_writes_completed,
        1
    );
    let events = handle.events();
    let write_completed = events
        .iter()
        .find(|event| event.event_type == HarnessEventType::MemoryWriteCompleted)
        .expect("memory write completed");
    let HarnessEventPayload::Action { fields, .. } = &write_completed.payload else {
        panic!("expected action payload");
    };
    assert_eq!(fields["result"]["record_id"], json!("process-review-1"));
}
