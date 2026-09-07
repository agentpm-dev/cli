use super::*;
#[test]
fn before_tool_selection_hook_subsets_model_visible_tool_catalog() {
    let mut session = HarnessSession::with_runtime_snapshot(runtime_with_two_tools_and_skill());
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new([completion("done", "handoff")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        tool_selection: Some(BeforeToolSelectionDecision {
            candidate_ids: Some(vec!["@zack/comment".into()]),
        }),
        active_hooks: vec![HarnessHookId::BeforeToolSelection],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
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
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-selection".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    assert!(matches!(result, HarnessRunResult::Terminal(_)));
    let prompt = model.requests[0].prompt.render_text();
    assert!(prompt.contains("@zack/comment"));
    assert!(!prompt.contains("@zack/search"));
    let hook_input = hooks.tool_selection_hooks.first().expect("hook input");
    assert_eq!(hook_input.phase.phase_id, "assess");
    assert_eq!(
        hook_input
            .candidates
            .iter()
            .map(|candidate| candidate.canonical_id.as_str())
            .collect::<Vec<_>>(),
        vec!["@zack/search", "@zack/comment"]
    );
    let event_types: Vec<_> = handle
        .events()
        .iter()
        .map(|event| event.event_type)
        .collect();
    assert!(event_types.contains(&HarnessEventType::HookStarted));
    assert!(event_types.contains(&HarnessEventType::HookCompleted));
    let phase_enter_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::PhaseEnterRequested)
        .expect("phase enter event");
    let hook_started_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookStarted)
        .expect("hook started event");
    let hook_completed_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookCompleted)
        .expect("hook completed event");
    let effective_phase_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::EffectivePhaseComputed)
        .expect("effective phase event");
    assert!(phase_enter_position < hook_started_position);
    assert!(hook_completed_position < effective_phase_position);
    let fields = hook_event_fields_for(
        &handle.events(),
        HarnessEventType::HookCompleted,
        "before_tool_selection",
    );
    assert_eq!(fields["binding_count"], json!(1));
    assert_eq!(fields["candidate_count_before"], json!(2));
    assert_eq!(fields["candidate_count_after"], json!(1));
    assert_eq!(fields["candidate_ids_after"], json!(["@zack/comment"]));
    assert_eq!(fields["patched"], json!(true));
}

#[test]
fn before_tool_selection_invalid_patch_still_reports_queued_nonfatal_failures() {
    let mut session = HarnessSession::with_runtime_snapshot(runtime_with_two_tools_and_skill());
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new([completion("done", "handoff")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        tool_selection: Some(BeforeToolSelectionDecision {
            candidate_ids: Some(vec!["@zack/introduced".into()]),
        }),
        nonfatal_before_tool_selection: Some("advisory hook failed".into()),
        active_hooks: vec![HarnessHookId::BeforeToolSelection],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
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
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-selection-invalid-patch".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert!(model.requests.is_empty());
    let events = handle.events();
    let event_types = events
        .iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    let first_hook_failed_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookFailed)
        .expect("nonfatal hook failed event");
    let phase_failed_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::PhaseFailed)
        .expect("phase failed event");
    assert!(first_hook_failed_position < phase_failed_position);
    let hook_failed_count = event_types
        .iter()
        .filter(|event_type| **event_type == HarnessEventType::HookFailed)
        .count();
    assert_eq!(hook_failed_count, 2);
    assert_eq!(
        hook_event_fields_for(
            &events,
            HarnessEventType::HookFailed,
            "before_tool_selection"
        )["nonfatal"],
        json!(true)
    );
}

#[test]
fn before_tool_call_hook_patches_arguments_and_revalidates() {
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model =
        ScriptedModelRuntime::new([tool_turn("@zack/search"), completion("done", "handoff")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        tool_call: Some(BeforeToolCallDecision {
            arguments: Some(json!({ "query": "patched" })),
        }),
        active_hooks: vec![HarnessHookId::BeforeToolCall],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
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
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-tool-call".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    assert!(matches!(result, HarnessRunResult::Terminal(_)));
    assert_eq!(
        dispatcher.dispatched[0],
        SemanticAction::AgentPmTool {
            tool: "@zack/search".into(),
            arguments: json!({ "query": "patched" }),
        }
    );
    let hook_input = hooks.tool_call_hooks.first().expect("hook input");
    assert_eq!(hook_input.phase_id, "assess");
    assert_eq!(hook_input.tool, "@zack/search");
    assert_eq!(hook_input.arguments, json!({ "query": "x" }));
    let event_types: Vec<_> = handle
        .events()
        .iter()
        .map(|event| event.event_type)
        .collect();
    assert!(event_types.contains(&HarnessEventType::HookStarted));
    assert!(event_types.contains(&HarnessEventType::HookCompleted));
    let fields = hook_event_fields_for(
        &handle.events(),
        HarnessEventType::HookCompleted,
        "before_tool_call",
    );
    assert_eq!(fields["binding_count"], json!(1));
    assert_eq!(fields["tool"], json!("@zack/search"));
    assert_eq!(fields["argument_keys_before"], json!(["query"]));
    assert_eq!(fields["argument_keys_after"], json!(["query"]));
    assert_eq!(fields["arguments_patched"], json!(true));
}

#[test]
fn before_tool_call_continue_failure_is_reported_before_completed() {
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model =
        ScriptedModelRuntime::new([tool_turn("@zack/search"), completion("done", "handoff")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        nonfatal_before_tool_call: Some("invalid hook patch".into()),
        active_hooks: vec![HarnessHookId::BeforeToolCall],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
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
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-tool-call-continue-failure".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    assert!(matches!(result, HarnessRunResult::Terminal(_)));
    assert_eq!(dispatcher.dispatched.len(), 1);
    let event_types = handle
        .events()
        .iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    let hook_started_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookStarted)
        .expect("hook started event");
    let hook_failed_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookFailed)
        .expect("hook failed event");
    let hook_completed_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookCompleted)
        .expect("hook completed event");
    assert!(hook_started_position < hook_failed_position);
    assert!(hook_failed_position < hook_completed_position);
    let fields = hook_event_fields_for(
        &handle.events(),
        HarnessEventType::HookFailed,
        "before_tool_call",
    );
    assert_eq!(fields["nonfatal"], json!(true));
}

#[test]
fn before_tool_call_hook_revalidates_patched_arguments_before_dispatch() {
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model =
        ScriptedModelRuntime::new([tool_turn("@zack/search"), completion("done", "handoff")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        tool_call: Some(BeforeToolCallDecision {
            arguments: Some(json!({})),
        }),
        nonfatal_before_tool_call: Some("advisory hook failed".into()),
        active_hooks: vec![HarnessHookId::BeforeToolCall],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
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
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-tool-call-invalid-args".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert!(dispatcher.dispatched.is_empty());
    let events = handle.events();
    let event_types: Vec<_> = events.iter().map(|event| event.event_type).collect();
    assert!(event_types.contains(&HarnessEventType::HookFailed));
    let hook_failed_count = event_types
        .iter()
        .filter(|event_type| **event_type == HarnessEventType::HookFailed)
        .count();
    assert_eq!(hook_failed_count, 2);
    let hook_failed_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookFailed)
        .expect("nonfatal hook failed event");
    let phase_failed_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::PhaseFailed)
        .expect("phase failed event");
    assert!(hook_failed_position < phase_failed_position);
    assert_eq!(
        hook_event_fields_for(&events, HarnessEventType::HookFailed, "before_tool_call")["nonfatal"],
        json!(true)
    );
    assert!(!event_types.contains(&HarnessEventType::ToolInvoked));
}

#[test]
fn before_tool_call_hook_rejection_blocks_dispatch_and_emits_rejected() {
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new([tool_turn("@zack/search")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        reject_before_tool_call: Some("blocked by policy".into()),
        active_hooks: vec![HarnessHookId::BeforeToolCall],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
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
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-tool-call-reject".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert!(dispatcher.dispatched.is_empty());
    let event_types: Vec<_> = handle
        .events()
        .iter()
        .map(|event| event.event_type)
        .collect();
    assert!(event_types.contains(&HarnessEventType::HookRejected));
    assert!(!event_types.contains(&HarnessEventType::ToolInvoked));
}

#[test]
fn before_tool_call_rejection_still_reports_queued_nonfatal_failures() {
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new([tool_turn("@zack/search")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        nonfatal_before_tool_call: Some("advisory hook failed".into()),
        reject_before_tool_call: Some("blocked by policy".into()),
        active_hooks: vec![HarnessHookId::BeforeToolCall],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
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
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-tool-call-nonfatal-then-reject".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert!(dispatcher.dispatched.is_empty());
    let events = handle.events();
    let event_types = events
        .iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    let hook_failed_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookFailed)
        .expect("hook failed event");
    let hook_rejected_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookRejected)
        .expect("hook rejected event");
    assert!(hook_failed_position < hook_rejected_position);
    assert_eq!(
        hook_event_fields_for(&events, HarnessEventType::HookFailed, "before_tool_call")["nonfatal"],
        json!(true)
    );
    assert!(!event_types.contains(&HarnessEventType::ToolInvoked));
}

#[test]
fn before_tool_call_hook_failure_emits_failed_not_rejected() {
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new([tool_turn("@zack/search")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        fail_before_tool_call: Some("transport timeout".into()),
        active_hooks: vec![HarnessHookId::BeforeToolCall],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
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
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-tool-call-failure".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert!(dispatcher.dispatched.is_empty());
    let event_types: Vec<_> = handle
        .events()
        .iter()
        .map(|event| event.event_type)
        .collect();
    assert!(event_types.contains(&HarnessEventType::HookFailed));
    assert!(!event_types.contains(&HarnessEventType::HookRejected));
    assert!(!event_types.contains(&HarnessEventType::ToolInvoked));
}

#[test]
fn before_knowledge_request_hook_shapes_request_before_dispatch() {
    let mut session =
        HarnessSession::with_runtime_snapshot(runtime_with_knowledge_packages(&["@zack/guide"]));
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        knowledge_query_turn("@zack/guide"),
        completion("assess-complete", "execute"),
        completion("execute-complete", "review"),
        completion("review-complete", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        knowledge_request: Some(BeforeKnowledgeRequestDecision {
            document: None,
            query: Some("patched query".into()),
            top_k: Some(3),
            score_threshold: Some(0.42),
            return_citations: Some(false),
        }),
        active_hooks: vec![HarnessHookId::BeforeKnowledgeRequest],
        ..TestHookRuntime::default()
    };
    let mut knowledge = RecordingKnowledgeRuntime::new(ActionDispatchResult::success(json!({
        "ok": true,
        "package": "@zack/guide",
        "version": "0.1.0",
        "mode": "vector_query",
        "query": "patched query",
        "results": [],
        "citations": []
    })));
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
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
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-knowledge-request".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    assert!(matches!(result, HarnessRunResult::Terminal(_)));
    assert_eq!(knowledge.dispatched.len(), 1);
    assert_eq!(
        knowledge.dispatched[0],
        SemanticAction::KnowledgeRequest {
            package: "@zack/guide".into(),
            mode: None,
            document: None,
            query: Some("patched query".into()),
            top_k: Some(3),
            score_threshold: Some(0.42),
            return_citations: Some(false),
        }
    );
    let hook_input = hooks.knowledge_request_hooks.first().expect("hook input");
    assert_eq!(hook_input.phase_id, "assess");
    assert_eq!(hook_input.request.package, "@zack/guide");
    assert_eq!(hook_input.request.query.as_deref(), Some("alpha"));
    assert_eq!(hook_input.request.top_k, Some(1));
    let fields = hook_event_fields_for(
        &handle.events(),
        HarnessEventType::HookCompleted,
        "before_knowledge_request",
    );
    assert_eq!(fields["binding_count"], json!(1));
    assert_eq!(fields["query"], json!("patched query"));
    assert_eq!(fields["top_k"], json!(3));
    assert_eq!(fields["score_threshold"], json!(0.42));
    assert_eq!(fields["return_citations"], json!(false));
    assert_eq!(fields["patched"], json!(true));
}

#[test]
fn before_model_request_hook_appends_context_and_merges_provider_options() {
    let mut runtime = runtime_with_tool_and_skill();
    runtime.model = Some(ModelProviderSelection {
        provider: "test-provider".into(),
        model: "test-model".into(),
        options: json!({ "temperature": 0.1, "existing": true }),
    });
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new([completion("done", "handoff")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut provider_options = serde_json::Map::new();
    provider_options.insert("temperature".into(), json!(0.2));
    provider_options.insert("metadata".into(), json!({ "hook": true }));
    let mut hooks = TestHookRuntime {
        model_request: Some(BeforeModelRequestDecision {
            context_sections: vec![BeforeModelRequestContextSection {
                title: "Policy Note".into(),
                content: "Prefer the safest concise answer.".into(),
            }],
            provider_options,
        }),
        active_hooks: vec![HarnessHookId::BeforeModelRequest],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
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
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-model-patch".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    assert!(matches!(result, HarnessRunResult::Terminal(_)));
    let request = &model.requests[0];
    assert!(request.prompt.render_text().contains("Hook Context:"));
    assert!(
        request
            .prompt
            .render_text()
            .contains("Prefer the safest concise answer.")
    );
    let model = request.model.as_ref().expect("selected model");
    assert_eq!(model.provider, "test-provider");
    assert_eq!(model.model, "test-model");
    assert_eq!(model.options["temperature"], json!(0.2));
    assert_eq!(model.options["existing"], json!(true));
    assert_eq!(
        request.runtime.model.as_ref().unwrap().options,
        model.options
    );

    let hook_input = hooks.before_model_hooks.first().expect("hook input");
    assert_eq!(hook_input.phase.phase_id, "assess");
    assert_eq!(
        hook_input.phase.completion.explicit_outcomes,
        vec!["execute", "handoff"]
    );
    assert!(
        hook_input
            .sections
            .iter()
            .any(|section| section.title == CONSUMER_RUN_CONTEXT_SECTION_TITLE && section.mutable)
    );
    assert!(
        hook_input
            .sections
            .iter()
            .any(|section| section.title == "HARNESS CONTROL" && !section.mutable)
    );
    let fields = hook_event_fields_for(
        &handle.events(),
        HarnessEventType::HookCompleted,
        "before_model_request",
    );
    assert_eq!(fields["binding_count"], json!(1));
    assert_eq!(fields["model_provider"], json!("test-provider"));
    assert_eq!(fields["model_id"], json!("test-model"));
    assert_eq!(fields["context_sections_added"], json!(1));
    assert_eq!(
        fields["provider_option_patch_keys"],
        json!(["metadata", "temperature"])
    );
    assert_eq!(fields["patched"], json!(true));
}

#[test]
fn before_model_request_hook_fails_closed_before_model_runtime() {
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new([completion("done", "handoff")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        fail_before_model: Some("blocked by policy".into()),
        active_hooks: vec![HarnessHookId::BeforeModelRequest],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
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
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-model".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert_eq!(hooks.before_model_calls, 1);
    assert!(model.requests.is_empty());
    let event_types: Vec<_> = handle
        .events()
        .iter()
        .map(|event| event.event_type)
        .collect();
    assert!(event_types.contains(&HarnessEventType::HookStarted));
    assert!(event_types.contains(&HarnessEventType::HookFailed));
}

#[test]
fn before_model_request_invalid_patch_still_reports_queued_nonfatal_failures() {
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new([completion("done", "handoff")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut provider_options = serde_json::Map::new();
    provider_options.insert("temperature".into(), json!(0.2));
    let mut hooks = TestHookRuntime {
        model_request: Some(BeforeModelRequestDecision {
            context_sections: Vec::new(),
            provider_options,
        }),
        nonfatal_before_model: Some("advisory model hook failed".into()),
        active_hooks: vec![HarnessHookId::BeforeModelRequest],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
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
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-model-invalid-patch".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert!(model.requests.is_empty());
    let events = handle.events();
    let event_types = events
        .iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    let hook_failed_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookFailed)
        .expect("nonfatal hook failed event");
    let phase_failed_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::PhaseFailed)
        .expect("phase failed event");
    assert!(hook_failed_position < phase_failed_position);
    assert_eq!(
        hook_event_fields_for(
            &events,
            HarnessEventType::HookFailed,
            "before_model_request"
        )["nonfatal"],
        json!(true)
    );
}
