use super::*;
#[test]
fn executes_multi_phase_loop_and_accumulates_session_usage() {
    let (result, session, model) = run_engine(
        base_loop(),
        vec![
            completion("a", "execute"),
            completion("b", "review"),
            completion("c", "ready"),
        ],
    );
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.phase_summaries.len(), 3);
    assert_eq!(session.usage.started_runs, 1);
    assert_eq!(session.usage.completed_runs, 1);
    assert_eq!(model.requests.len(), 3);
    assert_eq!(model.requests[1].prior_phase_results.len(), 1);
    assert_eq!(model.requests[1].prior_phase_results[0].loop_step_number, 1);
    assert_eq!(model.requests[2].prior_phase_results[1].loop_step_number, 2);
}

#[test]
fn supports_cycles_and_phase_reentry() {
    let (result, _, _) = run_engine(
        base_loop(),
        vec![
            completion("a", "execute"),
            completion("b", "review"),
            completion("c", "again"),
            completion("d", "review"),
            completion("e", "ready"),
        ],
    );
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Ended);
    let phases: Vec<_> = result
        .report
        .phase_summaries
        .iter()
        .map(|phase| phase.phase_id.as_str())
        .collect();
    assert_eq!(
        phases,
        vec!["assess", "execute", "review", "execute", "review"]
    );
}

#[test]
fn rejects_starting_second_run_while_pending_approval_without_mutating_active_run() {
    let mut loop_manifest = base_loop();
    loop_manifest.r#loop.checkpoints = vec![LoopCheckpoint {
        id: "approve-assess".into(),
        r#type: "approval".into(),
        before_phase: "assess".into(),
        on_reject: "$handoff".into(),
    }];
    let mut engine = HarnessEngine::new(
        loop_manifest,
        HarnessEngineOptions {
            runtime_limits: limits(),
            retain_active_on_approval_required: true,
        },
    );
    let mut session = HarnessSession::new();
    let mut model = ScriptedModelRuntime::new(vec![completion("a", "execute")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    approvals.push("approve-assess", ApprovalDecision::Pending);
    let result = engine
        .execute_run(
            &mut session,
            "first",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::PendingApproval { run_id, .. } = result else {
        panic!("expected pending approval");
    };
    let err = session.start_run("second".into()).unwrap_err();
    assert!(err.to_string().contains(&run_id));
    assert_eq!(session.active_run().unwrap().run_id(), run_id);
    assert_eq!(session.usage.started_runs, 1);
}

#[test]
fn keeps_phase_transcripts_isolated() {
    let (_, _, model) = run_engine(
        base_loop(),
        vec![
            completion("a", "execute"),
            completion("b", "review"),
            completion("c", "ready"),
        ],
    );
    assert_eq!(model.requests[0].transcript.len(), 1);
    assert_eq!(model.requests[1].transcript.len(), 1);
    assert_eq!(model.requests[2].transcript.len(), 1);
    assert_eq!(model.requests[2].prior_phase_results.len(), 2);
}

#[test]
fn model_request_contains_canonical_prompt_sections_and_runtime_snapshot() {
    let mut snapshot = RuntimeSnapshot::empty("session-test".into());
    snapshot.workspace_root = PathBuf::from("/workspace");
    snapshot.state_dir = PathBuf::from("/workspace/.agentpm-state");
    snapshot.runtime_scopes = BTreeMap::from([("user".into(), "user-1".into())]);
    snapshot.consumer_context = Some(crate::harness_runtime::ConsumerContextSnapshot {
        state: "Available".into(),
        file: Some("ops-context.md".into()),
        path: Some(PathBuf::from("/workspace/ops-context.md")),
        content: None,
        byte_size: None,
        approximate_tokens: None,
        sha256: None,
    });
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::with_runtime_snapshot(snapshot);
    let mut model = ScriptedModelRuntime::new(vec![
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    assert!(matches!(result, HarnessRunResult::Terminal(_)));
    let request = &model.requests[0];
    let titles: Vec<_> = request
        .prompt
        .sections
        .iter()
        .map(|section| section.title.as_str())
        .collect();
    assert_eq!(
        titles,
        vec![
            "HARNESS CONTROL",
            "AUTHORED PHASE + BEHAVIOR",
            "CONSUMER / RUN CONTEXT",
            "CROSS-PHASE STATE",
            "EFFECTIVE CAPABILITY CATALOG",
            "CURRENT PHASE-LOCAL TRANSCRIPT"
        ]
    );
    assert_eq!(request.runtime.runtime_scopes["user"], "user-1");
    assert_eq!(request.prompt.action_aliases.len(), 1);
    assert!(request.prompt.render_text().contains("Harness authority"));
}

#[test]
fn effective_phase_injects_profiles_global_then_phase_and_dedupes() {
    let mut snapshot = RuntimeSnapshot::empty("session-test".into());
    snapshot.profiles = vec![
        profile_snapshot("@zack/global-style", "0.1.0", "Global role"),
        profile_snapshot("@zack/phase-style", "0.2.0", "Phase role"),
    ];
    snapshot.profile_bindings = crate::harness_runtime::model::ProfileBindingSnapshot {
        global: vec!["@zack/global-style".into(), "@zack/phase-style".into()],
        phases: BTreeMap::from([(
            "assess".into(),
            vec![
                "@zack/phase-style".into(),
                "@zack/global-style".into(),
                "@zack/missing-style".into(),
            ],
        )]),
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::with_runtime_snapshot(snapshot);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();

    engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let effective = &model.requests[0].effective_phase;
    assert_eq!(
        effective.authored_profile_candidates,
        vec![
            "@zack/global-style",
            "@zack/phase-style",
            "@zack/missing-style"
        ]
    );
    assert_eq!(effective.active_profiles.len(), 2);
    assert_eq!(effective.suppressed_capabilities.len(), 1);
    let prompt = model.requests[0].prompt.render_text();
    let global_index = prompt.find("Profile: @zack/global-style@0.1.0").unwrap();
    let phase_index = prompt.find("Profile: @zack/phase-style@0.2.0").unwrap();
    assert!(global_index < phase_index);
    assert_eq!(prompt.matches("Profile: @zack/global-style").count(), 1);
    assert!(prompt.contains("[required] cite-evidence"));
    assert!(prompt.contains("Preferred vocabulary"));
    let effective_event = handle
        .events()
        .into_iter()
        .find(|event| event.event_type == HarnessEventType::EffectivePhaseComputed)
        .unwrap();
    let HarnessEventPayload::Lifecycle { fields, .. } = effective_event.payload else {
        panic!("expected lifecycle payload");
    };
    assert_eq!(
        fields["suppressed_profiles"][0]["identity"],
        "@zack/missing-style"
    );
}

#[test]
fn consumer_context_is_snapshotted_per_run_and_reloaded_next_run() {
    let path = temp_context_file("reload", "first run context\n");
    let mut snapshot = RuntimeSnapshot::empty("session-test".into());
    snapshot.consumer_context = Some(crate::harness_runtime::ConsumerContextSnapshot {
        state: "Available".into(),
        file: Some("context.md".into()),
        path: Some(path.clone()),
        content: None,
        byte_size: None,
        approximate_tokens: None,
        sha256: None,
    });
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::with_runtime_snapshot(snapshot);
    let mut first_model = MutatingModelRuntime::new(
        vec![
            completion("a", "execute"),
            completion("b", "review"),
            completion("c", "ready"),
        ],
        path.clone(),
        "edited during first run\n",
    );
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let first_result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut first_model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(first_result) = first_result else {
        panic!("expected terminal result");
    };
    let report_context = first_result.report.consumer_context.as_ref().unwrap();
    assert_eq!(report_context.status, "loaded");
    assert!(report_context.byte_size.is_some());
    assert!(report_context.approximate_tokens.is_some());
    assert!(
        first_model
            .requests
            .iter()
            .all(|request| request.prompt.render_text().contains("first run context"))
    );
    assert!(first_model.requests.iter().all(|request| {
        !request
            .prompt
            .render_text()
            .contains("edited during first run")
    }));

    session.clear_terminal_active_run();
    let mut second_model = ScriptedModelRuntime::new(vec![
        completion("d", "execute"),
        completion("e", "review"),
        completion("f", "ready"),
    ]);
    engine
        .execute_run(
            &mut session,
            "hello",
            &mut second_model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    assert!(
        second_model.requests[0]
            .prompt
            .render_text()
            .contains("edited during first run")
    );
    let context = second_model.requests[0]
        .runtime
        .consumer_context
        .as_ref()
        .unwrap();
    assert!(context.byte_size.is_some());
    assert!(context.approximate_tokens.is_some());
    assert!(context.sha256.as_deref().unwrap().starts_with("sha256:"));
}

#[test]
fn missing_consumer_context_without_resolved_path_is_evented_and_non_fatal() {
    let mut snapshot = RuntimeSnapshot::empty("session-test".into());
    snapshot.consumer_context = Some(crate::harness_runtime::ConsumerContextSnapshot {
        state: "Unavailable".into(),
        file: Some("missing-context.md".into()),
        path: None,
        content: None,
        byte_size: None,
        approximate_tokens: None,
        sha256: None,
    });
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::with_runtime_snapshot(snapshot);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();

    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    assert!(matches!(result, HarnessRunResult::Terminal(_)));
    assert!(
        model.requests[0]
            .prompt
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.contains("consumer context content is not loaded") })
    );
    assert_consumer_context_unavailable_event(&handle);
}

#[test]
fn consumer_context_deleted_before_run_start_is_evented_and_non_fatal() {
    let path = temp_context_file("deleted", "deleted before run\n");
    std::fs::remove_file(&path).unwrap();
    let mut snapshot = RuntimeSnapshot::empty("session-test".into());
    snapshot.consumer_context = Some(crate::harness_runtime::ConsumerContextSnapshot {
        state: "Available".into(),
        file: Some("context.md".into()),
        path: Some(path),
        content: None,
        byte_size: Some(19),
        approximate_tokens: Some(5),
        sha256: Some("sha256:preflight".into()),
    });
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::with_runtime_snapshot(snapshot);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();

    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(
        result.report.consumer_context.as_ref().unwrap().status,
        "unavailable"
    );
    assert!(
        !model.requests[0]
            .prompt
            .render_text()
            .contains("deleted before run")
    );
    assert_consumer_context_unavailable_event(&handle);
}

fn assert_consumer_context_unavailable_event(handle: &InMemoryEventSink) {
    assert!(
        handle
            .events()
            .iter()
            .any(|event| event.event_type == HarnessEventType::ConsumerContextUnavailable)
    );
}

#[test]
fn multi_turn_phase_processes_multiple_ordered_actions() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = session_with_tool_and_skill();
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: Some("I will act.".into()),
            actions: vec![
                SemanticActionProposal::new(
                    "tool-1",
                    SemanticAction::AgentPmTool {
                        tool: "@zack/search".into(),
                        arguments: json!({ "query": "incident" }),
                    },
                ),
                SemanticActionProposal::new(
                    "skill-1",
                    SemanticAction::SkillResourceRead {
                        skill: "@zack/skill".into(),
                        resource: "entrypoint".into(),
                    },
                ),
            ],
            usage: RunUsage::default(),
            finish_reason: None,
            provider_metadata: BTreeMap::new(),
        },
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    assert!(matches!(result, HarnessRunResult::Terminal(_)));
    assert_eq!(dispatcher.dispatched.len(), 2);
    assert_eq!(dispatcher.dispatched[0].identity(), "@zack/search");
    assert_eq!(
        dispatcher.dispatched[1].identity(),
        "@zack/skill/entrypoint"
    );
    assert_eq!(model.requests.len(), 4);
    assert!(model.requests[1].transcript.len() > model.requests[0].transcript.len());
}

#[test]
fn invalid_tool_arguments_request_repair_before_dispatch() {
    let mut runtime_limits = limits();
    runtime_limits.max_structured_output_repairs = 0;
    runtime_limits.max_tool_call_repairs = 1;
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(runtime_limits));
    let mut session = session_with_tool_and_skill();
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "bad-tool",
                SemanticAction::AgentPmTool {
                    tool: "@zack/search".into(),
                    arguments: json!({ "query": 42 }),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: None,
            provider_metadata: BTreeMap::new(),
        },
        tool_turn("@zack/search"),
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();

    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.repair_count, 1);
    assert_eq!(dispatcher.dispatched.len(), 1);
    assert_eq!(dispatcher.dispatched[0].identity(), "@zack/search");
    assert!(
        model.requests[1]
            .prompt
            .render_text()
            .contains("arguments are invalid")
    );
}

#[test]
fn tool_call_repair_limit_exhaustion_fails_before_dispatch() {
    let mut runtime_limits = limits();
    runtime_limits.max_tool_call_repairs = 0;
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(runtime_limits));
    let mut session = session_with_tool_and_skill();
    let mut model = ScriptedModelRuntime::new(vec![ModelTurn {
        assistant_content: None,
        actions: vec![SemanticActionProposal::new(
            "bad-tool",
            SemanticAction::AgentPmTool {
                tool: "@zack/search".into(),
                arguments: json!({ "query": 42 }),
            },
        )],
        usage: RunUsage::default(),
        finish_reason: None,
        provider_metadata: BTreeMap::new(),
    }]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();

    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert_eq!(result.report.repair_count, 0);
    assert_eq!(
        result.output,
        Some(json!({ "error": "tool call repair limit exhausted" }))
    );
    assert!(dispatcher.dispatched.is_empty());
}

#[test]
fn two_tools_preserve_order_and_validate_against_each_tool_schema() {
    let mut runtime_limits = limits();
    runtime_limits.max_tool_call_repairs = 1;
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(runtime_limits));
    let mut session = HarnessSession::with_runtime_snapshot(runtime_with_two_tools_and_skill());
    let mut model = ScriptedModelRuntime::new(vec![
        tool_turn_with_arguments("@zack/comment", json!({ "query": "wrong schema" })),
        tool_turn("@zack/search"),
        tool_turn_with_arguments("@zack/comment", json!({ "body": "ready" })),
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();

    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.repair_count, 1);
    let tool_descriptors = model.requests[0]
        .effective_phase
        .capability_catalog
        .iter()
        .filter(|descriptor| descriptor.action_kind == "agentpm_tool")
        .map(|descriptor| descriptor.identity.as_str())
        .collect::<Vec<_>>();
    assert_eq!(tool_descriptors, vec!["@zack/search", "@zack/comment"]);
    assert_eq!(dispatcher.dispatched.len(), 2);
    assert_eq!(dispatcher.dispatched[0].identity(), "@zack/search");
    assert_eq!(dispatcher.dispatched[1].identity(), "@zack/comment");
    assert!(
        model.requests[1]
            .prompt
            .render_text()
            .contains("Tool `@zack/comment` arguments are invalid")
    );
}

#[test]
fn phase_scoped_tool_candidates_are_active_only_for_matching_phase() {
    let mut runtime = runtime_with_two_tools_and_skill();
    runtime.capability_candidates = vec![
        RuntimeCapabilitySnapshot {
            kind: "tool".into(),
            identity: "@zack/search".into(),
            scope: "phase:assess".into(),
            source: "agent_binding".into(),
            state: "available".into(),
        },
        RuntimeCapabilitySnapshot {
            kind: "tool".into(),
            identity: "@zack/comment".into(),
            scope: "phase:execute".into(),
            source: "agent_binding".into(),
            state: "available".into(),
        },
    ];

    let loop_manifest = base_loop();
    let assess = EffectivePhase::from_phase(&loop_manifest.r#loop.phases[0], &runtime);
    let execute = EffectivePhase::from_phase(&loop_manifest.r#loop.phases[1], &runtime);

    assert!(
        assess
            .capability_catalog
            .iter()
            .any(|descriptor| descriptor.action_kind == "agentpm_tool"
                && descriptor.identity == "@zack/search")
    );
    assert!(
        !assess
            .capability_catalog
            .iter()
            .any(|descriptor| descriptor.action_kind == "agentpm_tool"
                && descriptor.identity == "@zack/comment")
    );
    assert!(
        execute
            .capability_catalog
            .iter()
            .any(|descriptor| descriptor.action_kind == "agentpm_tool"
                && descriptor.identity == "@zack/comment")
    );
}

#[test]
fn schema_valid_tool_domain_failure_is_returned_to_phase_transcript() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = session_with_tool_and_skill();
    let mut model = ScriptedModelRuntime::new(vec![
        tool_turn("@zack/search"),
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    dispatcher.push_result(
        "@zack/search",
        ActionDispatchResult::success(json!({
            "ok": false,
            "error": "domain-level failure",
            "reason": "manual_review_required"
        })),
    );
    let mut approvals = ScriptedApprovalController::default();

    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Ended);
    assert_eq!(dispatcher.dispatched.len(), 1);
    let next_prompt = model.requests[1].prompt.render_text();
    assert!(next_prompt.contains("ActionResult [agentpm_tool @zack/search]"));
    assert!(next_prompt.contains("\"ok\":false"));
    assert!(next_prompt.contains("domain-level failure"));
}

#[test]
fn knowledge_backend_failure_is_returned_to_phase_transcript() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
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
    let mut knowledge = RecordingKnowledgeRuntime::new(ActionDispatchResult::success(json!({
        "ok": false,
        "package": "@zack/guide",
        "version": "0.1.0",
        "mode": "vector_query",
        "query": "alpha",
        "error": {
            "code": "knowledge_backend_down",
            "message": "backend unavailable",
            "retryable": true
        }
    })));
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = NoopHookRuntime;
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
            "run-knowledge-backend-failure".into(),
            "hello",
            &mut services,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Ended);
    assert_eq!(knowledge.dispatched.len(), 1);
    assert!(model.requests.len() >= 2);
    let next_prompt = model.requests[1].prompt.render_text();
    assert!(next_prompt.contains("ActionResult [knowledge_request @zack/guide]"));
    assert!(next_prompt.contains("\"ok\":false"));
    assert!(next_prompt.contains("knowledge_backend_down"));
    assert!(next_prompt.contains("backend unavailable"));
    let event_types = handle
        .events()
        .iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    assert!(event_types.contains(&HarnessEventType::KnowledgeFailed));
    assert!(!event_types.contains(&HarnessEventType::PhaseFailed));
}

#[test]
fn skill_resource_content_is_loaded_on_demand_and_phase_local() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        skill_read_turn("@zack/skill", "entrypoint"),
        skill_read_turn("@zack/skill", "references/handoff-template.md"),
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    dispatcher.push_result(
        "@zack/skill/entrypoint",
        ActionDispatchResult::success(json!({
            "action_kind": "skill_resource_read",
            "ok": true,
            "skill": "@zack/skill",
            "resource": "entrypoint",
            "content": "Use concise handoff guidance."
        })),
    );
    dispatcher.push_result(
        "@zack/skill/references/handoff-template.md",
        ActionDispatchResult::success(json!({
            "action_kind": "skill_resource_read",
            "ok": true,
            "skill": "@zack/skill",
            "resource": "references/handoff-template.md",
            "content": "Use the handoff template."
        })),
    );
    let mut approvals = ScriptedApprovalController::default();

    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    assert!(matches!(result, HarnessRunResult::Terminal(_)));
    let prompt_with_one_resource = model.requests[1].prompt.render_text();
    assert!(prompt_with_one_resource.contains("Loaded resource: entrypoint"));
    assert!(prompt_with_one_resource.contains("Use concise handoff guidance."));
    assert!(
        prompt_with_one_resource
            .contains("ActionResult [skill_resource_read @zack/skill/entrypoint]")
    );
    let prompt_with_grouped_resources = model.requests[2].prompt.render_text();
    assert_eq!(
        prompt_with_grouped_resources
            .matches("Skill: @zack/skill")
            .count(),
        1
    );
    assert!(prompt_with_grouped_resources.contains("Loaded resource: entrypoint"));
    assert!(
        prompt_with_grouped_resources.contains("Loaded resource: references/handoff-template.md")
    );
    assert!(prompt_with_grouped_resources.contains("Use concise handoff guidance."));
    assert!(prompt_with_grouped_resources.contains("Use the handoff template."));
    assert!(
        !model.requests[3]
            .prompt
            .render_text()
            .contains("Use concise handoff guidance.")
    );
    let event_types: Vec<_> = handle
        .events()
        .iter()
        .map(|event| event.event_type)
        .collect();
    assert!(event_types.contains(&HarnessEventType::SkillActivated));
    assert!(event_types.contains(&HarnessEventType::SkillResourceRequested));
    assert!(event_types.contains(&HarnessEventType::SkillResourceLoaded));
    let skill_requested = handle
        .events()
        .into_iter()
        .find(|event| event.event_type == HarnessEventType::SkillResourceRequested)
        .unwrap();
    let HarnessEventPayload::Action { fields, .. } = skill_requested.payload else {
        panic!("expected action payload");
    };
    assert_eq!(fields["source"], "agent_binding");
}

#[test]
fn unavailable_tool_is_suppressed_from_effective_phase() {
    let mut runtime = runtime_with_tool_and_skill();
    runtime.tools[0].state = "unavailable".into();
    runtime.capability_candidates[0].state = "unavailable".into();
    let phase = &base_loop().r#loop.phases[0];
    let effective = EffectivePhase::from_phase(phase, &runtime);

    assert!(
        !effective
            .capability_catalog
            .iter()
            .any(|descriptor| descriptor.action_kind == "agentpm_tool")
    );
    assert!(
        effective
            .suppressed_capabilities
            .iter()
            .any(|capability| capability.kind == "tool" && capability.identity == "@zack/search")
    );
}

#[test]
fn knowledge_packages_remain_distinct_model_visible_surfaces() {
    let runtime = runtime_with_knowledge_packages(&["@zack/alpha", "@zack/beta"]);
    let phase = &base_loop().r#loop.phases[0];
    let effective = EffectivePhase::from_phase(phase, &runtime);

    let knowledge_descriptors = effective
        .capability_catalog
        .iter()
        .filter(|descriptor| descriptor.action_kind == "knowledge_request")
        .map(|descriptor| descriptor.identity.as_str())
        .collect::<Vec<_>>();
    assert_eq!(knowledge_descriptors, vec!["@zack/alpha", "@zack/beta"]);
    assert_eq!(
        effective
            .active_knowledge
            .iter()
            .map(|knowledge| knowledge.name.as_str())
            .collect::<Vec<_>>(),
        vec!["@zack/alpha", "@zack/beta"]
    );
    assert!(
        !effective
            .capability_catalog
            .iter()
            .any(|descriptor| descriptor.action_kind == "knowledge_request"
                && descriptor.identity == "knowledge")
    );
}

#[test]
fn loop_access_suppresses_skill_inherited_tools_but_not_skill_resources() {
    let mut runtime = runtime_with_tool_and_skill();
    runtime.capability_candidates = vec![
        RuntimeCapabilitySnapshot {
            kind: "skill".into(),
            identity: "@zack/skill".into(),
            scope: "global".into(),
            source: "agent_binding".into(),
            state: "available".into(),
        },
        RuntimeCapabilitySnapshot {
            kind: "tool".into(),
            identity: "@zack/search".into(),
            scope: "global".into(),
            source: "skill:@zack/skill".into(),
            state: "available".into(),
        },
    ];
    let mut loop_manifest = base_loop();
    loop_manifest.r#loop.phases[0].access = Some(LoopPhaseAccess {
        tools: Some(false),
        knowledge: None,
        memory: None,
    });

    let effective = EffectivePhase::from_phase(&loop_manifest.r#loop.phases[0], &runtime);

    assert!(
        effective
            .capability_catalog
            .iter()
            .any(|descriptor| descriptor.action_kind == "skill_resource_read"
                && descriptor.identity == "@zack/skill")
    );
    assert!(
        !effective
            .capability_catalog
            .iter()
            .any(|descriptor| descriptor.action_kind == "agentpm_tool")
    );
    assert!(
        effective
            .suppressed_capabilities
            .iter()
            .any(|capability| capability.kind == "tool"
                && capability.identity == "@zack/search"
                && capability.source == "skill:@zack/skill"
                && capability.reason == "Loop access.tools=false for this phase")
    );
}

#[test]
fn ambiguous_completion_plus_action_requests_repair_without_executing_action() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::new();
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![
                SemanticActionProposal::new(
                    "tool",
                    SemanticAction::AgentPmTool {
                        tool: "@zack/search".into(),
                        arguments: json!({}),
                    },
                ),
                SemanticActionProposal::new(
                    "complete",
                    SemanticAction::PhaseCompletion {
                        outcome: Some("execute".into()),
                        output: None,
                    },
                ),
            ],
            usage: RunUsage::default(),
            finish_reason: None,
            provider_metadata: BTreeMap::new(),
        },
        completion("repair", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.repair_count, 1);
    assert!(dispatcher.dispatched.is_empty());
    assert_eq!(
        model.requests[1].repair_feedback.as_deref(),
        Some("A phase completion proposal cannot be combined with executable actions.")
    );
}

#[test]
fn loop_access_gates_tools_knowledge_and_memory_but_not_skill_resources() {
    let mut loop_manifest = base_loop();
    loop_manifest.r#loop.phases[0].access = Some(LoopPhaseAccess {
        tools: Some(false),
        knowledge: Some(false),
        memory: Some(LoopAccessMemory {
            read: Some(false),
            write: Some(false),
        }),
    });
    let mut engine = HarnessEngine::new(loop_manifest, HarnessEngineOptions::new(limits()));
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        tool_turn("@zack/search"),
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "skill",
                SemanticAction::SkillResourceRead {
                    skill: "@zack/skill".into(),
                    resource: "entrypoint".into(),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: None,
            provider_metadata: BTreeMap::new(),
        },
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    assert!(matches!(result, HarnessRunResult::Terminal(_)));
    assert_eq!(dispatcher.dispatched.len(), 1);
    assert!(matches!(
        dispatcher.dispatched[0],
        SemanticAction::SkillResourceRead { .. }
    ));
    let events = handle.events();
    let tool_candidates = events
        .iter()
        .find(|event| event.event_type == HarnessEventType::ToolCandidatesComputed)
        .unwrap();
    let HarnessEventPayload::Lifecycle { fields, .. } = &tool_candidates.payload else {
        panic!("expected lifecycle payload");
    };
    assert_eq!(fields["suppressed"][0]["identity"], "@zack/search");
    assert_eq!(fields["suppressed"][0]["source"], "agent_binding");
    assert_eq!(
        fields["suppressed"][0]["reason"],
        "Loop access.tools=false for this phase"
    );
}

#[test]
fn approval_checkpoints_are_ordered_and_first_rejection_routes() {
    let mut loop_manifest = base_loop();
    loop_manifest.r#loop.checkpoints = vec![
        LoopCheckpoint {
            id: "first".into(),
            r#type: "approval".into(),
            before_phase: "execute".into(),
            on_reject: "$abort".into(),
        },
        LoopCheckpoint {
            id: "second".into(),
            r#type: "approval".into(),
            before_phase: "execute".into(),
            on_reject: "$handoff".into(),
        },
    ];
    let mut engine = HarnessEngine::new(loop_manifest, HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::new();
    let mut model = ScriptedModelRuntime::new(vec![completion("a", "execute")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    approvals.push("first", ApprovalDecision::Approve);
    approvals.push("second", ApprovalDecision::Deny);
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::HandedOff);
    let checkpoints: Vec<_> = result
        .report
        .checkpoint_summaries
        .iter()
        .map(|checkpoint| (&checkpoint.checkpoint_id, &checkpoint.status))
        .collect();
    assert_eq!(
        checkpoints,
        vec![
            (&"first".into(), &"approved".into()),
            (&"second".into(), &"denied".into())
        ]
    );
}

#[test]
fn implicit_and_invalid_explicit_outcomes_are_handled_with_repair() {
    let mut loop_manifest = base_loop();
    loop_manifest.r#loop.phases[2].outcomes.clear();
    loop_manifest.r#loop.transitions.push(LoopTransition {
        from: "review".into(),
        on: "complete".into(),
        to: "$end".into(),
    });
    let (result, _, model) = run_engine(
        loop_manifest,
        vec![
            completion("a", "bogus"),
            completion("repair", "execute"),
            completion("b", "review"),
            ModelTurn {
                assistant_content: Some("implicit".into()),
                actions: Vec::new(),
                usage: RunUsage::default(),
                finish_reason: None,
                provider_metadata: BTreeMap::new(),
            },
        ],
    );
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.repair_count, 1);
    assert!(
        model.requests[1]
            .repair_feedback
            .as_deref()
            .unwrap_or_default()
            .contains("not declared")
    );
}

#[test]
fn max_step_exhaustion_returns_limit_reached() {
    let mut loop_manifest = base_loop();
    loop_manifest.r#loop.limits = Some(LoopLimits { max_steps: Some(2) });
    let (result, _, _) = run_engine(
        loop_manifest,
        vec![
            completion("a", "execute"),
            completion("b", "review"),
            completion("c", "again"),
        ],
    );
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::LimitReached);
}

#[test]
fn tool_retry_counts_additional_attempts_after_initial_failure() {
    let mut loop_manifest = base_loop();
    loop_manifest.r#loop.error_policy = Some(LoopErrorPolicy {
        tool_failure: Some(LoopToolFailurePolicy {
            action: LoopToolFailureAction::Retry,
            max_retries: Some(2),
            on_exhausted: Some(LoopToolFailureExhaustedAction::FailPhase),
        }),
        phase_failure: None,
    });
    let mut engine = HarnessEngine::new(loop_manifest, HarnessEngineOptions::new(limits()));
    let mut session = session_with_tool_and_skill();
    let mut model = ScriptedModelRuntime::new(vec![
        tool_turn("@zack/search"),
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    dispatcher.push_result("@zack/search", ActionDispatchResult::failure("temporary"));
    dispatcher.push_result(
        "@zack/search",
        ActionDispatchResult::success(json!({"results": ["cached hit"]})),
    );
    let mut approvals = ScriptedApprovalController::default();
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.retry_count, 1);
    assert_eq!(result.report.usage.tool_retries, 1);
    assert_eq!(dispatcher.dispatched.len(), 2);
    let next_prompt = model.requests[1].prompt.render_text();
    assert!(next_prompt.contains("ActionResult [agentpm_tool @zack/search]"));
    assert!(next_prompt.contains(SUCCESSFUL_ACTION_RESULT_CONTROL));
}

#[test]
fn default_action_failure_becomes_runtime_failed() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = session_with_tool_and_skill();
    let mut model = ScriptedModelRuntime::new(vec![tool_turn("@zack/search")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    dispatcher.push_result("@zack/search", ActionDispatchResult::failure("boom"));
    let mut approvals = ScriptedApprovalController::default();
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert_eq!(result.report.error_count, 1);
}

#[test]
fn direct_tool_failure_policies_route_with_structured_terminal_status() {
    let (fail_phase_result, _) = run_tool_failure_policy(
        LoopToolFailurePolicy {
            action: LoopToolFailureAction::FailPhase,
            max_retries: None,
            on_exhausted: None,
        },
        vec![ActionDispatchResult::failure("$abort")],
    );
    assert_eq!(fail_phase_result.status, HarnessTerminalStatus::Failed);
    assert_eq!(fail_phase_result.output, Some(json!({ "error": "$abort" })));

    let (abort_result, _) = run_tool_failure_policy(
        LoopToolFailurePolicy {
            action: LoopToolFailureAction::Abort,
            max_retries: None,
            on_exhausted: None,
        },
        vec![ActionDispatchResult::failure("tool failed")],
    );
    assert_eq!(abort_result.status, HarnessTerminalStatus::Aborted);

    let (handoff_result, _) = run_tool_failure_policy(
        LoopToolFailurePolicy {
            action: LoopToolFailureAction::Handoff,
            max_retries: None,
            on_exhausted: None,
        },
        vec![ActionDispatchResult::failure("tool failed")],
    );
    assert_eq!(handoff_result.status, HarnessTerminalStatus::HandedOff);
}

#[test]
fn retry_exhaustion_tool_policies_route_terminal_status() {
    let (fail_phase_result, fail_phase_dispatcher) = run_tool_failure_policy(
        LoopToolFailurePolicy {
            action: LoopToolFailureAction::Retry,
            max_retries: Some(1),
            on_exhausted: Some(LoopToolFailureExhaustedAction::FailPhase),
        },
        vec![
            ActionDispatchResult::failure("temporary"),
            ActionDispatchResult::failure("still failing"),
        ],
    );
    assert_eq!(fail_phase_result.status, HarnessTerminalStatus::Failed);
    assert_eq!(fail_phase_result.report.retry_count, 1);
    assert_eq!(fail_phase_dispatcher.dispatched.len(), 2);

    let (abort_result, abort_dispatcher) = run_tool_failure_policy(
        LoopToolFailurePolicy {
            action: LoopToolFailureAction::Retry,
            max_retries: Some(1),
            on_exhausted: Some(LoopToolFailureExhaustedAction::Abort),
        },
        vec![
            ActionDispatchResult::failure("temporary"),
            ActionDispatchResult::failure("still failing"),
        ],
    );
    assert_eq!(abort_result.status, HarnessTerminalStatus::Aborted);
    assert_eq!(abort_result.report.retry_count, 1);
    assert_eq!(abort_dispatcher.dispatched.len(), 2);

    let (handoff_result, handoff_dispatcher) = run_tool_failure_policy(
        LoopToolFailurePolicy {
            action: LoopToolFailureAction::Retry,
            max_retries: Some(1),
            on_exhausted: Some(LoopToolFailureExhaustedAction::Handoff),
        },
        vec![
            ActionDispatchResult::failure("temporary"),
            ActionDispatchResult::failure("still failing"),
        ],
    );
    assert_eq!(handoff_result.status, HarnessTerminalStatus::HandedOff);
    assert_eq!(handoff_result.report.retry_count, 1);
    assert_eq!(handoff_dispatcher.dispatched.len(), 2);
}

#[test]
fn deterministic_tool_failure_categories_do_not_retry() {
    let mut loop_manifest = base_loop();
    loop_manifest.r#loop.error_policy = Some(LoopErrorPolicy {
        tool_failure: Some(LoopToolFailurePolicy {
            action: LoopToolFailureAction::Retry,
            max_retries: Some(2),
            on_exhausted: Some(LoopToolFailureExhaustedAction::FailPhase),
        }),
        phase_failure: None,
    });
    let mut engine = HarnessEngine::new(loop_manifest, HarnessEngineOptions::new(limits()));
    let mut session = session_with_tool_and_skill();
    let mut model = ScriptedModelRuntime::new(vec![tool_turn("@zack/search")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    dispatcher.push_result(
        "@zack/search",
        ActionDispatchResult::failure_with_category(
            ActionFailureCategory::Schema,
            "authoritative Tool input schema rejected arguments",
        ),
    );
    let mut approvals = ScriptedApprovalController::default();

    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert_eq!(result.report.retry_count, 0);
    assert_eq!(dispatcher.dispatched.len(), 1);
}

#[test]
fn terminal_tool_failure_status_does_not_retry() {
    let (result, dispatcher) = run_tool_failure_policy(
        LoopToolFailurePolicy {
            action: LoopToolFailureAction::Retry,
            max_retries: Some(2),
            on_exhausted: Some(LoopToolFailureExhaustedAction::FailPhase),
        },
        vec![ActionDispatchResult::terminal_failure(
            HarnessTerminalStatus::Cancelled,
            "ToolRuntime cancelled agentpm run for `@zack/search`",
        )],
    );

    assert_eq!(result.status, HarnessTerminalStatus::Cancelled);
    assert_eq!(result.report.retry_count, 0);
    assert_eq!(result.report.usage.tool_retries, 0);
    assert_eq!(dispatcher.dispatched.len(), 1);
}

#[test]
fn multiple_sequential_runs_reuse_session_and_reset_run_state() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::new();
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();

    let mut first_model = ScriptedModelRuntime::new(vec![
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let first = engine
        .execute_run(
            &mut session,
            "first",
            &mut first_model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(first) = first else {
        panic!("expected terminal first run");
    };

    let mut second_model = ScriptedModelRuntime::new(vec![
        completion("d", "execute"),
        completion("e", "review"),
        completion("f", "ready"),
    ]);
    let second = engine
        .execute_run(
            &mut session,
            "second",
            &mut second_model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(second) = second else {
        panic!("expected terminal second run");
    };

    assert_ne!(first.report.run_id, second.report.run_id);
    assert_eq!(session.usage.started_runs, 2);
    assert_eq!(session.usage.completed_runs, 2);
    assert_eq!(first_model.requests[0].prior_phase_results.len(), 0);
    assert_eq!(second_model.requests[0].prior_phase_results.len(), 0);
}

#[test]
fn unresolved_approval_terminalizes_as_approval_required_when_not_retained() {
    let mut loop_manifest = base_loop();
    loop_manifest.r#loop.checkpoints = vec![LoopCheckpoint {
        id: "approve-assess".into(),
        r#type: "approval".into(),
        before_phase: "assess".into(),
        on_reject: "$handoff".into(),
    }];
    let mut engine = HarnessEngine::new(loop_manifest, HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::new();
    let mut model = ScriptedModelRuntime::new(vec![completion("a", "execute")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    approvals.push("approve-assess", ApprovalDecision::Pending);

    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal approval-required");
    };
    assert_eq!(result.status, HarnessTerminalStatus::ApprovalRequired);
    assert_eq!(result.report.checkpoint_summaries[0].status, "pending");
    assert!(session.active_run().is_none());
}

#[test]
fn authored_abort_and_cancellation_have_distinct_terminal_statuses() {
    let mut abort_loop = base_loop();
    abort_loop.r#loop.transitions[1].to = "$abort".into();
    let (abort_result, _, _) = run_engine(abort_loop, vec![completion("a", "handoff")]);
    let HarnessRunResult::Terminal(abort_result) = abort_result else {
        panic!("expected terminal abort");
    };
    assert_eq!(abort_result.status, HarnessTerminalStatus::Aborted);

    let mut engine = HarnessEngine::new(
        base_loop(),
        HarnessEngineOptions {
            runtime_limits: limits(),
            retain_active_on_approval_required: true,
        },
    );
    let mut session = HarnessSession::new();
    session.start_run("cancel me".into()).unwrap();
    let cancelled = engine
        .cancel_active_run(&mut session, "test cancellation")
        .unwrap();
    assert_eq!(cancelled.status, HarnessTerminalStatus::Cancelled);
    assert_eq!(session.usage.completed_runs, 1);
}

#[test]
fn phase_failure_policy_can_abort_or_handoff() {
    let mut abort_loop = base_loop();
    abort_loop.r#loop.error_policy = Some(LoopErrorPolicy {
        tool_failure: None,
        phase_failure: Some(LoopPhaseFailurePolicy {
            action: LoopPhaseFailureAction::Abort,
        }),
    });
    let (abort_result, _, _) = {
        let mut engine = HarnessEngine::new(abort_loop, HarnessEngineOptions::new(limits()));
        let mut session = HarnessSession::new();
        let mut model =
            ScriptedModelRuntime::with_results(vec![Err(ModelRuntimeFailure::new("model down"))]);
        let mut dispatcher = ScriptedActionDispatcher::default();
        let mut approvals = ScriptedApprovalController::default();
        let result = engine
            .execute_run(
                &mut session,
                "hello",
                &mut model,
                &mut dispatcher,
                &mut approvals,
            )
            .unwrap();
        (result, session, model)
    };
    let HarnessRunResult::Terminal(abort_result) = abort_result else {
        panic!("expected terminal abort");
    };
    assert_eq!(abort_result.status, HarnessTerminalStatus::Aborted);

    let mut handoff_loop = base_loop();
    handoff_loop.r#loop.error_policy = Some(LoopErrorPolicy {
        tool_failure: None,
        phase_failure: Some(LoopPhaseFailurePolicy {
            action: LoopPhaseFailureAction::Handoff,
        }),
    });
    let (handoff_result, _, _) = {
        let mut engine = HarnessEngine::new(handoff_loop, HarnessEngineOptions::new(limits()));
        let mut session = HarnessSession::new();
        let mut model =
            ScriptedModelRuntime::with_results(vec![Err(ModelRuntimeFailure::new("model down"))]);
        let mut dispatcher = ScriptedActionDispatcher::default();
        let mut approvals = ScriptedApprovalController::default();
        let result = engine
            .execute_run(
                &mut session,
                "hello",
                &mut model,
                &mut dispatcher,
                &mut approvals,
            )
            .unwrap();
        (result, session, model)
    };
    let HarnessRunResult::Terminal(handoff_result) = handoff_result else {
        panic!("expected terminal handoff");
    };
    assert_eq!(handoff_result.status, HarnessTerminalStatus::HandedOff);
}

#[test]
fn runtime_limits_cover_model_action_tool_and_repair_exhaustion() {
    let mut model_limit = limits();
    model_limit.max_model_calls_per_phase = 1;
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(model_limit));
    let mut session = session_with_tool_and_skill();
    let mut model =
        ScriptedModelRuntime::new(vec![tool_turn("@zack/search"), completion("a", "execute")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal limit");
    };
    assert_eq!(result.status, HarnessTerminalStatus::LimitReached);

    let mut action_limit = limits();
    action_limit.max_actions_per_phase = 0;
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(action_limit));
    let mut session = session_with_tool_and_skill();
    let mut model = ScriptedModelRuntime::new(vec![completion("a", "execute")]);
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal limit");
    };
    assert_eq!(result.status, HarnessTerminalStatus::LimitReached);

    let mut tool_limit = limits();
    tool_limit.max_tool_calls_per_phase = 0;
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(tool_limit));
    let mut session = session_with_tool_and_skill();
    let mut model = ScriptedModelRuntime::new(vec![tool_turn("@zack/search")]);
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal limit");
    };
    assert_eq!(result.status, HarnessTerminalStatus::LimitReached);

    let mut repair_limit = limits();
    repair_limit.max_structured_output_repairs = 0;
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(repair_limit));
    let mut session = HarnessSession::new();
    let mut model = ScriptedModelRuntime::new(vec![completion("bad", "missing")]);
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal failed");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert_eq!(result.report.repair_count, 0);
}

#[test]
fn report_and_events_include_phase_transition_action_and_usage_data() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        tool_turn("@zack/search"),
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.action_summaries.len(), 1);
    assert_eq!(result.report.tool_summaries.len(), 1);
    assert_eq!(result.report.usage.accepted_semantic_actions, 4);
    let events = handle.events();
    let event_types: Vec<_> = events.iter().map(|event| event.event_type).collect();
    assert!(event_types.contains(&HarnessEventType::RunStarted));
    assert!(event_types.contains(&HarnessEventType::ToolCandidatesComputed));
    assert!(event_types.contains(&HarnessEventType::PhaseStarted));
    assert!(event_types.contains(&HarnessEventType::SemanticActionProposed));
    assert!(event_types.contains(&HarnessEventType::ToolInvoked));
    assert!(event_types.contains(&HarnessEventType::ToolCompleted));
    assert!(event_types.contains(&HarnessEventType::TransitionSelected));
    assert!(event_types.contains(&HarnessEventType::RunCompleted));
    assert!(event_types.contains(&HarnessEventType::SessionUsageUpdated));
    let tool_invoked = events
        .iter()
        .find(|event| event.event_type == HarnessEventType::ToolInvoked)
        .unwrap();
    assert_eq!(
        tool_invoked.phase_execution_id.as_deref(),
        Some("phase-exec-1")
    );
    let HarnessEventPayload::Action { fields, .. } = &tool_invoked.payload else {
        panic!("expected action payload");
    };
    assert_eq!(fields["source"], "agent_binding");
}

#[test]
fn knowledge_dispatch_usage_is_reported_and_rolled_up() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::new();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    session
        .runtime_snapshot
        .knowledge
        .push(KnowledgeRuntimeSnapshot {
            name: "@zack/guide".into(),
            version: "0.1.0".into(),
            mode: "vector".into(),
            description: "Guide".into(),
            root: None,
            source: "agent_binding".into(),
            state: "available".into(),
            runtime: "local".into(),
            readiness_reason: None,
            documents: Vec::new(),
            embedding: Some(KnowledgeEmbeddingSnapshot {
                id: "default".into(),
                provider: "manual".into(),
                model: "toy-3d".into(),
                dimensions: 3,
                metric: "cosine".into(),
                normalized: true,
            }),
            retrieval: None,
        });
    session
        .runtime_snapshot
        .capability_candidates
        .push(RuntimeCapabilitySnapshot {
            kind: "knowledge".into(),
            identity: "@zack/guide".into(),
            scope: "global".into(),
            source: "agent_binding".into(),
            state: "available".into(),
        });
    let usage = RunUsage {
        embedding_requests: 1,
        ..Default::default()
    };
    let mut knowledge = UsageKnowledgeRuntime {
        result: ActionDispatchResult::success(json!({
            "ok": true,
            "package": "@zack/guide",
            "version": "0.1.0",
            "mode": "vector_query",
            "query": "alpha",
            "results": [],
            "citations": []
        }))
        .with_usage(usage),
    };
    let mut model = ScriptedModelRuntime::new(vec![
        knowledge_query_turn("@zack/guide"),
        completion("assess-complete", "execute"),
        completion("execute-complete", "review"),
        completion("review-complete", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = NoopHookRuntime;
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
        .execute_run_with_id(&mut session, "run-usage".into(), "hello", &mut services)
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };

    assert_eq!(result.report.usage.knowledge_requests, 1);
    assert_eq!(result.report.usage.embedding_requests, 1);
    assert_eq!(session.usage.knowledge_requests, 1);
    assert_eq!(session.usage.embedding_requests, 1);
    let events = handle.events();
    let event_types: Vec<_> = events.iter().map(|event| event.event_type).collect();
    assert!(event_types.contains(&HarnessEventType::EmbeddingRequestStarted));
    assert!(event_types.contains(&HarnessEventType::EmbeddingRequestCompleted));
    let embedding_started = events
        .iter()
        .find(|event| event.event_type == HarnessEventType::EmbeddingRequestStarted)
        .unwrap();
    let HarnessEventPayload::Action { fields, .. } = &embedding_started.payload else {
        panic!("expected embedding action payload");
    };
    assert_eq!(fields["package"], "@zack/guide");
    assert_eq!(fields["provider"], "manual");
    assert_eq!(fields["model"], "toy-3d");
    assert_eq!(fields["dimensions"], 3);
    assert_eq!(fields["normalized"], true);
    assert!(fields.get("duration_ms").is_none());
    let embedding_completed = events
        .iter()
        .find(|event| event.event_type == HarnessEventType::EmbeddingRequestCompleted)
        .unwrap();
    let HarnessEventPayload::Action { fields, .. } = &embedding_completed.payload else {
        panic!("expected embedding action payload");
    };
    assert!(fields["duration_ms"].as_u64().is_some());
}

#[test]
fn embedding_provider_failures_emit_embedding_failed_event() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::new();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    session
        .runtime_snapshot
        .knowledge
        .push(KnowledgeRuntimeSnapshot {
            name: "@zack/guide".into(),
            version: "0.1.0".into(),
            mode: "vector".into(),
            description: "Guide".into(),
            root: None,
            source: "agent_binding".into(),
            state: "available".into(),
            runtime: "local".into(),
            readiness_reason: None,
            documents: Vec::new(),
            embedding: Some(KnowledgeEmbeddingSnapshot {
                id: "default".into(),
                provider: "manual".into(),
                model: "toy-3d".into(),
                dimensions: 3,
                metric: "cosine".into(),
                normalized: true,
            }),
            retrieval: None,
        });
    session
        .runtime_snapshot
        .capability_candidates
        .push(RuntimeCapabilitySnapshot {
            kind: "knowledge".into(),
            identity: "@zack/guide".into(),
            scope: "global".into(),
            source: "agent_binding".into(),
            state: "available".into(),
        });
    let usage = RunUsage {
        embedding_requests: 1,
        ..Default::default()
    };
    let mut knowledge = UsageKnowledgeRuntime {
        result: ActionDispatchResult::success(json!({
            "ok": false,
            "package": "@zack/guide",
            "version": "0.1.0",
            "mode": "vector_query",
            "query": "alpha",
            "results": [],
            "citations": [],
            "error": {
                "code": "embedding_provider_failed",
                "message": "provider unavailable",
                "retryable": false
            }
        }))
        .with_usage(usage),
    };
    let mut model = ScriptedModelRuntime::new(vec![
        knowledge_query_turn("@zack/guide"),
        completion("assess-complete", "execute"),
        completion("execute-complete", "review"),
        completion("review-complete", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = NoopHookRuntime;
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
            "run-embedding-failed".into(),
            "hello",
            &mut services,
        )
        .unwrap();
    assert!(matches!(result, HarnessRunResult::Terminal(_)));

    let events = handle.events();
    let event_types: Vec<_> = events.iter().map(|event| event.event_type).collect();
    assert!(event_types.contains(&HarnessEventType::EmbeddingRequestStarted));
    assert!(event_types.contains(&HarnessEventType::EmbeddingRequestFailed));
    assert!(event_types.contains(&HarnessEventType::KnowledgeFailed));
    assert!(!event_types.contains(&HarnessEventType::EmbeddingRequestCompleted));
    let embedding_failed = events
        .iter()
        .find(|event| event.event_type == HarnessEventType::EmbeddingRequestFailed)
        .unwrap();
    let HarnessEventPayload::Action { fields, .. } = &embedding_failed.payload else {
        panic!("expected embedding action payload");
    };
    assert_eq!(fields["error_code"], "embedding_provider_failed");
    assert!(fields["duration_ms"].as_u64().is_some());
}

#[test]
fn non_tool_action_failures_emit_specific_failed_event_types() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::new();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    session
        .runtime_snapshot
        .knowledge
        .push(KnowledgeRuntimeSnapshot {
            name: "@zack/guide".into(),
            version: "0.1.0".into(),
            mode: "vector".into(),
            description: "Guide".into(),
            root: None,
            source: "agent_binding".into(),
            state: "available".into(),
            runtime: "local".into(),
            readiness_reason: None,
            documents: Vec::new(),
            embedding: None,
            retrieval: None,
        });
    session
        .runtime_snapshot
        .knowledge
        .push(KnowledgeRuntimeSnapshot {
            name: "@zack/stale-guide".into(),
            version: "0.1.0".into(),
            mode: "vector".into(),
            description: "Stale guide".into(),
            root: None,
            source: "agent_binding".into(),
            state: "unavailable".into(),
            runtime: "local".into(),
            readiness_reason: Some("vector index is stale".into()),
            documents: Vec::new(),
            embedding: None,
            retrieval: None,
        });
    session
        .runtime_snapshot
        .capability_candidates
        .push(RuntimeCapabilitySnapshot {
            kind: "knowledge".into(),
            identity: "@zack/guide".into(),
            scope: "global".into(),
            source: "agent_binding".into(),
            state: "available".into(),
        });
    session
        .runtime_snapshot
        .capability_candidates
        .push(RuntimeCapabilitySnapshot {
            kind: "knowledge".into(),
            identity: "@zack/stale-guide".into(),
            scope: "global".into(),
            source: "agent_binding".into(),
            state: "unavailable".into(),
        });
    let mut model = ScriptedModelRuntime::new(vec![ModelTurn {
        assistant_content: None,
        actions: vec![SemanticActionProposal::new(
            "knowledge",
            SemanticAction::KnowledgeRequest {
                package: "@zack/guide".into(),
                mode: None,
                document: None,
                query: Some("x".into()),
                top_k: None,
                score_threshold: None,
                return_citations: None,
            },
        )],
        usage: RunUsage::default(),
        finish_reason: None,
        provider_metadata: BTreeMap::new(),
    }]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    dispatcher.push_result("@zack/guide", ActionDispatchResult::failure("unavailable"));
    let mut approvals = ScriptedApprovalController::default();
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert_eq!(
        result.report.knowledge_summaries,
        vec![OperationReportSummary {
            operation_kind: "knowledge_request".into(),
            identity: "@zack/guide".into(),
            status: "failed".into(),
            count: 1,
        }]
    );
    let events = handle.events();
    let event_types: Vec<_> = events.iter().map(|event| event.event_type).collect();
    assert!(event_types.contains(&HarnessEventType::KnowledgeSurfaceReady));
    assert!(event_types.contains(&HarnessEventType::KnowledgeSurfaceUnavailable));
    assert!(event_types.contains(&HarnessEventType::KnowledgeFailed));
    assert!(!event_types.contains(&HarnessEventType::SemanticActionCompleted));
    let unavailable = events
        .iter()
        .find(|event| event.event_type == HarnessEventType::KnowledgeSurfaceUnavailable)
        .unwrap();
    let HarnessEventPayload::Action { fields, .. } = &unavailable.payload else {
        panic!("expected action payload");
    };
    assert_eq!(fields["reason"], "vector index is stale");
}
