use super::*;

impl HarnessEngine {
    pub(super) fn phase_result(
        &self,
        session: &mut HarnessSession,
        phase: &LoopPhase,
        phase_execution_id: &str,
        outcome: String,
        output: Option<Value>,
    ) -> Result<PhaseResult> {
        let usage = self.active_run(session)?.usage.clone();
        let result = PhaseResult {
            phase_execution_id: phase_execution_id.to_string(),
            phase_id: phase.id.clone(),
            loop_step_number: self.active_run(session)?.step_count,
            outcome: outcome.clone(),
            output: output.clone(),
            usage,
            metadata: BTreeMap::new(),
        };
        self.active_run_mut(session)?
            .phase_summaries
            .push(PhaseReportSummary {
                phase_execution_id: phase_execution_id.to_string(),
                phase_id: phase.id.clone(),
                outcome: Some(outcome.clone()),
                transition_to: None,
                status: "completed".into(),
            });
        let run_id = self.active_run(session)?.run_id().to_string();
        session.emitter.emit(
            HarnessEventType::PhaseResultReady,
            HarnessEventPayload::Phase {
                phase_id: phase.id.clone(),
                outcome: Some(outcome),
                transition_to: None,
                output,
            },
            HarnessEventBuilder {
                run_id: Some(run_id),
                phase_execution_id: Some(phase_execution_id.to_string()),
                ..HarnessEventBuilder::default()
            },
        )?;
        Ok(result)
    }

    pub(super) fn fail_phase(
        &self,
        session: &mut HarnessSession,
        phase_id: &str,
        phase_execution_id: &str,
        message: String,
        terminal_status: Option<HarnessTerminalStatus>,
    ) -> Result<PhaseResult> {
        self.active_run_mut(session)?.error_count += 1;
        let terminal_status = terminal_status.unwrap_or_else(|| self.phase_failure_status());
        let run_id = self.active_run(session)?.run_id().to_string();
        session.emitter.emit(
            HarnessEventType::PhaseFailed,
            HarnessEventPayload::Lifecycle {
                message: message.clone(),
                fields: BTreeMap::new(),
            },
            HarnessEventBuilder {
                run_id: Some(run_id),
                phase_execution_id: Some(phase_execution_id.to_string()),
                ..HarnessEventBuilder::default()
            },
        )?;
        self.active_run_mut(session)?
            .phase_summaries
            .push(PhaseReportSummary {
                phase_execution_id: phase_execution_id.to_string(),
                phase_id: phase_id.to_string(),
                outcome: Some("failed".into()),
                transition_to: None,
                status: "failed".into(),
            });
        Ok(PhaseResult {
            phase_execution_id: phase_execution_id.to_string(),
            phase_id: phase_id.to_string(),
            loop_step_number: self.active_run(session)?.step_count,
            outcome: "failed".into(),
            output: Some(json!({ "error": message })),
            usage: self.active_run(session)?.usage.clone(),
            metadata: BTreeMap::from([
                ("phase_failed".into(), json!(true)),
                ("terminal_status".into(), json!(status_str(terminal_status))),
            ]),
        })
    }

    pub(super) fn phase_failure_status(&self) -> HarnessTerminalStatus {
        match self
            .loop_manifest
            .r#loop
            .error_policy
            .as_ref()
            .and_then(|policy| policy.phase_failure.as_ref())
            .map(|policy| &policy.action)
        {
            Some(LoopPhaseFailureAction::Abort) => HarnessTerminalStatus::Aborted,
            Some(LoopPhaseFailureAction::Handoff) => HarnessTerminalStatus::HandedOff,
            None => HarnessTerminalStatus::Failed,
        }
    }

    pub(super) fn limit_phase(
        &self,
        session: &mut HarnessSession,
        phase_execution_id: &str,
        reason: &str,
    ) -> Result<PhaseResult> {
        let run_id = self.active_run(session)?.run_id().to_string();
        let phase_id = self
            .active_run(session)?
            .current_phase_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        session.emitter.emit(
            HarnessEventType::RunLimitReached,
            HarnessEventPayload::Lifecycle {
                message: format!("Runtime limit `{reason}` was reached."),
                fields: BTreeMap::from([("limit".into(), json!(reason))]),
            },
            HarnessEventBuilder {
                run_id: Some(run_id),
                ..HarnessEventBuilder::default()
            },
        )?;
        self.active_run_mut(session)?.status = RuntimeTerminalStatus::LimitReached;
        Ok(PhaseResult {
            phase_execution_id: phase_execution_id.to_string(),
            phase_id,
            loop_step_number: self.active_run(session)?.step_count,
            outcome: "limit_reached".into(),
            output: Some(json!({ "reason": reason })),
            usage: self.active_run(session)?.usage.clone(),
            metadata: BTreeMap::from([
                ("limit_reached".into(), json!(true)),
                ("terminal_status".into(), json!("limit_reached")),
            ]),
        })
    }

    pub(super) fn finalize(
        &mut self,
        session: &mut HarnessSession,
        status: HarnessTerminalStatus,
        output: Option<Value>,
    ) -> Result<HarnessRunResult> {
        self.flush_memory_operation_controls(
            "memory_operation_run_ended",
            "external Memory operation was not serviced before the active Run ended",
        )?;
        let mut run = session
            .active_run
            .take()
            .ok_or_else(|| anyhow!("Harness Session has no active Run"))?;
        run.status = match status {
            HarnessTerminalStatus::Ended => RuntimeTerminalStatus::Ended,
            HarnessTerminalStatus::HandedOff => RuntimeTerminalStatus::HandedOff,
            HarnessTerminalStatus::Aborted => RuntimeTerminalStatus::Aborted,
            HarnessTerminalStatus::Failed => RuntimeTerminalStatus::Failed,
            HarnessTerminalStatus::Cancelled => RuntimeTerminalStatus::Cancelled,
            HarnessTerminalStatus::LimitReached => RuntimeTerminalStatus::LimitReached,
            HarnessTerminalStatus::ApprovalRequired => RuntimeTerminalStatus::ApprovalRequired,
        };
        let ended_at = Utc::now();
        let duration_ms = ended_at
            .signed_duration_since(run.started_at)
            .num_milliseconds()
            .max(0) as u64;
        run.ended_at = Some(ended_at);
        run.usage.duration_ms = Some(duration_ms);
        run.terminal_output = output.clone();
        session.usage.record_run_completed(&run.usage);
        let event_type = terminal_event_type(status);
        session.emitter.emit(
            event_type,
            HarnessEventPayload::Terminal {
                status,
                output: output.clone(),
            },
            HarnessEventBuilder {
                run_id: Some(run.run_id().to_string()),
                ..HarnessEventBuilder::default()
            },
        )?;
        session.emitter.emit(
            HarnessEventType::SessionUsageUpdated,
            HarnessEventPayload::Usage {
                run_usage: Box::new(run.usage.clone()),
                session_usage: Box::new(session.usage.clone()),
            },
            HarnessEventBuilder::default(),
        )?;
        let report = self.report_for_run(&session.session_id, &run, status, output.clone());
        Ok(HarnessRunResult::Terminal(Box::new(
            RuntimeTerminalResult {
                status,
                output,
                report,
            },
        )))
    }

    pub(super) fn report_for_run(
        &self,
        session_id: &str,
        run: &RunState,
        status: HarnessTerminalStatus,
        output: Option<Value>,
    ) -> RunReport {
        let mut approval_summary = BTreeMap::new();
        approval_summary.insert(
            "checkpoints".into(),
            run.checkpoint_summaries.len().try_into().unwrap_or(0),
        );
        let cancellation_summary = if status == HarnessTerminalStatus::Cancelled {
            BTreeMap::from([("cancelled".into(), 1)])
        } else {
            BTreeMap::new()
        };
        RunReport {
            report_version: HARNESS_REPORT_SCHEMA_VERSION,
            session_id: session_id.to_string(),
            run_id: run.run_id().to_string(),
            agent: ReportPackageIdentity {
                name: "synthetic-agent".into(),
                version: "0.0.0".into(),
            },
            loop_package: ReportPackageIdentity {
                name: self.loop_manifest.name.clone(),
                version: self.loop_manifest.version.clone(),
            },
            started_at: run.started_at,
            ended_at: run.ended_at,
            duration_ms: run.usage.duration_ms,
            terminal_status: status,
            terminal_output: output,
            preflight_status: PreflightStatus::Ready,
            diagnostics: Vec::<PreflightDiagnostic>::new(),
            runtime: Default::default(),
            runtime_sources: BTreeMap::new(),
            consumer_context: run
                .context
                .runtime
                .consumer_context
                .as_ref()
                .map(
                    |context| crate::harness_observability::ConsumerContextReportSummary {
                        status: if context.content.is_some() {
                            "loaded".into()
                        } else {
                            context.state.to_ascii_lowercase()
                        },
                        path: context.file.clone().or_else(|| {
                            context.path.as_ref().map(|path| path.display().to_string())
                        }),
                        byte_size: context.byte_size,
                        approximate_tokens: context.approximate_tokens,
                        sha256: context.sha256.clone(),
                        content_included: false,
                    },
                ),
            scope_summaries: Vec::new(),
            phase_summaries: run.phase_summaries.clone(),
            checkpoint_summaries: run.checkpoint_summaries.clone(),
            action_summaries: run.action_summaries.clone(),
            tool_summaries: run
                .operation_summaries
                .iter()
                .filter(|summary| summary.operation_kind != "memory_operation")
                .cloned()
                .collect(),
            mcp_summaries: Vec::new(),
            knowledge_summaries: operation_summaries_for_action_kind(
                &run.action_summaries,
                "knowledge_request",
            ),
            memory_summaries: {
                let mut summaries = memory_summaries_for_actions(&run.action_summaries);
                summaries.extend(
                    run.operation_summaries
                        .iter()
                        .filter(|summary| summary.operation_kind == "memory_operation")
                        .cloned(),
                );
                summaries
            },
            memory_write_review_summaries: run.memory_write_review_summaries.clone(),
            usage: run.usage.clone(),
            retry_count: run.retry_count,
            repair_count: run.repair_count,
            error_count: run.error_count,
            approval_summary,
            cancellation_summary,
            trace_path: None,
        }
    }

    pub(super) fn evaluate_checkpoints(
        &self,
        session: &mut HarnessSession,
        run_id: &str,
        phase_id: &str,
        approvals: &mut dyn ApprovalController,
        service_events: &mut Option<&mut ServiceLifecycleEvents>,
    ) -> Result<Option<CheckpointFlow>> {
        let checkpoints: Vec<_> = self
            .loop_manifest
            .r#loop
            .checkpoints
            .iter()
            .filter(|checkpoint| checkpoint.before_phase == phase_id)
            .cloned()
            .collect();
        for checkpoint in checkpoints {
            session.emitter.emit(
                HarnessEventType::ApprovalRequested,
                HarnessEventPayload::Lifecycle {
                    message: format!("Approval requested for checkpoint `{}`.", checkpoint.id),
                    fields: BTreeMap::from([(
                        "before_phase".into(),
                        json!(checkpoint.before_phase),
                    )]),
                },
                HarnessEventBuilder {
                    run_id: Some(run_id.to_string()),
                    ..HarnessEventBuilder::default()
                },
            )?;
            let decision = approvals.request_approval(&checkpoint);
            self.emit_service_lifecycle_events(session, Some(run_id), service_events)?;
            match decision {
                ApprovalDecision::Approve => {
                    self.active_run_mut(session)?.checkpoint_summaries.push(
                        CheckpointReportSummary {
                            checkpoint_id: checkpoint.id.clone(),
                            before_phase: checkpoint.before_phase.clone(),
                            status: "approved".into(),
                            on_reject: Some(checkpoint.on_reject.clone()),
                        },
                    );
                    session.emitter.emit(
                        HarnessEventType::ApprovalApproved,
                        HarnessEventPayload::Lifecycle {
                            message: format!("Approval `{}` approved.", checkpoint.id),
                            fields: BTreeMap::new(),
                        },
                        HarnessEventBuilder {
                            run_id: Some(run_id.to_string()),
                            ..HarnessEventBuilder::default()
                        },
                    )?;
                }
                ApprovalDecision::Deny => {
                    self.active_run_mut(session)?.checkpoint_summaries.push(
                        CheckpointReportSummary {
                            checkpoint_id: checkpoint.id.clone(),
                            before_phase: checkpoint.before_phase.clone(),
                            status: "denied".into(),
                            on_reject: Some(checkpoint.on_reject.clone()),
                        },
                    );
                    session.emitter.emit(
                        HarnessEventType::ApprovalDenied,
                        HarnessEventPayload::Lifecycle {
                            message: format!("Approval `{}` denied.", checkpoint.id),
                            fields: BTreeMap::new(),
                        },
                        HarnessEventBuilder {
                            run_id: Some(run_id.to_string()),
                            ..HarnessEventBuilder::default()
                        },
                    )?;
                    return Ok(Some(CheckpointFlow::ContinueTo(
                        checkpoint.on_reject.clone(),
                    )));
                }
                ApprovalDecision::Pending => {
                    let pending = PendingApprovalState {
                        checkpoint_id: checkpoint.id.clone(),
                        before_phase: checkpoint.before_phase.clone(),
                        on_reject: checkpoint.on_reject.clone(),
                    };
                    self.active_run_mut(session)?.checkpoint_summaries.push(
                        CheckpointReportSummary {
                            checkpoint_id: checkpoint.id,
                            before_phase: checkpoint.before_phase,
                            status: "pending".into(),
                            on_reject: Some(checkpoint.on_reject),
                        },
                    );
                    return Ok(Some(CheckpointFlow::Pending(pending)));
                }
                ApprovalDecision::Failure(message) => {
                    self.active_run_mut(session)?.checkpoint_summaries.push(
                        CheckpointReportSummary {
                            checkpoint_id: checkpoint.id.clone(),
                            before_phase: checkpoint.before_phase.clone(),
                            status: "failed".into(),
                            on_reject: Some(checkpoint.on_reject.clone()),
                        },
                    );
                    session.emitter.emit(
                        HarnessEventType::ApprovalFailed,
                        HarnessEventPayload::Lifecycle {
                            message: format!("Approval `{}` failed: {message}", checkpoint.id),
                            fields: BTreeMap::new(),
                        },
                        HarnessEventBuilder {
                            run_id: Some(run_id.to_string()),
                            ..HarnessEventBuilder::default()
                        },
                    )?;
                    return Err(anyhow!("approval `{}` failed: {message}", checkpoint.id));
                }
            }
        }
        Ok(None)
    }

    pub(super) fn transition_target(&self, phase_id: &str, outcome: &str) -> Result<String> {
        self.loop_manifest
            .r#loop
            .transitions
            .iter()
            .find(|transition| transition.from == phase_id && transition.on == outcome)
            .map(|transition| transition.to.clone())
            .ok_or_else(|| anyhow!("missing transition for phase `{phase_id}` outcome `{outcome}`"))
    }

    pub(super) fn terminal_for_target(&self, target: &str) -> Option<HarnessTerminalStatus> {
        match target {
            "$end" => Some(HarnessTerminalStatus::Ended),
            "$abort" => Some(HarnessTerminalStatus::Aborted),
            "$handoff" => Some(HarnessTerminalStatus::HandedOff),
            _ => None,
        }
    }

    pub(super) fn phase(&self, phase_id: &str) -> Option<&LoopPhase> {
        self.loop_manifest
            .r#loop
            .phases
            .iter()
            .find(|phase| phase.id == phase_id)
    }

    pub(super) fn effective_max_steps(&self) -> u64 {
        self.loop_manifest
            .r#loop
            .limits
            .as_ref()
            .and_then(|limits| limits.max_steps)
            .map(|authored| authored.min(self.options.runtime_limits.max_steps))
            .unwrap_or(self.options.runtime_limits.max_steps)
    }

    pub(super) fn merge_usage(&self, session: &mut HarnessSession, usage: &RunUsage) {
        let run = session
            .active_run
            .as_mut()
            .expect("HarnessEngine owns active RunState");
        run.usage.accepted_semantic_actions += usage.accepted_semantic_actions;
        run.usage.knowledge_requests += usage.knowledge_requests;
        run.usage.memory_requests += usage.memory_requests;
        run.usage.embedding_requests += usage.embedding_requests;
        run.usage.tokens.input_tokens =
            add_optional(run.usage.tokens.input_tokens, usage.tokens.input_tokens);
        run.usage.tokens.output_tokens =
            add_optional(run.usage.tokens.output_tokens, usage.tokens.output_tokens);
        run.usage.tokens.total_tokens =
            add_optional(run.usage.tokens.total_tokens, usage.tokens.total_tokens);
    }

    pub(super) fn active_run<'a>(&self, session: &'a HarnessSession) -> Result<&'a RunState> {
        session
            .active_run
            .as_ref()
            .ok_or_else(|| anyhow!("Harness Session has no active Run"))
    }

    pub(super) fn active_run_mut<'a>(
        &self,
        session: &'a mut HarnessSession,
    ) -> Result<&'a mut RunState> {
        session
            .active_run
            .as_mut()
            .ok_or_else(|| anyhow!("Harness Session has no active Run"))
    }
}
