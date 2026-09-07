use super::*;

const REVIEW_COMPLETE_IDENTITY: &str = "harness/persistence_review";

#[derive(Debug, Clone, Copy)]
struct MemoryWriteReviewSelection {
    point: HarnessMemoryWriteReviewPoint,
}

#[derive(Debug, Default)]
struct MemoryWriteReviewStats {
    model_calls: u64,
    memory_reads_attempted: u64,
    memory_reads_completed: u64,
    memory_writes_attempted: u64,
    memory_writes_completed: u64,
}

impl HarnessEngine {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_memory_write_review_if_configured(
        &self,
        session: &mut HarnessSession,
        phase: &LoopPhase,
        phase_execution_id: &str,
        selected_outcome: &str,
        transition_target: &str,
        effective_phase: &EffectivePhase,
        phase_state: &PhaseExecutionState,
        model: &mut dyn ModelRuntime,
        custom_memory_runtime: &mut Option<CustomMemoryRuntime>,
        embedding_provider: &mut Option<Box<dyn crate::harness_runtime::EmbeddingProvider>>,
        hooks: &mut dyn HookRuntime,
        service_events: &mut Option<&mut ServiceLifecycleEvents>,
    ) -> Result<()> {
        let Some(selection) = self.memory_write_review_selection(transition_target) else {
            return Ok(());
        };
        let run_id = self.active_run(session)?.run_id().to_string();
        let point = memory_write_review_point_label(selection.point);
        let review_phase = persistence_review_effective_phase(effective_phase);
        let mut stats = MemoryWriteReviewStats::default();

        if !review_phase
            .capability_catalog
            .iter()
            .any(|descriptor| descriptor.action_kind == "memory_write")
        {
            self.emit_memory_write_review_terminal_event(
                session,
                HarnessEventType::MemoryWriteReviewSkipped,
                &run_id,
                phase_execution_id,
                point,
                selected_outcome,
                "no_writable_memory_surface",
                &stats,
            )?;
            return Ok(());
        }

        session.emitter.emit(
            HarnessEventType::MemoryWriteReviewStarted,
            HarnessEventPayload::Lifecycle {
                message: "Memory write review started.".into(),
                fields: memory_write_review_fields(point, selected_outcome, "started", &stats),
            },
            HarnessEventBuilder {
                run_id: Some(run_id.clone()),
                phase_execution_id: Some(phase_execution_id.to_string()),
                ..HarnessEventBuilder::default()
            },
        )?;

        let mut state = PhaseExecutionState {
            phase_execution_id: phase_execution_id.to_string(),
            phase_id: phase.id.clone(),
            transcript: phase_state.transcript.clone(),
            model_calls: 0,
            accepted_actions: 0,
            logical_tool_calls: 0,
            structured_repairs: 0,
            tool_call_repairs: 0,
        };
        let mut repair_feedback = None;

        loop {
            if state.model_calls >= self.options.runtime_limits.max_model_calls_per_phase {
                self.emit_memory_write_review_terminal_event(
                    session,
                    HarnessEventType::MemoryWriteReviewFailed,
                    &run_id,
                    phase_execution_id,
                    point,
                    selected_outcome,
                    "max_model_calls_per_phase",
                    &stats,
                )?;
                return Ok(());
            }

            let prompt = assemble_logical_prompt(PromptAssemblyInput {
                purpose: PromptAssemblyPurpose::MemoryWriteReview {
                    point,
                    pending_outcome: selected_outcome,
                },
                phase_id: &phase.id,
                phase_objective: &phase.objective,
                explicit_outcomes: &[],
                run_input: &self.active_run(session)?.context.input,
                consumer_context: self
                    .active_run(session)?
                    .context
                    .runtime
                    .consumer_context
                    .as_ref(),
                prior_phase_results: &self.active_run(session)?.phase_results,
                effective_phase: &review_phase,
                transcript: &state.transcript,
                repair_feedback: repair_feedback.as_deref(),
            });
            let mut request = ModelRequest {
                runtime: self.active_run(session)?.context.runtime.clone(),
                model: self.active_run(session)?.context.runtime.model.clone(),
                prompt,
                run_id: run_id.clone(),
                phase_execution_id: phase_execution_id.to_string(),
                phase_id: phase.id.clone(),
                phase_objective: phase.objective.clone(),
                run_input: self.active_run(session)?.context.input.clone(),
                prior_phase_results: self.active_run(session)?.phase_results.clone(),
                transcript: state.transcript.clone(),
                effective_phase: review_phase.clone(),
                repair_feedback: repair_feedback.clone(),
            };

            session.emitter.emit(
                HarnessEventType::PromptPrepared,
                HarnessEventPayload::Lifecycle {
                    message: "Canonical model prompt prepared.".into(),
                    fields: BTreeMap::from([
                        ("phase_id".into(), json!(phase.id)),
                        ("review_point".into(), json!(point)),
                        ("sections".into(), json!(request.prompt.sections.len())),
                        ("prompt".into(), json!(request.prompt.render_text())),
                        (
                            "action_descriptors".into(),
                            json!(request.prompt.action_aliases.len()),
                        ),
                    ]),
                },
                HarnessEventBuilder {
                    run_id: Some(run_id.clone()),
                    phase_execution_id: Some(phase_execution_id.to_string()),
                    ..HarnessEventBuilder::default()
                },
            )?;

            if let Err(reason) = self.apply_before_model_request_for_review(
                session,
                hooks,
                &run_id,
                phase,
                phase_execution_id,
                &mut request,
            ) {
                self.emit_memory_write_review_terminal_event(
                    session,
                    HarnessEventType::MemoryWriteReviewFailed,
                    &run_id,
                    phase_execution_id,
                    point,
                    selected_outcome,
                    &reason,
                    &stats,
                )?;
                return Ok(());
            }

            self.emit_service_lifecycle_events(session, Some(&run_id), service_events)?;
            if let Some(snapshot) = model.inspect_request(&request) {
                session.emitter.emit(
                    HarnessEventType::ModelRuntimeRequestPrepared,
                    HarnessEventPayload::Lifecycle {
                        message: "Model runtime request prepared.".into(),
                        fields: snapshot.into_trace_fields()?,
                    },
                    HarnessEventBuilder {
                        run_id: Some(run_id.clone()),
                        phase_execution_id: Some(phase_execution_id.to_string()),
                        ..HarnessEventBuilder::default()
                    },
                )?;
            }
            session.emitter.emit(
                HarnessEventType::ModelRequestStarted,
                HarnessEventPayload::Lifecycle {
                    message: "Model request started.".into(),
                    fields: BTreeMap::from([
                        ("phase_id".into(), json!(phase.id)),
                        ("review_point".into(), json!(point)),
                    ]),
                },
                HarnessEventBuilder {
                    run_id: Some(run_id.clone()),
                    phase_execution_id: Some(phase_execution_id.to_string()),
                    ..HarnessEventBuilder::default()
                },
            )?;
            state.model_calls += 1;
            stats.model_calls += 1;
            self.active_run_mut(session)?.usage.model_calls += 1;
            let turn = match model.generate(request) {
                Ok(turn) => turn,
                Err(err) => {
                    self.emit_service_lifecycle_events(session, Some(&run_id), service_events)?;
                    session.emitter.emit(
                        HarnessEventType::ModelRequestFailed,
                        HarnessEventPayload::Lifecycle {
                            message: err.message.clone(),
                            fields: BTreeMap::from([("review_point".into(), json!(point))]),
                        },
                        HarnessEventBuilder {
                            run_id: Some(run_id.clone()),
                            phase_execution_id: Some(phase_execution_id.to_string()),
                            ..HarnessEventBuilder::default()
                        },
                    )?;
                    self.emit_memory_write_review_terminal_event(
                        session,
                        HarnessEventType::MemoryWriteReviewFailed,
                        &run_id,
                        phase_execution_id,
                        point,
                        selected_outcome,
                        "model_request_failed",
                        &stats,
                    )?;
                    return Ok(());
                }
            };
            self.emit_service_lifecycle_events(session, Some(&run_id), service_events)?;
            self.merge_usage(session, &turn.usage);
            session.emitter.emit(
                HarnessEventType::ModelRequestCompleted,
                HarnessEventPayload::Lifecycle {
                    message: "Model request completed.".into(),
                    fields: model_turn_trace_fields(&turn),
                },
                HarnessEventBuilder {
                    run_id: Some(run_id.clone()),
                    phase_execution_id: Some(phase_execution_id.to_string()),
                    ..HarnessEventBuilder::default()
                },
            )?;
            if let Some(content) = &turn.assistant_content {
                state.transcript.push(TranscriptEntry {
                    kind: TranscriptEntryKind::Assistant,
                    content: json!(content),
                    action_succeeded: None,
                });
            }

            if turn.actions.is_empty() {
                repair_feedback = Some(
                    "Memory write review must propose authorized Memory actions or persistence_review_complete."
                        .to_string(),
                );
                if self
                    .request_repair(
                        session,
                        &mut state,
                        phase_execution_id,
                        repair_feedback.clone(),
                    )
                    .is_err()
                {
                    self.emit_memory_write_review_terminal_event(
                        session,
                        HarnessEventType::MemoryWriteReviewFailed,
                        &run_id,
                        phase_execution_id,
                        point,
                        selected_outcome,
                        "structured_output_repair_limit",
                        &stats,
                    )?;
                    return Ok(());
                }
                continue;
            }

            if turn.actions.iter().any(|proposal| {
                matches!(proposal.action, SemanticAction::PersistenceReviewComplete)
            }) {
                if turn.actions.len() == 1 {
                    self.emit_memory_write_review_terminal_event(
                        session,
                        HarnessEventType::MemoryWriteReviewCompleted,
                        &run_id,
                        phase_execution_id,
                        point,
                        selected_outcome,
                        "completed",
                        &stats,
                    )?;
                    return Ok(());
                }
                repair_feedback = Some(
                    "persistence_review_complete cannot be combined with Memory actions."
                        .to_string(),
                );
                if self
                    .request_repair(
                        session,
                        &mut state,
                        phase_execution_id,
                        repair_feedback.clone(),
                    )
                    .is_err()
                {
                    self.emit_memory_write_review_terminal_event(
                        session,
                        HarnessEventType::MemoryWriteReviewFailed,
                        &run_id,
                        phase_execution_id,
                        point,
                        selected_outcome,
                        "structured_output_repair_limit",
                        &stats,
                    )?;
                    return Ok(());
                }
                continue;
            }

            let mut requested_repair = false;
            for proposal in turn.actions {
                if !matches!(
                    proposal.action,
                    SemanticAction::MemoryRead { .. } | SemanticAction::MemoryWrite { .. }
                ) {
                    session.emitter.emit(
                        HarnessEventType::SemanticActionRejected,
                        HarnessEventPayload::Action {
                            action_kind: proposal.action.kind().into(),
                            identity: proposal.action.identity(),
                            status: "prohibited_by_review".into(),
                            fields: BTreeMap::new(),
                        },
                        HarnessEventBuilder {
                            run_id: Some(run_id.clone()),
                            phase_execution_id: Some(phase_execution_id.to_string()),
                            ..HarnessEventBuilder::default()
                        },
                    )?;
                    repair_feedback = Some(
                        "Memory write review can only use Memory actions or persistence_review_complete."
                            .to_string(),
                    );
                    requested_repair = true;
                    break;
                }
                if !review_phase.permits(&proposal.action) {
                    session.emitter.emit(
                        HarnessEventType::SemanticActionRejected,
                        HarnessEventPayload::Action {
                            action_kind: proposal.action.kind().into(),
                            identity: proposal.action.identity(),
                            status: "prohibited_by_loop_access".into(),
                            fields: BTreeMap::new(),
                        },
                        HarnessEventBuilder {
                            run_id: Some(run_id.clone()),
                            phase_execution_id: Some(phase_execution_id.to_string()),
                            ..HarnessEventBuilder::default()
                        },
                    )?;
                    repair_feedback = Some("Action is not permitted by Loop access.".to_string());
                    requested_repair = true;
                    break;
                }
                if let Err(err) = validate_semantic_action(&proposal.action, &review_phase) {
                    session.emitter.emit(
                        HarnessEventType::SemanticActionRejected,
                        HarnessEventPayload::Action {
                            action_kind: proposal.action.kind().into(),
                            identity: proposal.action.identity(),
                            status: "invalid_arguments".into(),
                            fields: BTreeMap::from([("error".into(), json!(err.clone()))]),
                        },
                        HarnessEventBuilder {
                            run_id: Some(run_id.clone()),
                            phase_execution_id: Some(phase_execution_id.to_string()),
                            ..HarnessEventBuilder::default()
                        },
                    )?;
                    repair_feedback = Some(err);
                    requested_repair = true;
                    break;
                }
                if state.accepted_actions >= self.options.runtime_limits.max_actions_per_phase {
                    self.emit_memory_write_review_terminal_event(
                        session,
                        HarnessEventType::MemoryWriteReviewFailed,
                        &run_id,
                        phase_execution_id,
                        point,
                        selected_outcome,
                        "max_actions_per_phase",
                        &stats,
                    )?;
                    return Ok(());
                }

                state.accepted_actions += 1;
                self.active_run_mut(session)?
                    .usage
                    .accepted_semantic_actions += 1;
                let action = match self.apply_memory_hook_for_review(
                    session,
                    hooks,
                    &run_id,
                    phase,
                    phase_execution_id,
                    &review_phase,
                    &proposal.action,
                ) {
                    Ok(action) => action,
                    Err(reason) => {
                        self.emit_memory_write_review_terminal_event(
                            session,
                            HarnessEventType::MemoryWriteReviewFailed,
                            &run_id,
                            phase_execution_id,
                            point,
                            selected_outcome,
                            &reason,
                            &stats,
                        )?;
                        return Ok(());
                    }
                };
                let action_source = action_source(&action, &review_phase);
                let mut action_fields = action_trace_fields(&action);
                if let Some(source) = &action_source {
                    action_fields.insert("source".into(), json!(source));
                }
                session.emitter.emit(
                    HarnessEventType::SemanticActionProposed,
                    HarnessEventPayload::Action {
                        action_kind: action.kind().into(),
                        identity: action.identity(),
                        status: "accepted".into(),
                        fields: action_fields,
                    },
                    HarnessEventBuilder {
                        run_id: Some(run_id.clone()),
                        phase_execution_id: Some(phase_execution_id.to_string()),
                        ..HarnessEventBuilder::default()
                    },
                )?;
                match action {
                    SemanticAction::MemoryRead { .. } => stats.memory_reads_attempted += 1,
                    SemanticAction::MemoryWrite { .. } => stats.memory_writes_attempted += 1,
                    _ => {}
                }
                let result = match self.dispatch_memory(
                    session,
                    &review_phase,
                    &action,
                    custom_memory_runtime,
                    embedding_provider,
                    action_source.as_deref(),
                    phase_execution_id,
                ) {
                    Ok(result) => result,
                    Err(err) => {
                        self.emit_memory_write_review_terminal_event(
                            session,
                            HarnessEventType::MemoryWriteReviewFailed,
                            &run_id,
                            phase_execution_id,
                            point,
                            selected_outcome,
                            &err.to_string(),
                            &stats,
                        )?;
                        return Ok(());
                    }
                };
                self.emit_service_lifecycle_events(session, Some(&run_id), service_events)?;
                let output_failed = result
                    .output
                    .get("ok")
                    .and_then(Value::as_bool)
                    .is_some_and(|ok| !ok);

                if !result.ok
                    && matches!(action, SemanticAction::MemoryWrite { .. })
                    && result.failure_category == Some(ActionFailureCategory::Schema)
                {
                    let error = result.error.unwrap_or_else(|| "action failed".to_string());
                    session.emitter.emit(
                        HarnessEventType::SemanticActionRejected,
                        HarnessEventPayload::Action {
                            action_kind: action.kind().into(),
                            identity: action.identity(),
                            status: "invalid_arguments".into(),
                            fields: BTreeMap::from([("error".into(), json!(error.clone()))]),
                        },
                        HarnessEventBuilder {
                            run_id: Some(run_id.clone()),
                            phase_execution_id: Some(phase_execution_id.to_string()),
                            ..HarnessEventBuilder::default()
                        },
                    )?;
                    repair_feedback = Some(error);
                    requested_repair = true;
                    break;
                }

                let action_ok = result.ok && !output_failed;
                if action_ok {
                    match action {
                        SemanticAction::MemoryRead { .. } => stats.memory_reads_completed += 1,
                        SemanticAction::MemoryWrite { .. } => stats.memory_writes_completed += 1,
                        _ => {}
                    }
                }
                state.transcript.push(TranscriptEntry {
                    kind: TranscriptEntryKind::ActionResult,
                    content: action_result_transcript_content(&action, result.output.clone()),
                    action_succeeded: Some(action_ok),
                });
                self.active_run_mut(session)?
                    .action_summaries
                    .push(ActionReportSummary {
                        action_kind: action.kind().into(),
                        identity: action.identity(),
                        status: if action_ok { "completed" } else { "failed" }.into(),
                        error: if action_ok {
                            None
                        } else {
                            result
                                .output
                                .get("error")
                                .and_then(|error| error.get("message"))
                                .and_then(Value::as_str)
                                .map(str::to_string)
                        },
                    });
            }

            if requested_repair {
                if self
                    .request_repair(
                        session,
                        &mut state,
                        phase_execution_id,
                        repair_feedback.clone(),
                    )
                    .is_err()
                {
                    self.emit_memory_write_review_terminal_event(
                        session,
                        HarnessEventType::MemoryWriteReviewFailed,
                        &run_id,
                        phase_execution_id,
                        point,
                        selected_outcome,
                        "structured_output_repair_limit",
                        &stats,
                    )?;
                    return Ok(());
                }
            } else {
                repair_feedback = None;
            }
        }
    }

    fn memory_write_review_selection(
        &self,
        transition_target: &str,
    ) -> Option<MemoryWriteReviewSelection> {
        let terminal = self.terminal_for_target(transition_target).is_some();
        let configured = &self.options.memory_write_review_points;
        if terminal && configured.contains(&HarnessMemoryWriteReviewPoint::RunEnd) {
            return Some(MemoryWriteReviewSelection {
                point: HarnessMemoryWriteReviewPoint::RunEnd,
            });
        }
        if configured.contains(&HarnessMemoryWriteReviewPoint::PhaseEnd) {
            return Some(MemoryWriteReviewSelection {
                point: HarnessMemoryWriteReviewPoint::PhaseEnd,
            });
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_before_model_request_for_review(
        &self,
        session: &mut HarnessSession,
        hooks: &mut dyn HookRuntime,
        run_id: &str,
        phase: &LoopPhase,
        phase_execution_id: &str,
        request: &mut ModelRequest,
    ) -> std::result::Result<(), String> {
        let hook = HarnessHookId::BeforeModelRequest;
        let binding_count = hooks.binding_count(&hook);
        let enabled = binding_count > 0;
        let mut fields = BTreeMap::from([
            (
                "section_count_before".into(),
                json!(request.prompt.sections.len()),
            ),
            (
                "mutable_section_count".into(),
                json!(mutable_model_request_sections(request)),
            ),
            (
                "action_descriptor_count".into(),
                json!(request.prompt.action_aliases.len()),
            ),
            (
                "repair_feedback_present".into(),
                json!(request.repair_feedback.is_some()),
            ),
            (
                "provider_option_keys_before".into(),
                json!(provider_option_keys(request)),
            ),
            ("review".into(), json!(true)),
        ]);
        if let Some(model) = &request.model {
            fields.insert("model_provider".into(), json!(model.provider.clone()));
            fields.insert("model_id".into(), json!(model.model.clone()));
        }
        if enabled {
            self.emit_hook_started(
                session,
                HookEventContext {
                    run_id,
                    phase_id: &phase.id,
                    phase_execution_id,
                    hook: &hook,
                    binding_count,
                },
                fields.clone(),
            )
            .map_err(|err| err.to_string())?;
        }
        match hooks.before_model_request(before_model_request_hook_from_request(request)) {
            Ok(decision) => {
                let context_sections_added = decision.context_sections.len();
                let mut provider_option_patch_keys = decision
                    .provider_options
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>();
                provider_option_patch_keys.sort();
                let patched = context_sections_added > 0 || !provider_option_patch_keys.is_empty();
                apply_before_model_request_decision(request, decision)?;
                if enabled {
                    self.emit_nonfatal_hook_failures(
                        session,
                        run_id,
                        &phase.id,
                        phase_execution_id,
                        hooks,
                    )
                    .map_err(|err| err.to_string())?;
                    let mut completed_fields = fields;
                    completed_fields.insert(
                        "section_count_after".into(),
                        json!(request.prompt.sections.len()),
                    );
                    completed_fields.insert(
                        "provider_option_keys_after".into(),
                        json!(provider_option_keys(request)),
                    );
                    completed_fields.insert(
                        "context_sections_added".into(),
                        json!(context_sections_added),
                    );
                    completed_fields.insert(
                        "provider_option_patch_keys".into(),
                        json!(provider_option_patch_keys),
                    );
                    completed_fields.insert("patched".into(), json!(patched));
                    self.emit_hook_completed(
                        session,
                        HookEventContext {
                            run_id,
                            phase_id: &phase.id,
                            phase_execution_id,
                            hook: &hook,
                            binding_count,
                        },
                        completed_fields,
                    )
                    .map_err(|err| err.to_string())?;
                }
                Ok(())
            }
            Err(err) => {
                let is_rejection = err.is_rejection();
                self.emit_nonfatal_hook_failures(
                    session,
                    run_id,
                    &phase.id,
                    phase_execution_id,
                    hooks,
                )
                .map_err(|emit_err| emit_err.to_string())?;
                session
                    .emitter
                    .emit(
                        if is_rejection {
                            HarnessEventType::HookRejected
                        } else {
                            HarnessEventType::HookFailed
                        },
                        HarnessEventPayload::Lifecycle {
                            message: err.message.clone(),
                            fields: hook_event_fields(&hook, &phase.id, binding_count, fields),
                        },
                        HarnessEventBuilder {
                            run_id: Some(run_id.to_string()),
                            phase_execution_id: Some(phase_execution_id.to_string()),
                            ..HarnessEventBuilder::default()
                        },
                    )
                    .map_err(|emit_err| emit_err.to_string())?;
                Err(format!(
                    "before_model_request hook {} during Memory write review: {}",
                    if is_rejection { "rejected" } else { "failed" },
                    err.message
                ))
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_memory_hook_for_review(
        &self,
        session: &mut HarnessSession,
        hooks: &mut dyn HookRuntime,
        run_id: &str,
        phase: &LoopPhase,
        phase_execution_id: &str,
        effective_phase: &EffectivePhase,
        action: &SemanticAction,
    ) -> std::result::Result<SemanticAction, String> {
        let hook = match action {
            SemanticAction::MemoryRead { .. } => HarnessHookId::BeforeMemoryRead,
            SemanticAction::MemoryWrite { .. } => HarnessHookId::BeforeMemoryWrite,
            _ => return Ok(action.clone()),
        };
        let binding_count = hooks.binding_count(&hook);
        let enabled = binding_count > 0;
        if enabled {
            self.emit_hook_started(
                session,
                HookEventContext {
                    run_id,
                    phase_id: &phase.id,
                    phase_execution_id,
                    hook: &hook,
                    binding_count,
                },
                action_trace_fields(action),
            )
            .map_err(|err| err.to_string())?;
        }

        let (patched_action, patched) = match action {
            SemanticAction::MemoryRead {
                package,
                space,
                mode,
                record_id,
                record_type,
                filter,
                query,
                limit,
            } => {
                let memory = active_memory_space(effective_phase, package, space)
                    .ok_or_else(|| format!("Memory space `{space}` is not active"))?;
                let scope = memory_actions::resolved_memory_scope(
                    memory,
                    &session.runtime_snapshot.runtime_scopes,
                )
                .map_err(|err| err.to_string())?;
                match hooks.before_memory_read(BeforeMemoryReadHook {
                    phase_id: phase.id.clone(),
                    package: package.clone(),
                    space: space.clone(),
                    scope: json!(scope),
                    record_id: record_id.clone(),
                    record_type: record_type.clone(),
                    query: query.clone(),
                    filter: (!filter.is_empty()).then(|| json!(filter)),
                    limit: *limit,
                    mode: Some(memory_read_mode_label(*mode).into()),
                    retrieval_modes: memory
                        .retrieval_modes
                        .iter()
                        .map(memory_actions::memory_retrieval_mode_label)
                        .map(str::to_string)
                        .collect(),
                }) {
                    Ok(decision) => {
                        let patched = decision.query.is_some()
                            || decision.filter.is_some()
                            || decision.limit.is_some()
                            || decision.mode.is_some();
                        (
                            memory_actions::apply_before_memory_read_decision_to_action(
                                action, decision,
                            )?,
                            patched,
                        )
                    }
                    Err(err) => {
                        return self.emit_review_hook_failure(
                            session,
                            hooks,
                            run_id,
                            phase,
                            phase_execution_id,
                            &hook,
                            binding_count,
                            action,
                            err,
                        );
                    }
                }
            }
            SemanticAction::MemoryWrite {
                package,
                space,
                operation,
                record_type,
                record_id,
                content,
            } => {
                let memory = active_memory_space(effective_phase, package, space)
                    .ok_or_else(|| format!("Memory space `{space}` is not active"))?;
                let scope = memory_actions::resolved_memory_scope(
                    memory,
                    &session.runtime_snapshot.runtime_scopes,
                )
                .map_err(|err| err.to_string())?;
                match hooks.before_memory_write(BeforeMemoryWriteHook {
                    phase_id: phase.id.clone(),
                    package: package.clone(),
                    space: space.clone(),
                    operation: memory_write_operation_label(*operation).into(),
                    record_type: record_type.clone(),
                    record_id: record_id.clone(),
                    scope: json!(scope),
                    content: content.clone().unwrap_or(Value::Null),
                }) {
                    Ok(decision) => {
                        let patched = decision.content.is_some();
                        (
                            memory_actions::apply_before_memory_write_decision_to_action(
                                action, decision,
                            )?,
                            patched,
                        )
                    }
                    Err(err) => {
                        return self.emit_review_hook_failure(
                            session,
                            hooks,
                            run_id,
                            phase,
                            phase_execution_id,
                            &hook,
                            binding_count,
                            action,
                            err,
                        );
                    }
                }
            }
            _ => return Ok(action.clone()),
        };

        if let Err(err) = validate_semantic_action(&patched_action, effective_phase) {
            return Err(format!(
                "{} hook produced invalid action during Memory write review: {err}",
                hook_id_label(&hook)
            ));
        }
        if enabled {
            self.emit_nonfatal_hook_failures(session, run_id, &phase.id, phase_execution_id, hooks)
                .map_err(|err| err.to_string())?;
            let mut fields = action_trace_fields(&patched_action);
            fields.insert("patched".into(), json!(patched));
            self.emit_hook_completed(
                session,
                HookEventContext {
                    run_id,
                    phase_id: &phase.id,
                    phase_execution_id,
                    hook: &hook,
                    binding_count,
                },
                fields,
            )
            .map_err(|err| err.to_string())?;
        }
        Ok(patched_action)
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_review_hook_failure(
        &self,
        session: &mut HarnessSession,
        hooks: &mut dyn HookRuntime,
        run_id: &str,
        phase: &LoopPhase,
        phase_execution_id: &str,
        hook: &HarnessHookId,
        binding_count: usize,
        action: &SemanticAction,
        err: crate::harness_runtime::hook::HookRuntimeFailure,
    ) -> std::result::Result<SemanticAction, String> {
        let is_rejection = err.is_rejection();
        self.emit_nonfatal_hook_failures(session, run_id, &phase.id, phase_execution_id, hooks)
            .map_err(|emit_err| emit_err.to_string())?;
        session
            .emitter
            .emit(
                if is_rejection {
                    HarnessEventType::HookRejected
                } else {
                    HarnessEventType::HookFailed
                },
                HarnessEventPayload::Lifecycle {
                    message: err.message.clone(),
                    fields: hook_event_fields(
                        hook,
                        &phase.id,
                        binding_count,
                        action_trace_fields(action),
                    ),
                },
                HarnessEventBuilder {
                    run_id: Some(run_id.to_string()),
                    phase_execution_id: Some(phase_execution_id.to_string()),
                    ..HarnessEventBuilder::default()
                },
            )
            .map_err(|emit_err| emit_err.to_string())?;
        Err(format!(
            "{} hook {} during Memory write review: {}",
            hook_id_label(hook),
            if is_rejection { "rejected" } else { "failed" },
            err.message
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_memory_write_review_terminal_event(
        &self,
        session: &mut HarnessSession,
        event_type: HarnessEventType,
        run_id: &str,
        phase_execution_id: &str,
        point: &str,
        selected_outcome: &str,
        reason: &str,
        stats: &MemoryWriteReviewStats,
    ) -> Result<()> {
        let message = match event_type {
            HarnessEventType::MemoryWriteReviewCompleted => "Memory write review completed.",
            HarnessEventType::MemoryWriteReviewSkipped => "Memory write review skipped.",
            HarnessEventType::MemoryWriteReviewFailed => "Memory write review failed.",
            _ => "Memory write review event.",
        };
        if matches!(
            event_type,
            HarnessEventType::MemoryWriteReviewCompleted
                | HarnessEventType::MemoryWriteReviewSkipped
                | HarnessEventType::MemoryWriteReviewFailed
        ) {
            self.active_run_mut(session)?
                .memory_write_review_summaries
                .push(MemoryWriteReviewReportSummary {
                    point: point.into(),
                    phase_execution_id: phase_execution_id.into(),
                    status: match event_type {
                        HarnessEventType::MemoryWriteReviewCompleted => "completed",
                        HarnessEventType::MemoryWriteReviewSkipped => "skipped",
                        HarnessEventType::MemoryWriteReviewFailed => "failed",
                        _ => "unknown",
                    }
                    .into(),
                    reason: reason.into(),
                    model_calls: stats.model_calls,
                    memory_reads_attempted: stats.memory_reads_attempted,
                    memory_reads_completed: stats.memory_reads_completed,
                    memory_writes_attempted: stats.memory_writes_attempted,
                    memory_writes_completed: stats.memory_writes_completed,
                });
        }
        session.emitter.emit(
            event_type,
            HarnessEventPayload::Lifecycle {
                message: message.into(),
                fields: memory_write_review_fields(point, selected_outcome, reason, stats),
            },
            HarnessEventBuilder {
                run_id: Some(run_id.to_string()),
                phase_execution_id: Some(phase_execution_id.to_string()),
                ..HarnessEventBuilder::default()
            },
        )?;
        Ok(())
    }
}

fn persistence_review_effective_phase(effective_phase: &EffectivePhase) -> EffectivePhase {
    let mut review_phase = effective_phase.clone();
    review_phase.tools_allowed = Some(false);
    review_phase.knowledge_allowed = Some(false);
    review_phase.active_tools.clear();
    review_phase.active_skills.clear();
    review_phase.active_knowledge.clear();
    review_phase.capability_catalog = effective_phase
        .capability_catalog
        .iter()
        .filter(|descriptor| {
            descriptor.action_kind == "memory_read" || descriptor.action_kind == "memory_write"
        })
        .cloned()
        .collect();
    review_phase.capability_catalog.push(CapabilityDescriptor {
        action_kind: "persistence_review_complete".into(),
        identity: REVIEW_COMPLETE_IDENTITY.into(),
        description: "Complete the bounded Memory write review without changing the pending phase completion.".into(),
        source: "harness".into(),
    });
    review_phase
}

fn memory_write_review_fields(
    point: &str,
    selected_outcome: &str,
    reason: &str,
    stats: &MemoryWriteReviewStats,
) -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("point".into(), json!(point)),
        ("pending_outcome".into(), json!(selected_outcome)),
        ("reason".into(), json!(reason)),
        ("model_calls".into(), json!(stats.model_calls)),
        (
            "memory_reads_attempted".into(),
            json!(stats.memory_reads_attempted),
        ),
        (
            "memory_reads_completed".into(),
            json!(stats.memory_reads_completed),
        ),
        (
            "memory_writes_attempted".into(),
            json!(stats.memory_writes_attempted),
        ),
        (
            "memory_writes_completed".into(),
            json!(stats.memory_writes_completed),
        ),
    ])
}

fn memory_write_review_point_label(point: HarnessMemoryWriteReviewPoint) -> &'static str {
    match point {
        HarnessMemoryWriteReviewPoint::PhaseEnd => "phase_end",
        HarnessMemoryWriteReviewPoint::RunEnd => "run_end",
    }
}
