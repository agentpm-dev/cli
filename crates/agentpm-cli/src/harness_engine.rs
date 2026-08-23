#![allow(dead_code)]

use crate::harness_config::HarnessRuntimeLimits;
use crate::harness_observability::{
    ActionReportSummary, CheckpointReportSummary, HARNESS_REPORT_SCHEMA_VERSION,
    HarnessEventBuilder, HarnessEventEmitter, HarnessEventPayload, HarnessEventType,
    HarnessTerminalStatus, OperationReportSummary, PhaseReportSummary, ReportPackageIdentity,
    RunReport, RunUsage, SessionUsage, allocate_harness_run_id, allocate_harness_session_id,
};
use crate::harness_plan::{PreflightDiagnostic, PreflightStatus};
use crate::harness_runtime::{
    ActionDispatchResult, ActionDispatcher, ApprovalController, ApprovalDecision, ModelRequest,
    ModelRuntime, SemanticAction, TranscriptEntry, TranscriptEntryKind,
};
use crate::manifest::{
    LoopManifest, LoopPhase, LoopPhaseFailureAction, LoopToolFailureAction,
    LoopToolFailureExhaustedAction,
};
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeTerminalStatus {
    Running,
    PendingApproval,
    Ended,
    HandedOff,
    Aborted,
    Failed,
    Cancelled,
    LimitReached,
    ApprovalRequired,
}

impl RuntimeTerminalStatus {
    fn harness_status(&self) -> Option<HarnessTerminalStatus> {
        match self {
            Self::Running | Self::PendingApproval => None,
            Self::Ended => Some(HarnessTerminalStatus::Ended),
            Self::HandedOff => Some(HarnessTerminalStatus::HandedOff),
            Self::Aborted => Some(HarnessTerminalStatus::Aborted),
            Self::Failed => Some(HarnessTerminalStatus::Failed),
            Self::Cancelled => Some(HarnessTerminalStatus::Cancelled),
            Self::LimitReached => Some(HarnessTerminalStatus::LimitReached),
            Self::ApprovalRequired => Some(HarnessTerminalStatus::ApprovalRequired),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunContext {
    pub run_id: String,
    pub input: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectivePhase {
    pub phase_id: String,
    pub tools_allowed: Option<bool>,
    pub knowledge_allowed: Option<bool>,
    pub memory_read_allowed: Option<bool>,
    pub memory_write_allowed: Option<bool>,
}

impl EffectivePhase {
    fn from_phase(phase: &LoopPhase) -> Self {
        let access = phase.access.as_ref();
        Self {
            phase_id: phase.id.clone(),
            tools_allowed: access.and_then(|access| access.tools),
            knowledge_allowed: access.and_then(|access| access.knowledge),
            memory_read_allowed: access
                .and_then(|access| access.memory.as_ref())
                .and_then(|memory| memory.read),
            memory_write_allowed: access
                .and_then(|access| access.memory.as_ref())
                .and_then(|memory| memory.write),
        }
    }

    fn permits(&self, action: &SemanticAction) -> bool {
        match action {
            SemanticAction::AgentPmTool { .. } | SemanticAction::ExternalMcpTool { .. } => {
                self.tools_allowed != Some(false)
            }
            SemanticAction::KnowledgeRequest { .. } => self.knowledge_allowed != Some(false),
            SemanticAction::MemoryRead { .. } => self.memory_read_allowed != Some(false),
            SemanticAction::MemoryWrite { .. } => self.memory_write_allowed != Some(false),
            SemanticAction::SkillResourceRead { .. } | SemanticAction::PhaseCompletion { .. } => {
                true
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhaseExecutionState {
    pub phase_execution_id: String,
    pub phase_id: String,
    pub transcript: Vec<TranscriptEntry>,
    pub model_calls: u64,
    pub accepted_actions: u64,
    pub logical_tool_calls: u64,
    pub structured_repairs: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhaseResult {
    pub phase_execution_id: String,
    pub phase_id: String,
    pub loop_step_number: u64,
    pub outcome: String,
    pub output: Option<Value>,
    pub usage: RunUsage,
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingApprovalState {
    pub checkpoint_id: String,
    pub before_phase: String,
    pub on_reject: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunState {
    context: RunContext,
    status: RuntimeTerminalStatus,
    current_phase_id: Option<String>,
    step_count: u64,
    phase_results: Vec<PhaseResult>,
    phase_summaries: Vec<PhaseReportSummary>,
    action_summaries: Vec<ActionReportSummary>,
    checkpoint_summaries: Vec<CheckpointReportSummary>,
    operation_summaries: Vec<OperationReportSummary>,
    pending_approval: Option<PendingApprovalState>,
    usage: RunUsage,
    started_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
    terminal_output: Option<Value>,
    retry_count: u64,
    repair_count: u64,
    error_count: u64,
}

impl RunState {
    pub fn run_id(&self) -> &str {
        &self.context.run_id
    }

    pub fn status(&self) -> &RuntimeTerminalStatus {
        &self.status
    }

    pub fn pending_approval(&self) -> Option<&PendingApprovalState> {
        self.pending_approval.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeTerminalResult {
    pub status: HarnessTerminalStatus,
    pub output: Option<Value>,
    pub report: RunReport,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HarnessRunResult {
    Terminal(Box<RuntimeTerminalResult>),
    PendingApproval {
        run_id: String,
        checkpoint: PendingApprovalState,
    },
}

pub struct HarnessSession {
    pub session_id: String,
    pub emitter: HarnessEventEmitter,
    pub usage: SessionUsage,
    active_run: Option<RunState>,
}

impl HarnessSession {
    pub fn new() -> Self {
        let session_id = allocate_harness_session_id();
        Self {
            emitter: HarnessEventEmitter::new(session_id.clone()),
            session_id,
            usage: SessionUsage::default(),
            active_run: None,
        }
    }

    pub fn active_run(&self) -> Option<&RunState> {
        self.active_run.as_ref()
    }

    pub fn clear_terminal_active_run(&mut self) {
        if self
            .active_run
            .as_ref()
            .is_some_and(|run| run.status.harness_status().is_some())
        {
            self.active_run = None;
        }
    }

    fn start_run(&mut self, input: String) -> Result<String> {
        if let Some(run) = &self.active_run
            && matches!(
                run.status,
                RuntimeTerminalStatus::Running | RuntimeTerminalStatus::PendingApproval
            )
        {
            return Err(anyhow!(
                "Harness Session already has an active Run `{}`",
                run.run_id()
            ));
        }
        self.active_run = None;
        let run_id = allocate_harness_run_id();
        self.usage.record_run_started();
        self.emitter.emit(
            HarnessEventType::RunStarted,
            HarnessEventPayload::Lifecycle {
                message: "Harness run started.".into(),
                fields: BTreeMap::from([("input".into(), json!(input))]),
            },
            HarnessEventBuilder {
                run_id: Some(run_id.clone()),
                ..HarnessEventBuilder::default()
            },
        )?;
        self.active_run = Some(RunState {
            context: RunContext {
                run_id: run_id.clone(),
                input,
            },
            status: RuntimeTerminalStatus::Running,
            current_phase_id: None,
            step_count: 0,
            phase_results: Vec::new(),
            phase_summaries: Vec::new(),
            action_summaries: Vec::new(),
            checkpoint_summaries: Vec::new(),
            operation_summaries: Vec::new(),
            pending_approval: None,
            usage: RunUsage::default(),
            started_at: Utc::now(),
            ended_at: None,
            terminal_output: None,
            retry_count: 0,
            repair_count: 0,
            error_count: 0,
        });
        Ok(run_id)
    }
}

impl Default for HarnessSession {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct HarnessEngineOptions {
    pub runtime_limits: HarnessRuntimeLimits,
    pub retain_active_on_approval_required: bool,
}

impl HarnessEngineOptions {
    pub fn new(runtime_limits: HarnessRuntimeLimits) -> Self {
        Self {
            runtime_limits,
            retain_active_on_approval_required: false,
        }
    }
}

pub struct HarnessEngine {
    loop_manifest: LoopManifest,
    options: HarnessEngineOptions,
    phase_executions: u64,
}

impl HarnessEngine {
    pub fn new(loop_manifest: LoopManifest, options: HarnessEngineOptions) -> Self {
        Self {
            loop_manifest,
            options,
            phase_executions: 0,
        }
    }

    pub fn execute_run(
        &mut self,
        session: &mut HarnessSession,
        input: impl Into<String>,
        model: &mut dyn ModelRuntime,
        dispatcher: &mut dyn ActionDispatcher,
        approvals: &mut dyn ApprovalController,
    ) -> Result<HarnessRunResult> {
        let run_id = session.start_run(input.into())?;
        let mut current_phase = self.loop_manifest.r#loop.entry_phase.clone();
        loop {
            // Checkpoints are evaluated before entering the target phase. A
            // rejection can route directly to another phase or terminal target,
            // while an unresolved approval either pauses or terminalizes the Run
            // depending on the execution surface.
            if let Some(result) =
                self.evaluate_checkpoints(session, &run_id, &current_phase, approvals)?
            {
                match result {
                    CheckpointFlow::ContinueTo(target) => {
                        if let Some(terminal) = self.terminal_for_target(&target) {
                            return self.finalize(
                                session,
                                terminal,
                                Some(json!({ "target": target })),
                            );
                        }
                        current_phase = target;
                        continue;
                    }
                    CheckpointFlow::Pending(pending) => {
                        if let Some(run) = session.active_run.as_mut() {
                            run.status = RuntimeTerminalStatus::PendingApproval;
                            run.pending_approval = Some(pending.clone());
                        }
                        if self.options.retain_active_on_approval_required {
                            return Ok(HarnessRunResult::PendingApproval {
                                run_id,
                                checkpoint: pending,
                            });
                        }
                        return self.finalize(
                            session,
                            HarnessTerminalStatus::ApprovalRequired,
                            Some(json!({ "checkpoint": pending.checkpoint_id })),
                        );
                    }
                }
            }

            // Loop max_steps is checked before starting a new phase execution so
            // a phase is never partially entered after the authored/runtime step
            // budget is exhausted.
            let effective_max_steps = self.effective_max_steps();
            if self.active_run(session)?.step_count >= effective_max_steps {
                session.emitter.emit(
                    HarnessEventType::LoopLimitReached,
                    HarnessEventPayload::Lifecycle {
                        message: "Loop max_steps was reached.".into(),
                        fields: BTreeMap::from([("max_steps".into(), json!(effective_max_steps))]),
                    },
                    HarnessEventBuilder {
                        run_id: Some(run_id.clone()),
                        ..HarnessEventBuilder::default()
                    },
                )?;
                return self.finalize(
                    session,
                    HarnessTerminalStatus::LimitReached,
                    Some(json!({ "reason": "max_steps" })),
                );
            }

            // Execute exactly one phase entry. The phase executor owns
            // phase-local model turns and action dispatch; this run loop only
            // persists the PhaseResult and chooses the next transition.
            let phase = self
                .phase(&current_phase)
                .ok_or_else(|| anyhow!("Loop phase `{current_phase}` is not declared"))?
                .clone();
            let phase_result = match self.execute_phase(session, &phase, model, dispatcher) {
                Ok(result) => result,
                Err(err) => {
                    return self.finalize(
                        session,
                        HarnessTerminalStatus::Failed,
                        Some(json!({ "error": err.to_string() })),
                    );
                }
            };
            let outcome = phase_result.outcome.clone();
            // Some phase-local failures already resolve to runtime terminal
            // states via authored error policy or hard limits. Keep those
            // terminalizations authoritative rather than looking for a Loop
            // transition from the synthetic failure outcome.
            if let Some(terminal_status) = phase_result
                .metadata
                .get("terminal_status")
                .and_then(Value::as_str)
                .and_then(harness_status_from_str)
            {
                let output = phase_result.output.clone();
                self.active_run_mut(session)?
                    .phase_results
                    .push(phase_result);
                return self.finalize(session, terminal_status, output);
            }
            self.active_run_mut(session)?
                .phase_results
                .push(phase_result);
            // Normal phase completion uses the Loop transition table. Terminal
            // targets finalize the Run; non-terminal targets become the next
            // phase in the same Session-owned active RunState.
            let target = self.transition_target(&phase.id, &outcome)?;
            session.emitter.emit(
                HarnessEventType::TransitionSelected,
                HarnessEventPayload::Phase {
                    phase_id: phase.id.clone(),
                    outcome: Some(outcome),
                    transition_to: Some(target.clone()),
                },
                HarnessEventBuilder {
                    run_id: Some(run_id.clone()),
                    ..HarnessEventBuilder::default()
                },
            )?;
            if let Some(terminal) = self.terminal_for_target(&target) {
                let output = self
                    .active_run(session)?
                    .phase_results
                    .last()
                    .and_then(|result| result.output.clone());
                return self.finalize(session, terminal, output);
            }
            current_phase = target;
        }
    }

    pub fn cancel_active_run(
        &mut self,
        session: &mut HarnessSession,
        reason: impl Into<String>,
    ) -> Result<RuntimeTerminalResult> {
        let run_id = self.active_run(session)?.run_id().to_string();
        let reason = reason.into();
        session.emitter.emit(
            HarnessEventType::CancellationRequested,
            HarnessEventPayload::Lifecycle {
                message: reason.clone(),
                fields: BTreeMap::new(),
            },
            HarnessEventBuilder {
                run_id: Some(run_id.clone()),
                ..HarnessEventBuilder::default()
            },
        )?;
        let result = self.finalize(
            session,
            HarnessTerminalStatus::Cancelled,
            Some(json!({ "reason": reason })),
        )?;
        match result {
            HarnessRunResult::Terminal(result) => {
                session.emitter.emit(
                    HarnessEventType::CancellationCompleted,
                    HarnessEventPayload::Lifecycle {
                        message: "Cancellation completed.".into(),
                        fields: BTreeMap::new(),
                    },
                    HarnessEventBuilder {
                        run_id: Some(run_id),
                        ..HarnessEventBuilder::default()
                    },
                )?;
                Ok(*result)
            }
            HarnessRunResult::PendingApproval { .. } => {
                Err(anyhow!("cancel unexpectedly returned pending approval"))
            }
        }
    }

    fn execute_phase(
        &mut self,
        session: &mut HarnessSession,
        phase: &LoopPhase,
        model: &mut dyn ModelRuntime,
        dispatcher: &mut dyn ActionDispatcher,
    ) -> Result<PhaseResult> {
        // A phase execution is one entry into a Loop phase. Re-entering the same
        // phase later creates a new phase_execution_id and fresh phase-local
        // transcript/counters.
        self.phase_executions += 1;
        let phase_execution_id = format!("phase-exec-{}", self.phase_executions);
        let run_id = self.active_run(session)?.run_id().to_string();
        let effective_phase = EffectivePhase::from_phase(phase);
        {
            let run = self.active_run_mut(session)?;
            run.current_phase_id = Some(phase.id.clone());
            run.step_count += 1;
        }
        session.emitter.emit(
            HarnessEventType::PhaseEnterRequested,
            HarnessEventPayload::Phase {
                phase_id: phase.id.clone(),
                outcome: None,
                transition_to: None,
            },
            HarnessEventBuilder {
                run_id: Some(run_id.clone()),
                phase_execution_id: Some(phase_execution_id.clone()),
                ..HarnessEventBuilder::default()
            },
        )?;
        session.emitter.emit(
            HarnessEventType::EffectivePhaseComputed,
            HarnessEventPayload::Lifecycle {
                message: "Effective phase computed.".into(),
                fields: BTreeMap::from([("phase_id".into(), json!(phase.id))]),
            },
            HarnessEventBuilder {
                run_id: Some(run_id.clone()),
                phase_execution_id: Some(phase_execution_id.clone()),
                ..HarnessEventBuilder::default()
            },
        )?;
        session.emitter.emit(
            HarnessEventType::PhaseStarted,
            HarnessEventPayload::Phase {
                phase_id: phase.id.clone(),
                outcome: None,
                transition_to: None,
            },
            HarnessEventBuilder {
                run_id: Some(run_id.clone()),
                phase_execution_id: Some(phase_execution_id.clone()),
                ..HarnessEventBuilder::default()
            },
        )?;

        let mut state = PhaseExecutionState {
            phase_execution_id: phase_execution_id.clone(),
            phase_id: phase.id.clone(),
            transcript: vec![TranscriptEntry {
                kind: TranscriptEntryKind::UserInput,
                content: json!(self.active_run(session)?.context.input),
            }],
            model_calls: 0,
            accepted_actions: 0,
            logical_tool_calls: 0,
            structured_repairs: 0,
        };
        // Explicit outcomes must be selected by the model. Phases with no
        // declared outcomes use the implicit `complete` outcome.
        let explicit_outcomes: Vec<String> = phase
            .outcomes
            .iter()
            .map(|outcome| outcome.id.clone())
            .collect();
        let mut repair_feedback = None;

        loop {
            // Each model turn sees the immutable run input, prior completed
            // phases, this phase's transcript, the current EffectivePhase, and
            // optional repair feedback from the previous malformed proposal.
            if state.model_calls >= self.options.runtime_limits.max_model_calls_per_phase {
                return self.limit_phase(session, &phase_execution_id, "max_model_calls_per_phase");
            }
            let request = ModelRequest {
                run_id: run_id.clone(),
                phase_execution_id: phase_execution_id.clone(),
                phase_id: phase.id.clone(),
                phase_objective: phase.objective.clone(),
                run_input: self.active_run(session)?.context.input.clone(),
                prior_phase_results: self.active_run(session)?.phase_results.clone(),
                transcript: state.transcript.clone(),
                effective_phase: effective_phase.clone(),
                repair_feedback: repair_feedback.clone(),
            };
            session.emitter.emit(
                HarnessEventType::ModelRequestStarted,
                HarnessEventPayload::Lifecycle {
                    message: "Model request started.".into(),
                    fields: BTreeMap::from([("phase_id".into(), json!(phase.id))]),
                },
                HarnessEventBuilder {
                    run_id: Some(run_id.clone()),
                    phase_execution_id: Some(phase_execution_id.clone()),
                    ..HarnessEventBuilder::default()
                },
            )?;
            state.model_calls += 1;
            self.active_run_mut(session)?.usage.model_calls += 1;
            let turn = match model.generate(request) {
                Ok(turn) => turn,
                Err(err) => {
                    self.active_run_mut(session)?.error_count += 1;
                    session.emitter.emit(
                        HarnessEventType::ModelRequestFailed,
                        HarnessEventPayload::Lifecycle {
                            message: err.message.clone(),
                            fields: BTreeMap::new(),
                        },
                        HarnessEventBuilder {
                            run_id: Some(run_id.clone()),
                            phase_execution_id: Some(phase_execution_id.clone()),
                            ..HarnessEventBuilder::default()
                        },
                    )?;
                    return self.fail_phase(
                        session,
                        &phase.id,
                        &phase_execution_id,
                        err.message,
                        None,
                    );
                }
            };
            self.merge_usage(session, &turn.usage);
            session.emitter.emit(
                HarnessEventType::ModelRequestCompleted,
                HarnessEventPayload::Lifecycle {
                    message: "Model request completed.".into(),
                    fields: BTreeMap::from([(
                        "semantic_actions".into(),
                        json!(turn.actions.len()),
                    )]),
                },
                HarnessEventBuilder {
                    run_id: Some(run_id.clone()),
                    phase_execution_id: Some(phase_execution_id.clone()),
                    ..HarnessEventBuilder::default()
                },
            )?;
            if let Some(content) = &turn.assistant_content {
                state.transcript.push(TranscriptEntry {
                    kind: TranscriptEntryKind::Assistant,
                    content: json!(content),
                });
            }

            // Completion proposals are control-flow decisions. They cannot be
            // mixed with executable semantic actions in the same model turn
            // because the Engine must know whether to keep acting or leave the
            // phase.
            let completion_count = turn
                .actions
                .iter()
                .filter(|action| action.action.is_completion())
                .count();
            if completion_count > 0 && completion_count != turn.actions.len() {
                repair_feedback = Some(
                    "A phase completion proposal cannot be combined with executable actions."
                        .to_string(),
                );
                self.request_repair(
                    session,
                    &mut state,
                    &phase_execution_id,
                    repair_feedback.clone(),
                )?;
                continue;
            }

            // A no-action response can complete only an implicit-complete
            // phase. Explicit-outcome phases require a structured completion so
            // transition choice is observable and repairable.
            if turn.actions.is_empty() {
                if explicit_outcomes.is_empty() {
                    let output = turn.assistant_content.map(Value::String);
                    return self.phase_result(
                        session,
                        phase,
                        &phase_execution_id,
                        "complete".to_string(),
                        output,
                    );
                }
                repair_feedback =
                    Some("This phase requires an explicit completion outcome.".to_string());
                self.request_repair(
                    session,
                    &mut state,
                    &phase_execution_id,
                    repair_feedback.clone(),
                )?;
                continue;
            }

            // A pure completion turn selects the phase outcome after validating
            // authored outcome constraints and action limits.
            if completion_count == turn.actions.len() {
                let proposal = turn.actions.first().expect("completion action exists");
                let SemanticAction::PhaseCompletion { outcome, output } = &proposal.action else {
                    unreachable!("completion_count ensures phase completion");
                };
                if state.accepted_actions >= self.options.runtime_limits.max_actions_per_phase {
                    return self.limit_phase(session, &phase_execution_id, "max_actions_per_phase");
                }
                state.accepted_actions += 1;
                self.active_run_mut(session)?
                    .usage
                    .accepted_semantic_actions += 1;
                let selected = outcome.clone().unwrap_or_else(|| "complete".to_string());
                if !explicit_outcomes.is_empty() && !explicit_outcomes.contains(&selected) {
                    session.emitter.emit(
                        HarnessEventType::OutcomeInvalid,
                        HarnessEventPayload::Phase {
                            phase_id: phase.id.clone(),
                            outcome: Some(selected.clone()),
                            transition_to: None,
                        },
                        HarnessEventBuilder {
                            run_id: Some(run_id.clone()),
                            phase_execution_id: Some(phase_execution_id.clone()),
                            ..HarnessEventBuilder::default()
                        },
                    )?;
                    repair_feedback = Some(format!(
                        "Outcome `{selected}` is not declared for phase `{}`.",
                        phase.id
                    ));
                    self.request_repair(
                        session,
                        &mut state,
                        &phase_execution_id,
                        repair_feedback.clone(),
                    )?;
                    continue;
                }
                if explicit_outcomes.is_empty() && selected != "complete" {
                    repair_feedback = Some(format!(
                        "Phase `{}` has implicit outcome `complete`; `{selected}` is invalid.",
                        phase.id
                    ));
                    self.request_repair(
                        session,
                        &mut state,
                        &phase_execution_id,
                        repair_feedback.clone(),
                    )?;
                    continue;
                }
                session.emitter.emit(
                    HarnessEventType::OutcomeSelected,
                    HarnessEventPayload::Phase {
                        phase_id: phase.id.clone(),
                        outcome: Some(selected.clone()),
                        transition_to: None,
                    },
                    HarnessEventBuilder {
                        run_id: Some(run_id),
                        phase_execution_id: Some(phase_execution_id.clone()),
                        ..HarnessEventBuilder::default()
                    },
                )?;
                return self.phase_result(
                    session,
                    phase,
                    &phase_execution_id,
                    selected,
                    output.clone(),
                );
            }

            // Executable semantic actions are processed in authored/model order.
            // The Engine gates them against EffectivePhase, enforces limits,
            // dispatches through the runtime boundary, and appends structured
            // results back into the phase-local transcript for the next turn.
            for proposal in turn.actions {
                if !effective_phase.permits(&proposal.action) {
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
                            phase_execution_id: Some(phase_execution_id.clone()),
                            ..HarnessEventBuilder::default()
                        },
                    )?;
                    repair_feedback = Some("Action is not permitted by Loop access.".to_string());
                    self.request_repair(
                        session,
                        &mut state,
                        &phase_execution_id,
                        repair_feedback.clone(),
                    )?;
                    continue;
                }
                if state.accepted_actions >= self.options.runtime_limits.max_actions_per_phase {
                    return self.limit_phase(session, &phase_execution_id, "max_actions_per_phase");
                }
                if proposal.action.is_tool_call() {
                    if state.logical_tool_calls
                        >= self.options.runtime_limits.max_tool_calls_per_phase
                    {
                        return self.limit_phase(
                            session,
                            &phase_execution_id,
                            "max_tool_calls_per_phase",
                        );
                    }
                    state.logical_tool_calls += 1;
                    self.active_run_mut(session)?.usage.tool_calls += 1;
                }
                state.accepted_actions += 1;
                self.active_run_mut(session)?
                    .usage
                    .accepted_semantic_actions += 1;
                session.emitter.emit(
                    HarnessEventType::SemanticActionProposed,
                    HarnessEventPayload::Action {
                        action_kind: proposal.action.kind().into(),
                        identity: proposal.action.identity(),
                        status: "accepted".into(),
                        fields: BTreeMap::new(),
                    },
                    HarnessEventBuilder {
                        run_id: Some(run_id.clone()),
                        phase_execution_id: Some(phase_execution_id.clone()),
                        ..HarnessEventBuilder::default()
                    },
                )?;
                let result = self.dispatch_with_retry(session, dispatcher, &proposal.action)?;
                if !result.ok {
                    let error = result.error.unwrap_or_else(|| "action failed".to_string());
                    let terminal_status = result.terminal_status;
                    self.active_run_mut(session)?
                        .action_summaries
                        .push(ActionReportSummary {
                            action_kind: proposal.action.kind().into(),
                            identity: proposal.action.identity(),
                            status: "failed".into(),
                            error: Some(error.clone()),
                        });
                    if proposal.action.is_tool_call() {
                        self.active_run_mut(session)?.operation_summaries.push(
                            OperationReportSummary {
                                operation_kind: proposal.action.kind().into(),
                                identity: proposal.action.identity(),
                                status: "failed".into(),
                                count: 1,
                            },
                        );
                    }
                    return self.fail_phase(
                        session,
                        &phase.id,
                        &phase_execution_id,
                        error,
                        terminal_status,
                    );
                }
                state.transcript.push(TranscriptEntry {
                    kind: TranscriptEntryKind::ActionResult,
                    content: result.output.clone(),
                });
                self.active_run_mut(session)?
                    .action_summaries
                    .push(ActionReportSummary {
                        action_kind: proposal.action.kind().into(),
                        identity: proposal.action.identity(),
                        status: "completed".into(),
                        error: None,
                    });
                if proposal.action.is_tool_call() {
                    self.active_run_mut(session)?.operation_summaries.push(
                        OperationReportSummary {
                            operation_kind: proposal.action.kind().into(),
                            identity: proposal.action.identity(),
                            status: "completed".into(),
                            count: 1,
                        },
                    );
                }
            }
            repair_feedback = None;
        }
    }

    fn request_repair(
        &self,
        session: &mut HarnessSession,
        state: &mut PhaseExecutionState,
        phase_execution_id: &str,
        feedback: Option<String>,
    ) -> Result<()> {
        if state.structured_repairs >= self.options.runtime_limits.max_structured_output_repairs {
            return Err(anyhow!("structured output repair limit exhausted"));
        }
        state.structured_repairs += 1;
        self.active_run_mut(session)?.repair_count += 1;
        let message = feedback.unwrap_or_else(|| "Repair requested.".to_string());
        state.transcript.push(TranscriptEntry {
            kind: TranscriptEntryKind::RepairFeedback,
            content: json!(message),
        });
        let run_id = self.active_run(session)?.run_id().to_string();
        session.emitter.emit(
            HarnessEventType::ModelRepairRequested,
            HarnessEventPayload::Lifecycle {
                message,
                fields: BTreeMap::from([(
                    "repair_attempt".into(),
                    json!(state.structured_repairs),
                )]),
            },
            HarnessEventBuilder {
                run_id: Some(run_id),
                phase_execution_id: Some(phase_execution_id.to_string()),
                ..HarnessEventBuilder::default()
            },
        )?;
        Ok(())
    }

    fn dispatch_with_retry(
        &self,
        session: &mut HarnessSession,
        dispatcher: &mut dyn ActionDispatcher,
        action: &SemanticAction,
    ) -> Result<ActionDispatchResult> {
        let mut attempts = 0;
        loop {
            attempts += 1;
            let result = dispatcher.dispatch(action);
            let event_type = action_dispatch_event_type(action, result.ok);
            let run_id = self.active_run(session)?.run_id().to_string();
            session.emitter.emit(
                event_type,
                HarnessEventPayload::Action {
                    action_kind: action.kind().into(),
                    identity: action.identity(),
                    status: if result.ok { "completed" } else { "failed" }.into(),
                    fields: BTreeMap::from([("attempt".into(), json!(attempts))]),
                },
                HarnessEventBuilder {
                    run_id: Some(run_id.clone()),
                    ..HarnessEventBuilder::default()
                },
            )?;
            if result.ok || !action.is_tool_call() {
                return Ok(result);
            }
            let Some(policy) = self
                .loop_manifest
                .r#loop
                .error_policy
                .as_ref()
                .and_then(|policy| policy.tool_failure.as_ref())
            else {
                return Ok(result);
            };
            if policy.action != LoopToolFailureAction::Retry {
                return match policy.action {
                    LoopToolFailureAction::FailPhase => Ok(result),
                    LoopToolFailureAction::Abort => Ok(ActionDispatchResult::terminal_failure(
                        HarnessTerminalStatus::Aborted,
                        "Tool failure policy requested abort",
                    )),
                    LoopToolFailureAction::Handoff => Ok(ActionDispatchResult::terminal_failure(
                        HarnessTerminalStatus::HandedOff,
                        "Tool failure policy requested handoff",
                    )),
                    LoopToolFailureAction::Retry => unreachable!(),
                };
            }
            let max_retries = policy.max_retries.unwrap_or(0);
            if attempts > max_retries {
                let exhausted = policy
                    .on_exhausted
                    .as_ref()
                    .unwrap_or(&LoopToolFailureExhaustedAction::FailPhase);
                return match exhausted {
                    LoopToolFailureExhaustedAction::FailPhase => Ok(result),
                    LoopToolFailureExhaustedAction::Abort => {
                        Ok(ActionDispatchResult::terminal_failure(
                            HarnessTerminalStatus::Aborted,
                            "Tool retry exhaustion policy requested abort",
                        ))
                    }
                    LoopToolFailureExhaustedAction::Handoff => {
                        Ok(ActionDispatchResult::terminal_failure(
                            HarnessTerminalStatus::HandedOff,
                            "Tool retry exhaustion policy requested handoff",
                        ))
                    }
                };
            }
            self.active_run_mut(session)?.retry_count += 1;
            self.active_run_mut(session)?.usage.tool_retries += 1;
            session.emitter.emit(
                HarnessEventType::ToolRetrying,
                HarnessEventPayload::Action {
                    action_kind: action.kind().into(),
                    identity: action.identity(),
                    status: "retrying".into(),
                    fields: BTreeMap::from([("attempt".into(), json!(attempts + 1))]),
                },
                HarnessEventBuilder {
                    run_id: Some(run_id),
                    ..HarnessEventBuilder::default()
                },
            )?;
        }
    }

    fn phase_result(
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
            },
            HarnessEventBuilder {
                run_id: Some(run_id),
                phase_execution_id: Some(phase_execution_id.to_string()),
                ..HarnessEventBuilder::default()
            },
        )?;
        Ok(result)
    }

    fn fail_phase(
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

    fn phase_failure_status(&self) -> HarnessTerminalStatus {
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

    fn limit_phase(
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

    fn finalize(
        &mut self,
        session: &mut HarnessSession,
        status: HarnessTerminalStatus,
        output: Option<Value>,
    ) -> Result<HarnessRunResult> {
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

    fn report_for_run(
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
            consumer_context: None,
            scope_summaries: Vec::new(),
            phase_summaries: run.phase_summaries.clone(),
            checkpoint_summaries: run.checkpoint_summaries.clone(),
            action_summaries: run.action_summaries.clone(),
            tool_summaries: run.operation_summaries.clone(),
            mcp_summaries: Vec::new(),
            knowledge_summaries: Vec::new(),
            memory_summaries: Vec::new(),
            usage: run.usage.clone(),
            retry_count: run.retry_count,
            repair_count: run.repair_count,
            error_count: run.error_count,
            approval_summary,
            cancellation_summary,
            trace_path: None,
        }
    }

    fn evaluate_checkpoints(
        &self,
        session: &mut HarnessSession,
        run_id: &str,
        phase_id: &str,
        approvals: &mut dyn ApprovalController,
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
            match approvals.request_approval(&checkpoint) {
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
            }
        }
        Ok(None)
    }

    fn transition_target(&self, phase_id: &str, outcome: &str) -> Result<String> {
        self.loop_manifest
            .r#loop
            .transitions
            .iter()
            .find(|transition| transition.from == phase_id && transition.on == outcome)
            .map(|transition| transition.to.clone())
            .ok_or_else(|| anyhow!("missing transition for phase `{phase_id}` outcome `{outcome}`"))
    }

    fn terminal_for_target(&self, target: &str) -> Option<HarnessTerminalStatus> {
        match target {
            "$end" => Some(HarnessTerminalStatus::Ended),
            "$abort" => Some(HarnessTerminalStatus::Aborted),
            "$handoff" => Some(HarnessTerminalStatus::HandedOff),
            _ => None,
        }
    }

    fn phase(&self, phase_id: &str) -> Option<&LoopPhase> {
        self.loop_manifest
            .r#loop
            .phases
            .iter()
            .find(|phase| phase.id == phase_id)
    }

    fn effective_max_steps(&self) -> u64 {
        self.loop_manifest
            .r#loop
            .limits
            .as_ref()
            .and_then(|limits| limits.max_steps)
            .map(|authored| authored.min(self.options.runtime_limits.max_steps))
            .unwrap_or(self.options.runtime_limits.max_steps)
    }

    fn merge_usage(&self, session: &mut HarnessSession, usage: &RunUsage) {
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

    fn active_run<'a>(&self, session: &'a HarnessSession) -> Result<&'a RunState> {
        session
            .active_run
            .as_ref()
            .ok_or_else(|| anyhow!("Harness Session has no active Run"))
    }

    fn active_run_mut<'a>(&self, session: &'a mut HarnessSession) -> Result<&'a mut RunState> {
        session
            .active_run
            .as_mut()
            .ok_or_else(|| anyhow!("Harness Session has no active Run"))
    }
}

enum CheckpointFlow {
    ContinueTo(String),
    Pending(PendingApprovalState),
}

fn action_dispatch_event_type(action: &SemanticAction, ok: bool) -> HarnessEventType {
    match action {
        SemanticAction::AgentPmTool { .. } => {
            if ok {
                HarnessEventType::ToolCompleted
            } else {
                HarnessEventType::ToolFailed
            }
        }
        SemanticAction::ExternalMcpTool { .. } => {
            if ok {
                HarnessEventType::McpToolCompleted
            } else {
                HarnessEventType::McpToolFailed
            }
        }
        SemanticAction::SkillResourceRead { .. } => {
            if ok {
                HarnessEventType::SkillResourceLoaded
            } else {
                HarnessEventType::SkillResourceFailed
            }
        }
        SemanticAction::KnowledgeRequest { .. } => {
            if ok {
                HarnessEventType::KnowledgeRetrieved
            } else {
                HarnessEventType::KnowledgeFailed
            }
        }
        SemanticAction::MemoryRead { .. } => {
            if ok {
                HarnessEventType::MemoryReadCompleted
            } else {
                HarnessEventType::MemoryReadFailed
            }
        }
        SemanticAction::MemoryWrite { .. } => {
            if ok {
                HarnessEventType::MemoryWriteCompleted
            } else {
                HarnessEventType::MemoryWriteFailed
            }
        }
        SemanticAction::PhaseCompletion { .. } => HarnessEventType::SemanticActionCompleted,
    }
}

fn terminal_event_type(status: HarnessTerminalStatus) -> HarnessEventType {
    match status {
        HarnessTerminalStatus::Ended | HarnessTerminalStatus::HandedOff => {
            HarnessEventType::RunCompleted
        }
        HarnessTerminalStatus::Aborted | HarnessTerminalStatus::Failed => {
            HarnessEventType::RunFailed
        }
        HarnessTerminalStatus::Cancelled => HarnessEventType::RunCancelled,
        HarnessTerminalStatus::LimitReached => HarnessEventType::RunLimitReached,
        HarnessTerminalStatus::ApprovalRequired => HarnessEventType::RunApprovalRequired,
    }
}

fn status_str(status: HarnessTerminalStatus) -> &'static str {
    match status {
        HarnessTerminalStatus::Ended => "ended",
        HarnessTerminalStatus::HandedOff => "handed_off",
        HarnessTerminalStatus::Aborted => "aborted",
        HarnessTerminalStatus::Failed => "failed",
        HarnessTerminalStatus::Cancelled => "cancelled",
        HarnessTerminalStatus::LimitReached => "limit_reached",
        HarnessTerminalStatus::ApprovalRequired => "approval_required",
    }
}

fn harness_status_from_str(value: &str) -> Option<HarnessTerminalStatus> {
    match value {
        "ended" => Some(HarnessTerminalStatus::Ended),
        "handed_off" => Some(HarnessTerminalStatus::HandedOff),
        "aborted" => Some(HarnessTerminalStatus::Aborted),
        "failed" => Some(HarnessTerminalStatus::Failed),
        "cancelled" => Some(HarnessTerminalStatus::Cancelled),
        "limit_reached" => Some(HarnessTerminalStatus::LimitReached),
        "approval_required" => Some(HarnessTerminalStatus::ApprovalRequired),
        _ => None,
    }
}

fn add_optional<T>(left: Option<T>, right: Option<T>) -> Option<T>
where
    T: std::ops::Add<Output = T>,
{
    match (left, right) {
        (Some(left), Some(right)) => Some(left + right),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness_observability::InMemoryEventSink;
    use crate::harness_runtime::action::{
        ActionDispatchResult, ScriptedActionDispatcher, SemanticActionProposal,
    };
    use crate::harness_runtime::approval::ScriptedApprovalController;
    use crate::harness_runtime::model::{ModelRuntimeFailure, ModelTurn, ScriptedModelRuntime};
    use crate::manifest::{
        LoopAccessMemory, LoopCheckpoint, LoopErrorPolicy, LoopLimits, LoopMetadata, LoopOutcome,
        LoopPhaseAccess, LoopPhaseFailurePolicy, LoopToolFailurePolicy, LoopTransition,
    };

    fn limits() -> HarnessRuntimeLimits {
        HarnessRuntimeLimits {
            max_steps: 8,
            max_model_calls_per_phase: 8,
            max_tool_calls_per_phase: 8,
            max_actions_per_phase: 16,
            max_tool_call_repairs: 2,
            max_structured_output_repairs: 2,
            max_memory_operation_repairs: 2,
        }
    }

    fn base_loop() -> LoopManifest {
        LoopManifest {
            kind: "loop".into(),
            name: "@zack/test-loop".into(),
            version: "0.1.0".into(),
            description: None,
            readme: None,
            license: None,
            r#loop: LoopMetadata {
                archetype: Some("test".into()),
                entry_phase: "assess".into(),
                limits: None,
                phases: vec![
                    LoopPhase {
                        id: "assess".into(),
                        objective: "Assess request.".into(),
                        access: None,
                        outcomes: vec![
                            LoopOutcome {
                                id: "execute".into(),
                                description: "Execute.".into(),
                            },
                            LoopOutcome {
                                id: "handoff".into(),
                                description: "Hand off.".into(),
                            },
                        ],
                    },
                    LoopPhase {
                        id: "execute".into(),
                        objective: "Execute.".into(),
                        access: None,
                        outcomes: vec![LoopOutcome {
                            id: "review".into(),
                            description: "Review.".into(),
                        }],
                    },
                    LoopPhase {
                        id: "review".into(),
                        objective: "Review.".into(),
                        access: None,
                        outcomes: vec![
                            LoopOutcome {
                                id: "again".into(),
                                description: "Try again.".into(),
                            },
                            LoopOutcome {
                                id: "ready".into(),
                                description: "Ready.".into(),
                            },
                        ],
                    },
                ],
                transitions: vec![
                    LoopTransition {
                        from: "assess".into(),
                        on: "execute".into(),
                        to: "execute".into(),
                    },
                    LoopTransition {
                        from: "assess".into(),
                        on: "handoff".into(),
                        to: "$handoff".into(),
                    },
                    LoopTransition {
                        from: "execute".into(),
                        on: "review".into(),
                        to: "review".into(),
                    },
                    LoopTransition {
                        from: "review".into(),
                        on: "again".into(),
                        to: "execute".into(),
                    },
                    LoopTransition {
                        from: "review".into(),
                        on: "ready".into(),
                        to: "$end".into(),
                    },
                ],
                checkpoints: Vec::new(),
                error_policy: None,
            },
        }
    }

    fn completion(id: &str, outcome: &str) -> ModelTurn {
        ModelTurn {
            assistant_content: Some(format!("content for {outcome}")),
            actions: vec![SemanticActionProposal::new(
                id,
                SemanticAction::PhaseCompletion {
                    outcome: Some(outcome.into()),
                    output: Some(json!({ "outcome": outcome })),
                },
            )],
            usage: RunUsage::default(),
        }
    }

    fn tool_turn(tool: &str) -> ModelTurn {
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "tool",
                SemanticAction::AgentPmTool {
                    tool: tool.into(),
                    arguments: json!({ "query": "x" }),
                },
            )],
            usage: RunUsage::default(),
        }
    }

    fn run_engine(
        loop_manifest: LoopManifest,
        turns: Vec<ModelTurn>,
    ) -> (HarnessRunResult, HarnessSession, ScriptedModelRuntime) {
        let mut engine = HarnessEngine::new(loop_manifest, HarnessEngineOptions::new(limits()));
        let mut session = HarnessSession::new();
        let memory = InMemoryEventSink::default();
        session.emitter.add_sink(Box::new(memory));
        let mut model = ScriptedModelRuntime::new(turns);
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
    }

    fn run_tool_failure_policy(
        tool_failure: LoopToolFailurePolicy,
        dispatcher_results: Vec<ActionDispatchResult>,
    ) -> (RuntimeTerminalResult, ScriptedActionDispatcher) {
        let mut loop_manifest = base_loop();
        loop_manifest.r#loop.error_policy = Some(LoopErrorPolicy {
            tool_failure: Some(tool_failure),
            phase_failure: None,
        });
        let mut engine = HarnessEngine::new(loop_manifest, HarnessEngineOptions::new(limits()));
        let mut session = HarnessSession::new();
        let mut model = ScriptedModelRuntime::new(vec![tool_turn("@zack/search")]);
        let mut dispatcher = ScriptedActionDispatcher::default();
        for result in dispatcher_results {
            dispatcher.push_result("@zack/search", result);
        }
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
        (*result, dispatcher)
    }

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
    fn multi_turn_phase_processes_multiple_ordered_actions() {
        let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
        let mut session = HarnessSession::new();
        let mut model = ScriptedModelRuntime::new(vec![
            ModelTurn {
                assistant_content: Some("I will act.".into()),
                actions: vec![
                    SemanticActionProposal::new(
                        "tool-1",
                        SemanticAction::AgentPmTool {
                            tool: "@zack/search".into(),
                            arguments: json!({}),
                        },
                    ),
                    SemanticActionProposal::new(
                        "skill-1",
                        SemanticAction::SkillResourceRead {
                            skill: "@zack/skill".into(),
                            resource: "SKILL.md".into(),
                        },
                    ),
                ],
                usage: RunUsage::default(),
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
        assert_eq!(dispatcher.dispatched[1].identity(), "@zack/skill/SKILL.md");
        assert_eq!(model.requests.len(), 4);
        assert!(model.requests[1].transcript.len() > model.requests[0].transcript.len());
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
        let mut session = HarnessSession::new();
        let mut model = ScriptedModelRuntime::new(vec![
            tool_turn("@zack/search"),
            ModelTurn {
                assistant_content: None,
                actions: vec![SemanticActionProposal::new(
                    "skill",
                    SemanticAction::SkillResourceRead {
                        skill: "@zack/skill".into(),
                        resource: "SKILL.md".into(),
                    },
                )],
                usage: RunUsage::default(),
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
        let mut session = HarnessSession::new();
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
            ActionDispatchResult::success(json!({"ok": true})),
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
    }

    #[test]
    fn default_action_failure_becomes_runtime_failed() {
        let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
        let mut session = HarnessSession::new();
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
            let mut model = ScriptedModelRuntime::with_results(vec![Err(
                ModelRuntimeFailure::new("model down"),
            )]);
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
            let mut model = ScriptedModelRuntime::with_results(vec![Err(
                ModelRuntimeFailure::new("model down"),
            )]);
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
        let mut session = HarnessSession::new();
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
        let mut session = HarnessSession::new();
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
        let mut session = HarnessSession::new();
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
        let mut session = HarnessSession::new();
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
        let event_types: Vec<_> = handle
            .events()
            .iter()
            .map(|event| event.event_type)
            .collect();
        assert!(event_types.contains(&HarnessEventType::RunStarted));
        assert!(event_types.contains(&HarnessEventType::PhaseStarted));
        assert!(event_types.contains(&HarnessEventType::ToolCompleted));
        assert!(event_types.contains(&HarnessEventType::TransitionSelected));
        assert!(event_types.contains(&HarnessEventType::RunCompleted));
        assert!(event_types.contains(&HarnessEventType::SessionUsageUpdated));
    }

    #[test]
    fn non_tool_action_failures_emit_specific_failed_event_types() {
        let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
        let mut session = HarnessSession::new();
        let memory = InMemoryEventSink::default();
        let handle = memory.clone();
        session.emitter.add_sink(Box::new(memory));
        let mut model = ScriptedModelRuntime::new(vec![ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "knowledge",
                SemanticAction::KnowledgeRequest {
                    package: "@zack/guide".into(),
                    query: "x".into(),
                },
            )],
            usage: RunUsage::default(),
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
        let event_types: Vec<_> = handle
            .events()
            .iter()
            .map(|event| event.event_type)
            .collect();
        assert!(event_types.contains(&HarnessEventType::KnowledgeFailed));
        assert!(!event_types.contains(&HarnessEventType::SemanticActionCompleted));
    }
}
