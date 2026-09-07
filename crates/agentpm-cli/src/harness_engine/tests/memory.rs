use super::*;

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
