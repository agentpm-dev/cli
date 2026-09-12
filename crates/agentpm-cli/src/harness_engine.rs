#![allow(dead_code)]

use crate::harness_config::{HarnessHookId, HarnessMemoryWriteReviewPoint, HarnessRuntimeLimits};
use crate::harness_observability::{
    ActionReportSummary, CheckpointReportSummary, HARNESS_REPORT_SCHEMA_VERSION,
    HarnessEventBuilder, HarnessEventEmitter, HarnessEventPayload, HarnessEventType,
    HarnessTerminalStatus, MemoryWriteReviewReportSummary, OperationReportSummary,
    PhaseReportSummary, ReportPackageIdentity, RunReport, RunUsage, SessionUsage,
    allocate_harness_run_id, allocate_harness_session_id,
};
use crate::harness_plan::{PreflightDiagnostic, PreflightStatus};
use crate::harness_runtime::action::{ActionFailureCategory, MemoryReadMode, MemoryWriteOperation};
use crate::harness_runtime::hook::{
    BeforeMemoryReadHook, BeforeMemoryWriteHook, after_knowledge_retrieval_hook_from_result,
    apply_after_knowledge_retrieval_decision, apply_before_knowledge_request_decision,
    apply_before_model_request_decision, apply_before_tool_call_decision,
    apply_before_tool_selection_decision, before_knowledge_request_hook_from_action,
    before_model_request_hook_from_request, before_tool_selection_hook_from_phase,
};
use crate::harness_runtime::memory::{
    LocalMemoryActionError, LocalMemoryReadMode, LocalMemoryReadRequest, LocalMemorySemanticConfig,
    LocalMemoryWriteOperation, LocalMemoryWriteRequest, LocalSqliteMemoryRuntime,
    MemoryContractCache, custom_memory_action_error_from_output,
    custom_memory_read_request_from_local, custom_memory_write_request_from_local,
};
use crate::harness_runtime::model::ModelTurn;
use crate::harness_runtime::model::PromptAssemblyPurpose;
use crate::harness_runtime::model::{CONSUMER_RUN_CONTEXT_SECTION_TITLE, CompletionContract};
use crate::harness_runtime::{
    ActionDispatchResult, ActionDispatcher, ApprovalController, ApprovalDecision,
    BeforeToolCallHook, CapabilityDescriptor, CustomMemoryRuntime, HookRuntime,
    MemorySpaceRuntimeSnapshot, ModelRequest, ModelRuntime, NoopHookRuntime, ProfileSnapshot,
    PromptAssemblyInput, RuntimeCapabilitySnapshot, RuntimeSnapshot, SemanticAction,
    ServiceLifecycleEvents, SkillRuntimeSnapshot, ToolRuntimeSnapshot, TranscriptEntry,
    TranscriptEntryKind, assemble_logical_prompt,
};
use crate::harness_runtime::{KnowledgeRuntime, KnowledgeRuntimeSnapshot, NoopKnowledgeRuntime};
use crate::manifest::{
    LoopManifest, LoopPhase, LoopPhaseFailureAction, LoopToolFailureAction,
    LoopToolFailureExhaustedAction, MemoryRetrievalMode, MemorySpaceModel, ProfileMetadata,
    load_manifest_value, parse_memory_manifest,
};
use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Utc};
use jsonschema::{Draft, JSONSchema};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::time::Instant;

mod effective_phase;
mod knowledge_actions;
mod lifecycle;
mod memory_actions;
mod memory_lifecycle;
mod observability;
mod persistence_review;
mod validation;

#[cfg(test)]
mod tests;

pub use effective_phase::EffectivePhase;
use effective_phase::{
    active_memory_space, memory_action_identity, memory_read_mode_label,
    memory_write_operation_label,
};
pub use memory_lifecycle::{MemoryOperationControlError, MemoryOperationInvocationResult};
use observability::*;
use validation::*;

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
    pub runtime: RuntimeSnapshot,
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
    pub tool_call_repairs: u64,
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
    memory_write_review_summaries: Vec<MemoryWriteReviewReportSummary>,
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
    pub runtime_snapshot: RuntimeSnapshot,
    memory_contract_cache: MemoryContractCache,
    local_memory_runtime: Option<LocalSqliteMemoryRuntime>,
    active_run: Option<RunState>,
}

impl HarnessSession {
    pub fn new() -> Self {
        let session_id = allocate_harness_session_id();
        let runtime_snapshot = RuntimeSnapshot::empty(session_id.clone());
        Self {
            emitter: HarnessEventEmitter::new(session_id.clone()),
            session_id,
            usage: SessionUsage::default(),
            runtime_snapshot,
            memory_contract_cache: MemoryContractCache::new(),
            local_memory_runtime: None,
            active_run: None,
        }
    }

    pub fn with_runtime_snapshot(mut runtime_snapshot: RuntimeSnapshot) -> Self {
        let session_id = allocate_harness_session_id();
        runtime_snapshot.session_id = session_id.clone();
        Self {
            emitter: HarnessEventEmitter::new(session_id.clone()),
            session_id,
            usage: SessionUsage::default(),
            runtime_snapshot,
            memory_contract_cache: MemoryContractCache::new(),
            local_memory_runtime: None,
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
        self.start_run_with_id(allocate_harness_run_id(), input)
    }

    fn start_run_with_id(&mut self, run_id: String, input: String) -> Result<String> {
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
        let runtime = self.snapshot_runtime_for_run(&run_id)?;
        self.active_run = Some(RunState {
            context: RunContext {
                run_id: run_id.clone(),
                input,
                runtime,
            },
            status: RuntimeTerminalStatus::Running,
            current_phase_id: None,
            step_count: 0,
            phase_results: Vec::new(),
            phase_summaries: Vec::new(),
            action_summaries: Vec::new(),
            memory_write_review_summaries: Vec::new(),
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

    fn snapshot_runtime_for_run(&mut self, run_id: &str) -> Result<RuntimeSnapshot> {
        let mut runtime = self.runtime_snapshot.clone();
        let Some(context) = runtime.consumer_context.as_mut() else {
            return Ok(runtime);
        };
        let Some(path) = context.path.clone() else {
            if context.file.is_some() {
                self.emitter.emit(
                    HarnessEventType::ConsumerContextUnavailable,
                    HarnessEventPayload::Lifecycle {
                        message: "Consumer context is unavailable for this Run.".into(),
                        fields: BTreeMap::from([("state".into(), json!(context.state))]),
                    },
                    HarnessEventBuilder {
                        run_id: Some(run_id.to_string()),
                        ..HarnessEventBuilder::default()
                    },
                )?;
            }
            return Ok(runtime);
        };
        match fs::read(&path) {
            Ok(bytes) => {
                let content = String::from_utf8_lossy(&bytes).to_string();
                context.state = "Available".into();
                context.byte_size = Some(bytes.len() as u64);
                context.approximate_tokens = Some(estimate_tokens(&content));
                context.sha256 = Some(format!("sha256:{:x}", Sha256::digest(&bytes)));
                context.content = Some(content);
                self.emitter.emit(
                    HarnessEventType::ConsumerContextLoaded,
                    HarnessEventPayload::Lifecycle {
                        message: "Consumer context loaded for this Run.".into(),
                        fields: BTreeMap::from([
                            ("path".into(), json!(path)),
                            ("byte_size".into(), json!(context.byte_size)),
                            (
                                "approximate_tokens".into(),
                                json!(context.approximate_tokens),
                            ),
                            ("sha256".into(), json!(context.sha256)),
                        ]),
                    },
                    HarnessEventBuilder {
                        run_id: Some(run_id.to_string()),
                        ..HarnessEventBuilder::default()
                    },
                )?;
            }
            Err(err) => {
                context.state = "Unavailable".into();
                context.content = None;
                self.emitter.emit(
                    HarnessEventType::ConsumerContextUnavailable,
                    HarnessEventPayload::Lifecycle {
                        message: "Consumer context could not be loaded for this Run.".into(),
                        fields: BTreeMap::from([
                            ("path".into(), json!(path)),
                            ("error".into(), json!(err.to_string())),
                        ]),
                    },
                    HarnessEventBuilder {
                        run_id: Some(run_id.to_string()),
                        ..HarnessEventBuilder::default()
                    },
                )?;
            }
        }
        Ok(runtime)
    }

    fn local_memory_runtime(&mut self) -> Result<&mut LocalSqliteMemoryRuntime> {
        if self.local_memory_runtime.is_none() {
            self.local_memory_runtime = Some(LocalSqliteMemoryRuntime::open(
                &self.runtime_snapshot.workspace_root,
                Some(&self.runtime_snapshot.state_dir),
            )?);
        }
        Ok(self
            .local_memory_runtime
            .as_mut()
            .expect("local MemoryRuntime was initialized"))
    }
}

fn estimate_tokens(content: &str) -> u64 {
    let words = content.split_whitespace().count() as u64;
    words.max((content.len() as u64).div_ceil(4))
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
    pub memory_write_review_points: Vec<HarnessMemoryWriteReviewPoint>,
}

impl HarnessEngineOptions {
    pub fn new(runtime_limits: HarnessRuntimeLimits) -> Self {
        Self {
            runtime_limits,
            retain_active_on_approval_required: false,
            memory_write_review_points: Vec::new(),
        }
    }

    pub fn with_memory_write_review_points(
        mut self,
        points: Vec<HarnessMemoryWriteReviewPoint>,
    ) -> Self {
        self.memory_write_review_points = points;
        self
    }
}

pub struct HarnessEngine {
    loop_manifest: LoopManifest,
    options: HarnessEngineOptions,
    phase_executions: u64,
    control_ingress: Option<Box<dyn EngineControlIngress>>,
}

pub struct HarnessRuntimeServices<'a> {
    pub model: &'a mut dyn ModelRuntime,
    pub dispatcher: &'a mut dyn ActionDispatcher,
    pub knowledge: &'a mut dyn KnowledgeRuntime,
    pub memory: Option<CustomMemoryRuntime>,
    pub embedding_provider: Option<Box<dyn crate::harness_runtime::EmbeddingProvider>>,
    pub approvals: &'a mut dyn ApprovalController,
    pub hooks: &'a mut dyn HookRuntime,
    pub service_events: Option<&'a mut ServiceLifecycleEvents>,
}

pub trait EngineControlIngress {
    fn service_memory_operation_controls(
        &mut self,
        engine: &mut HarnessEngine,
        session: &mut HarnessSession,
        model: &mut dyn ModelRuntime,
        hooks: &mut dyn HookRuntime,
    ) -> Result<()>;

    fn flush_memory_operation_controls(&mut self, code: &str, message: &str) -> Result<()>;
}

struct HookEventContext<'a> {
    run_id: &'a str,
    phase_id: &'a str,
    phase_execution_id: &'a str,
    hook: &'a HarnessHookId,
    binding_count: usize,
}

impl HarnessEngine {
    pub fn new(loop_manifest: LoopManifest, options: HarnessEngineOptions) -> Self {
        Self {
            loop_manifest,
            options,
            phase_executions: 0,
            control_ingress: None,
        }
    }

    pub fn set_control_ingress(&mut self, control_ingress: Box<dyn EngineControlIngress>) {
        self.control_ingress = Some(control_ingress);
    }

    pub fn execute_run(
        &mut self,
        session: &mut HarnessSession,
        input: impl Into<String>,
        model: &mut dyn ModelRuntime,
        dispatcher: &mut dyn ActionDispatcher,
        approvals: &mut dyn ApprovalController,
    ) -> Result<HarnessRunResult> {
        let mut hooks = NoopHookRuntime;
        let mut knowledge = NoopKnowledgeRuntime;
        let mut services = HarnessRuntimeServices {
            model,
            dispatcher,
            knowledge: &mut knowledge,
            memory: None,
            embedding_provider: None,
            approvals,
            hooks: &mut hooks,
            service_events: None,
        };
        self.execute_run_with_id(session, allocate_harness_run_id(), input, &mut services)
    }

    fn service_memory_operation_controls(
        &mut self,
        session: &mut HarnessSession,
        model: &mut dyn ModelRuntime,
        hooks: &mut dyn HookRuntime,
    ) -> Result<()> {
        let Some(mut control_ingress) = self.control_ingress.take() else {
            return Ok(());
        };
        let result = control_ingress.service_memory_operation_controls(self, session, model, hooks);
        self.control_ingress = Some(control_ingress);
        result
    }

    fn flush_memory_operation_controls(&mut self, code: &str, message: &str) -> Result<()> {
        let Some(mut control_ingress) = self.control_ingress.take() else {
            return Ok(());
        };
        let result = control_ingress.flush_memory_operation_controls(code, message);
        self.control_ingress = Some(control_ingress);
        result
    }

    pub fn execute_run_with_id(
        &mut self,
        session: &mut HarnessSession,
        run_id: String,
        input: impl Into<String>,
        services: &mut HarnessRuntimeServices<'_>,
    ) -> Result<HarnessRunResult> {
        let run_id = session.start_run_with_id(run_id, input.into())?;
        self.emit_service_lifecycle_events(session, Some(&run_id), &mut services.service_events)?;
        let mut current_phase = self.loop_manifest.r#loop.entry_phase.clone();
        loop {
            // Checkpoints are evaluated before entering the target phase. A
            // rejection can route directly to another phase or terminal target,
            // while an unresolved approval either pauses or terminalizes the Run
            // depending on the execution surface.
            if let Some(result) = self.evaluate_checkpoints(
                session,
                &run_id,
                &current_phase,
                services.approvals,
                &mut services.service_events,
            )? {
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
            let phase_result = match self.execute_phase(
                session,
                &phase,
                services.model,
                services.dispatcher,
                services.knowledge,
                &mut services.memory,
                &mut services.embedding_provider,
                services.hooks,
                &mut services.service_events,
            ) {
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
                    output: None,
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

    fn emit_nonfatal_hook_failures(
        &self,
        session: &mut HarnessSession,
        run_id: &str,
        phase_id: &str,
        phase_execution_id: &str,
        hooks: &mut dyn HookRuntime,
    ) -> Result<()> {
        for failure in hooks.drain_nonfatal_failures() {
            let fields = hook_event_fields(
                &failure.hook,
                phase_id,
                hooks.binding_count(&failure.hook),
                BTreeMap::from([("nonfatal".into(), json!(true))]),
            );
            session.emitter.emit(
                HarnessEventType::HookFailed,
                HarnessEventPayload::Lifecycle {
                    message: failure.message,
                    fields,
                },
                HarnessEventBuilder {
                    run_id: Some(run_id.to_string()),
                    phase_execution_id: Some(phase_execution_id.to_string()),
                    ..HarnessEventBuilder::default()
                },
            )?;
        }
        Ok(())
    }

    fn emit_service_lifecycle_events(
        &self,
        session: &mut HarnessSession,
        run_id: Option<&str>,
        service_events: &mut Option<&mut ServiceLifecycleEvents>,
    ) -> Result<()> {
        let Some(events) = service_events.as_deref_mut() else {
            return Ok(());
        };
        for event in events.drain() {
            session.emitter.emit(
                event.event_type,
                HarnessEventPayload::Service {
                    service: event.service,
                    status: event.status,
                    fields: BTreeMap::from([
                        ("registry_id".into(), json!(event.registry_id)),
                        ("message".into(), json!(event.message)),
                    ]),
                },
                HarnessEventBuilder {
                    run_id: run_id.map(str::to_string),
                    ..HarnessEventBuilder::default()
                },
            )?;
        }
        Ok(())
    }

    fn emit_hook_started(
        &self,
        session: &mut HarnessSession,
        context: HookEventContext<'_>,
        extra_fields: BTreeMap<String, Value>,
    ) -> Result<()> {
        session.emitter.emit(
            HarnessEventType::HookStarted,
            HarnessEventPayload::Lifecycle {
                message: format!("Hook `{}` started.", hook_id_label(context.hook)),
                fields: hook_event_fields(
                    context.hook,
                    context.phase_id,
                    context.binding_count,
                    extra_fields,
                ),
            },
            HarnessEventBuilder {
                run_id: Some(context.run_id.to_string()),
                phase_execution_id: Some(context.phase_execution_id.to_string()),
                ..HarnessEventBuilder::default()
            },
        )?;
        Ok(())
    }

    fn emit_hook_completed(
        &self,
        session: &mut HarnessSession,
        context: HookEventContext<'_>,
        extra_fields: BTreeMap<String, Value>,
    ) -> Result<()> {
        session.emitter.emit(
            HarnessEventType::HookCompleted,
            HarnessEventPayload::Lifecycle {
                message: format!("Hook `{}` completed.", hook_id_label(context.hook)),
                fields: hook_event_fields(
                    context.hook,
                    context.phase_id,
                    context.binding_count,
                    extra_fields,
                ),
            },
            HarnessEventBuilder {
                run_id: Some(context.run_id.to_string()),
                phase_execution_id: Some(context.phase_execution_id.to_string()),
                ..HarnessEventBuilder::default()
            },
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_phase(
        &mut self,
        session: &mut HarnessSession,
        phase: &LoopPhase,
        model: &mut dyn ModelRuntime,
        dispatcher: &mut dyn ActionDispatcher,
        knowledge: &mut dyn KnowledgeRuntime,
        memory: &mut Option<CustomMemoryRuntime>,
        embedding_provider: &mut Option<Box<dyn crate::harness_runtime::EmbeddingProvider>>,
        hooks: &mut dyn HookRuntime,
        service_events: &mut Option<&mut ServiceLifecycleEvents>,
    ) -> Result<PhaseResult> {
        // A phase execution is one entry into a Loop phase. Re-entering the same
        // phase later creates a new phase_execution_id and fresh phase-local
        // transcript/counters.
        self.phase_executions += 1;
        let phase_execution_id = format!("phase-exec-{}", self.phase_executions);
        let run_id = self.active_run(session)?.run_id().to_string();
        let mut effective_phase =
            EffectivePhase::from_phase(phase, &self.active_run(session)?.context.runtime);
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
                output: None,
            },
            HarnessEventBuilder {
                run_id: Some(run_id.clone()),
                phase_execution_id: Some(phase_execution_id.clone()),
                ..HarnessEventBuilder::default()
            },
        )?;
        let explicit_outcomes: Vec<String> = phase
            .outcomes
            .iter()
            .map(|outcome| outcome.id.clone())
            .collect();
        let completion = CompletionContract {
            phase_id: phase.id.clone(),
            explicit_outcomes: explicit_outcomes.clone(),
            implicit_complete: explicit_outcomes.is_empty(),
        };
        let before_tool_selection_hook = HarnessHookId::BeforeToolSelection;
        let before_tool_selection_binding_count = hooks.binding_count(&before_tool_selection_hook);
        let before_tool_selection_enabled = before_tool_selection_binding_count > 0;
        let tool_candidate_ids_before = tool_candidate_ids(&effective_phase);
        if before_tool_selection_enabled {
            self.emit_hook_started(
                session,
                HookEventContext {
                    run_id: &run_id,
                    phase_id: &phase.id,
                    phase_execution_id: &phase_execution_id,
                    hook: &before_tool_selection_hook,
                    binding_count: before_tool_selection_binding_count,
                },
                BTreeMap::from([
                    (
                        "candidate_count_before".into(),
                        json!(tool_candidate_ids_before.len()),
                    ),
                    (
                        "candidate_ids_before".into(),
                        json!(tool_candidate_ids_before.clone()),
                    ),
                ]),
            )?;
        }
        match hooks.before_tool_selection(before_tool_selection_hook_from_phase(
            &phase.id,
            &phase.objective,
            completion,
            &effective_phase,
        )) {
            Ok(decision) => {
                let patched = decision.candidate_ids.is_some();
                if let Err(err) =
                    apply_before_tool_selection_decision(&mut effective_phase, decision)
                {
                    self.emit_nonfatal_hook_failures(
                        session,
                        &run_id,
                        &phase.id,
                        &phase_execution_id,
                        hooks,
                    )?;
                    session.emitter.emit(
                        HarnessEventType::HookFailed,
                        HarnessEventPayload::Lifecycle {
                            message: err.clone(),
                            fields: hook_event_fields(
                                &before_tool_selection_hook,
                                &phase.id,
                                before_tool_selection_binding_count,
                                BTreeMap::from([
                                    (
                                        "candidate_count_before".into(),
                                        json!(tool_candidate_ids_before.len()),
                                    ),
                                    ("patched".into(), json!(patched)),
                                ]),
                            ),
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
                        format!("before_tool_selection hook returned invalid patch: {err}"),
                        None,
                    );
                }
                if before_tool_selection_enabled {
                    let tool_candidate_ids_after = tool_candidate_ids(&effective_phase);
                    self.emit_nonfatal_hook_failures(
                        session,
                        &run_id,
                        &phase.id,
                        &phase_execution_id,
                        hooks,
                    )?;
                    self.emit_hook_completed(
                        session,
                        HookEventContext {
                            run_id: &run_id,
                            phase_id: &phase.id,
                            phase_execution_id: &phase_execution_id,
                            hook: &before_tool_selection_hook,
                            binding_count: before_tool_selection_binding_count,
                        },
                        BTreeMap::from([
                            (
                                "candidate_count_before".into(),
                                json!(tool_candidate_ids_before.len()),
                            ),
                            (
                                "candidate_count_after".into(),
                                json!(tool_candidate_ids_after.len()),
                            ),
                            (
                                "candidate_ids_before".into(),
                                json!(tool_candidate_ids_before.clone()),
                            ),
                            (
                                "candidate_ids_after".into(),
                                json!(tool_candidate_ids_after),
                            ),
                            ("patched".into(), json!(patched)),
                        ]),
                    )?;
                }
            }
            Err(err) => {
                let is_rejection = err.is_rejection();
                self.emit_nonfatal_hook_failures(
                    session,
                    &run_id,
                    &phase.id,
                    &phase_execution_id,
                    hooks,
                )?;
                session.emitter.emit(
                    if is_rejection {
                        HarnessEventType::HookRejected
                    } else {
                        HarnessEventType::HookFailed
                    },
                    HarnessEventPayload::Lifecycle {
                        message: err.message.clone(),
                        fields: hook_event_fields(
                            &before_tool_selection_hook,
                            &phase.id,
                            before_tool_selection_binding_count,
                            BTreeMap::from([(
                                "candidate_count_before".into(),
                                json!(tool_candidate_ids_before.len()),
                            )]),
                        ),
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
                    format!(
                        "before_tool_selection hook {}: {}",
                        if is_rejection { "rejected" } else { "failed" },
                        err.message
                    ),
                    None,
                );
            }
        }
        self.emit_service_lifecycle_events(session, Some(&run_id), service_events)?;
        session.emitter.emit(
            HarnessEventType::EffectivePhaseComputed,
            HarnessEventPayload::Lifecycle {
                message: "Effective phase computed.".into(),
                fields: BTreeMap::from([
                    ("phase_id".into(), json!(phase.id)),
                    (
                        "profile_candidates".into(),
                        json!(effective_phase.authored_profile_candidates.clone()),
                    ),
                    (
                        "active_profiles".into(),
                        json!(effective_phase.active_profiles.clone()),
                    ),
                    (
                        "suppressed_profiles".into(),
                        json!(
                            effective_phase
                                .suppressed_capabilities
                                .iter()
                                .filter(|capability| capability.kind == "profile")
                                .cloned()
                                .collect::<Vec<_>>()
                        ),
                    ),
                    (
                        "capability_descriptors".into(),
                        json!(effective_phase.capability_catalog.clone()),
                    ),
                    (
                        "suppressed_capabilities".into(),
                        json!(effective_phase.suppressed_capabilities.clone()),
                    ),
                ]),
            },
            HarnessEventBuilder {
                run_id: Some(run_id.clone()),
                phase_execution_id: Some(phase_execution_id.clone()),
                ..HarnessEventBuilder::default()
            },
        )?;
        session.emitter.emit(
            HarnessEventType::ToolCandidatesComputed,
            HarnessEventPayload::Lifecycle {
                message: "Tool candidates computed.".into(),
                fields: BTreeMap::from([
                    ("phase_id".into(), json!(phase.id)),
                    (
                        "ready".into(),
                        json!(
                            effective_phase
                                .capability_catalog
                                .iter()
                                .filter(|descriptor| descriptor.action_kind == "agentpm_tool")
                                .cloned()
                                .collect::<Vec<_>>()
                        ),
                    ),
                    (
                        "suppressed".into(),
                        json!(
                            effective_phase
                                .suppressed_capabilities
                                .iter()
                                .filter(|capability| capability.kind == "tool")
                                .cloned()
                                .collect::<Vec<_>>()
                        ),
                    ),
                ]),
            },
            HarnessEventBuilder {
                run_id: Some(run_id.clone()),
                phase_execution_id: Some(phase_execution_id.clone()),
                ..HarnessEventBuilder::default()
            },
        )?;
        for knowledge in &effective_phase.active_knowledge {
            session.emitter.emit(
                HarnessEventType::KnowledgeSurfaceReady,
                HarnessEventPayload::Action {
                    action_kind: "knowledge_request".into(),
                    identity: knowledge.name.clone(),
                    status: "available".into(),
                    fields: BTreeMap::from([
                        ("source".into(), json!(knowledge.source.clone())),
                        ("runtime".into(), json!(knowledge.runtime.clone())),
                        ("mode".into(), json!(knowledge.mode.clone())),
                        ("documents".into(), json!(knowledge.documents.clone())),
                        ("embedding".into(), json!(knowledge.embedding.clone())),
                        ("retrieval".into(), json!(knowledge.retrieval.clone())),
                    ]),
                },
                HarnessEventBuilder {
                    run_id: Some(run_id.clone()),
                    phase_execution_id: Some(phase_execution_id.clone()),
                    ..HarnessEventBuilder::default()
                },
            )?;
        }
        for knowledge in effective_phase
            .suppressed_capabilities
            .iter()
            .filter(|capability| capability.kind == "knowledge")
        {
            session.emitter.emit(
                HarnessEventType::KnowledgeSurfaceUnavailable,
                HarnessEventPayload::Action {
                    action_kind: "knowledge_request".into(),
                    identity: knowledge.identity.clone(),
                    status: "unavailable".into(),
                    fields: BTreeMap::from([
                        ("source".into(), json!(knowledge.source.clone())),
                        ("reason".into(), json!(knowledge.reason.clone())),
                    ]),
                },
                HarnessEventBuilder {
                    run_id: Some(run_id.clone()),
                    phase_execution_id: Some(phase_execution_id.clone()),
                    ..HarnessEventBuilder::default()
                },
            )?;
        }
        for memory in &effective_phase.active_memory {
            session.emitter.emit(
                HarnessEventType::MemorySurfaceReady,
                HarnessEventPayload::Action {
                    action_kind: "memory".into(),
                    identity: memory_action_identity(&memory.package, &memory.space),
                    status: "available".into(),
                    fields: BTreeMap::from([
                        ("source".into(), json!(memory.source.clone())),
                        ("runtime".into(), json!(memory.runtime.clone())),
                        ("package".into(), json!(memory.package.clone())),
                        (
                            "package_version".into(),
                            json!(memory.package_version.clone()),
                        ),
                        ("space".into(), json!(memory.space.clone())),
                        ("model".into(), json!(memory.model.clone())),
                        ("scope_keys".into(), json!(memory.scope_keys.clone())),
                        (
                            "retrieval_modes".into(),
                            json!(memory.retrieval_modes.clone()),
                        ),
                        (
                            "record_types".into(),
                            json!(
                                memory
                                    .record_types
                                    .iter()
                                    .map(|record_type| record_type.name.clone())
                                    .collect::<Vec<_>>()
                            ),
                        ),
                    ]),
                },
                HarnessEventBuilder {
                    run_id: Some(run_id.clone()),
                    phase_execution_id: Some(phase_execution_id.clone()),
                    ..HarnessEventBuilder::default()
                },
            )?;
        }
        for operation in &effective_phase.active_memory_operations {
            session.emitter.emit(
                HarnessEventType::MemoryOperationEligible,
                HarnessEventPayload::Lifecycle {
                    message: "Memory lifecycle operation is participating in this phase.".into(),
                    fields: BTreeMap::from([
                        ("source".into(), json!(operation.source.clone())),
                        ("runtime".into(), json!(operation.runtime.clone())),
                        ("package".into(), json!(operation.package.clone())),
                        (
                            "package_version".into(),
                            json!(operation.package_version.clone()),
                        ),
                        ("operation".into(), json!(operation.operation.clone())),
                        (
                            "operation_type".into(),
                            json!(operation.operation_type.clone()),
                        ),
                        ("trigger".into(), operation.trigger.clone()),
                        ("scope_keys".into(), json!(operation.scope_keys.clone())),
                        (
                            "referenced_spaces".into(),
                            json!(operation.referenced_spaces.clone()),
                        ),
                    ]),
                },
                HarnessEventBuilder {
                    run_id: Some(run_id.clone()),
                    phase_execution_id: Some(phase_execution_id.clone()),
                    ..HarnessEventBuilder::default()
                },
            )?;
        }
        for memory in effective_phase
            .suppressed_capabilities
            .iter()
            .filter(|capability| {
                capability.kind == "memory_read" || capability.kind == "memory_write"
            })
        {
            session.emitter.emit(
                HarnessEventType::MemorySurfaceUnavailable,
                HarnessEventPayload::Action {
                    action_kind: memory.kind.clone(),
                    identity: memory.identity.clone(),
                    status: "unavailable".into(),
                    fields: BTreeMap::from([
                        ("source".into(), json!(memory.source.clone())),
                        ("reason".into(), json!(memory.reason.clone())),
                    ]),
                },
                HarnessEventBuilder {
                    run_id: Some(run_id.clone()),
                    phase_execution_id: Some(phase_execution_id.clone()),
                    ..HarnessEventBuilder::default()
                },
            )?;
        }
        for operation in effective_phase
            .suppressed_capabilities
            .iter()
            .filter(|capability| capability.kind == "memory_operation")
        {
            session.emitter.emit(
                HarnessEventType::MemoryOperationFailed,
                HarnessEventPayload::Lifecycle {
                    message: "Memory lifecycle operation is unavailable.".into(),
                    fields: BTreeMap::from([
                        ("source".into(), json!(operation.source.clone())),
                        ("identity".into(), json!(operation.identity.clone())),
                        ("reason".into(), json!(operation.reason.clone())),
                    ]),
                },
                HarnessEventBuilder {
                    run_id: Some(run_id.clone()),
                    phase_execution_id: Some(phase_execution_id.clone()),
                    ..HarnessEventBuilder::default()
                },
            )?;
        }
        for skill in &effective_phase.active_skills {
            session.emitter.emit(
                HarnessEventType::SkillActivated,
                HarnessEventPayload::Action {
                    action_kind: "skill_resource_read".into(),
                    identity: skill.name.clone(),
                    status: "available".into(),
                    fields: BTreeMap::from([
                        ("source".into(), json!(skill.source.clone())),
                        (
                            "resources".into(),
                            json!(
                                skill
                                    .resources
                                    .iter()
                                    .map(|resource| resource.id.clone())
                                    .collect::<Vec<_>>()
                            ),
                        ),
                    ]),
                },
                HarnessEventBuilder {
                    run_id: Some(run_id.clone()),
                    phase_execution_id: Some(phase_execution_id.clone()),
                    ..HarnessEventBuilder::default()
                },
            )?;
        }
        session.emitter.emit(
            HarnessEventType::PhaseStarted,
            HarnessEventPayload::Phase {
                phase_id: phase.id.clone(),
                outcome: None,
                transition_to: None,
                output: None,
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
                action_succeeded: None,
            }],
            model_calls: 0,
            accepted_actions: 0,
            logical_tool_calls: 0,
            structured_repairs: 0,
            tool_call_repairs: 0,
        };
        self.evaluate_memory_lifecycle_intervals_at_phase_start(
            session,
            &effective_phase,
            &mut state,
            phase,
            model,
            hooks,
            memory,
        )?;
        // Explicit outcomes must be selected by the model. Phases with no
        // declared outcomes use the implicit `complete` outcome.
        let mut repair_feedback = None;

        loop {
            self.service_memory_operation_controls(session, model, hooks)?;
            // Each model turn sees the immutable run input, prior completed
            // phases, this phase's transcript, the current EffectivePhase, and
            // optional repair feedback from the previous malformed proposal.
            if state.model_calls >= self.options.runtime_limits.max_model_calls_per_phase {
                return self.limit_phase(session, &phase_execution_id, "max_model_calls_per_phase");
            }
            let prompt = assemble_logical_prompt(PromptAssemblyInput {
                purpose: PromptAssemblyPurpose::Phase,
                phase_id: &phase.id,
                phase_objective: &phase.objective,
                explicit_outcomes: &explicit_outcomes,
                run_input: &self.active_run(session)?.context.input,
                consumer_context: self
                    .active_run(session)?
                    .context
                    .runtime
                    .consumer_context
                    .as_ref(),
                prior_phase_results: &self.active_run(session)?.phase_results,
                effective_phase: &effective_phase,
                transcript: &state.transcript,
                repair_feedback: repair_feedback.as_deref(),
            });
            let request = ModelRequest {
                runtime: self.active_run(session)?.context.runtime.clone(),
                model: self.active_run(session)?.context.runtime.model.clone(),
                prompt,
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
                HarnessEventType::PromptPrepared,
                HarnessEventPayload::Lifecycle {
                    message: "Canonical model prompt prepared.".into(),
                    fields: BTreeMap::from([
                        ("phase_id".into(), json!(phase.id)),
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
                    phase_execution_id: Some(phase_execution_id.clone()),
                    ..HarnessEventBuilder::default()
                },
            )?;
            let before_model_request_hook = HarnessHookId::BeforeModelRequest;
            let before_model_request_binding_count =
                hooks.binding_count(&before_model_request_hook);
            let before_model_request_enabled = before_model_request_binding_count > 0;
            let mut before_model_request_fields = BTreeMap::from([
                (
                    "section_count_before".into(),
                    json!(request.prompt.sections.len()),
                ),
                (
                    "mutable_section_count".into(),
                    json!(mutable_model_request_sections(&request)),
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
                    json!(provider_option_keys(&request)),
                ),
            ]);
            if let Some(model) = &request.model {
                before_model_request_fields
                    .insert("model_provider".into(), json!(model.provider.clone()));
                before_model_request_fields.insert("model_id".into(), json!(model.model.clone()));
            }
            if before_model_request_enabled {
                self.emit_hook_started(
                    session,
                    HookEventContext {
                        run_id: &run_id,
                        phase_id: &phase.id,
                        phase_execution_id: &phase_execution_id,
                        hook: &before_model_request_hook,
                        binding_count: before_model_request_binding_count,
                    },
                    before_model_request_fields.clone(),
                )?;
            }
            let mut request = request;
            match hooks.before_model_request(before_model_request_hook_from_request(&request)) {
                Ok(decision) => {
                    let context_sections_added = decision.context_sections.len();
                    let mut provider_option_patch_keys = decision
                        .provider_options
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>();
                    provider_option_patch_keys.sort();
                    let patched =
                        context_sections_added > 0 || !provider_option_patch_keys.is_empty();
                    if let Err(err) = apply_before_model_request_decision(&mut request, decision) {
                        self.emit_nonfatal_hook_failures(
                            session,
                            &run_id,
                            &phase.id,
                            &phase_execution_id,
                            hooks,
                        )?;
                        session.emitter.emit(
                            HarnessEventType::HookFailed,
                            HarnessEventPayload::Lifecycle {
                                message: err.clone(),
                                fields: hook_event_fields(
                                    &before_model_request_hook,
                                    &phase.id,
                                    before_model_request_binding_count,
                                    BTreeMap::from([
                                        (
                                            "context_sections_added".into(),
                                            json!(context_sections_added),
                                        ),
                                        (
                                            "provider_option_patch_keys".into(),
                                            json!(provider_option_patch_keys),
                                        ),
                                        ("patched".into(), json!(patched)),
                                    ]),
                                ),
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
                            format!("before_model_request hook returned invalid patch: {err}"),
                            None,
                        );
                    }
                    if before_model_request_enabled {
                        self.emit_nonfatal_hook_failures(
                            session,
                            &run_id,
                            &phase.id,
                            &phase_execution_id,
                            hooks,
                        )?;
                        let mut fields = before_model_request_fields.clone();
                        fields.insert(
                            "section_count_after".into(),
                            json!(request.prompt.sections.len()),
                        );
                        fields.insert(
                            "provider_option_keys_after".into(),
                            json!(provider_option_keys(&request)),
                        );
                        fields.insert(
                            "context_sections_added".into(),
                            json!(context_sections_added),
                        );
                        fields.insert(
                            "provider_option_patch_keys".into(),
                            json!(provider_option_patch_keys),
                        );
                        fields.insert("patched".into(), json!(patched));
                        self.emit_hook_completed(
                            session,
                            HookEventContext {
                                run_id: &run_id,
                                phase_id: &phase.id,
                                phase_execution_id: &phase_execution_id,
                                hook: &before_model_request_hook,
                                binding_count: before_model_request_binding_count,
                            },
                            fields,
                        )?;
                    }
                }
                Err(err) => {
                    let is_rejection = err.is_rejection();
                    self.emit_nonfatal_hook_failures(
                        session,
                        &run_id,
                        &phase.id,
                        &phase_execution_id,
                        hooks,
                    )?;
                    session.emitter.emit(
                        if is_rejection {
                            HarnessEventType::HookRejected
                        } else {
                            HarnessEventType::HookFailed
                        },
                        HarnessEventPayload::Lifecycle {
                            message: err.message.clone(),
                            fields: hook_event_fields(
                                &before_model_request_hook,
                                &phase.id,
                                before_model_request_binding_count,
                                before_model_request_fields.clone(),
                            ),
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
                        format!(
                            "before_model_request hook {}: {}",
                            if is_rejection { "rejected" } else { "failed" },
                            err.message
                        ),
                        None,
                    );
                }
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
                        phase_execution_id: Some(phase_execution_id.clone()),
                        ..HarnessEventBuilder::default()
                    },
                )?;
            }
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
                    self.emit_service_lifecycle_events(session, Some(&run_id), service_events)?;
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
                    phase_execution_id: Some(phase_execution_id.clone()),
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
                            output: None,
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
                        output: None,
                    },
                    HarnessEventBuilder {
                        run_id: Some(run_id),
                        phase_execution_id: Some(phase_execution_id.clone()),
                        ..HarnessEventBuilder::default()
                    },
                )?;
                let target = self.transition_target(&phase.id, &selected)?;
                self.run_memory_write_review_if_configured(
                    session,
                    phase,
                    &phase_execution_id,
                    &selected,
                    &target,
                    &effective_phase,
                    &state,
                    model,
                    memory,
                    embedding_provider,
                    hooks,
                    service_events,
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
                if let Err(err) = validate_semantic_action(&proposal.action, &effective_phase) {
                    session.emitter.emit(
                        HarnessEventType::SemanticActionRejected,
                        HarnessEventPayload::Action {
                            action_kind: proposal.action.kind().into(),
                            identity: proposal.action.identity(),
                            status: "invalid_arguments".into(),
                            fields: BTreeMap::from([("error".into(), json!(err))]),
                        },
                        HarnessEventBuilder {
                            run_id: Some(run_id.clone()),
                            phase_execution_id: Some(phase_execution_id.clone()),
                            ..HarnessEventBuilder::default()
                        },
                    )?;
                    repair_feedback = Some(err);
                    if proposal.action.is_tool_call() {
                        self.request_tool_call_repair(
                            session,
                            &mut state,
                            &phase_execution_id,
                            repair_feedback.clone(),
                        )?;
                    } else {
                        self.request_repair(
                            session,
                            &mut state,
                            &phase_execution_id,
                            repair_feedback.clone(),
                        )?;
                    }
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
                let action = if let SemanticAction::AgentPmTool { tool, arguments } =
                    &proposal.action
                {
                    let before_tool_call_hook = HarnessHookId::BeforeToolCall;
                    let before_tool_call_binding_count =
                        hooks.binding_count(&before_tool_call_hook);
                    let before_tool_call_enabled = before_tool_call_binding_count > 0;
                    let argument_keys_before = argument_keys(arguments);
                    if before_tool_call_enabled {
                        self.emit_hook_started(
                            session,
                            HookEventContext {
                                run_id: &run_id,
                                phase_id: &phase.id,
                                phase_execution_id: &phase_execution_id,
                                hook: &before_tool_call_hook,
                                binding_count: before_tool_call_binding_count,
                            },
                            BTreeMap::from([
                                ("tool".into(), json!(tool.clone())),
                                (
                                    "argument_keys_before".into(),
                                    json!(argument_keys_before.clone()),
                                ),
                            ]),
                        )?;
                    }
                    let action = match hooks.before_tool_call(BeforeToolCallHook {
                        phase_id: phase.id.clone(),
                        tool: tool.clone(),
                        arguments: arguments.clone(),
                    }) {
                        Ok(decision) => {
                            let patched_arguments = decision.arguments.is_some();
                            let patched =
                                match apply_before_tool_call_decision(&proposal.action, decision) {
                                    Ok(patched) => patched,
                                    Err(err) => {
                                        self.emit_nonfatal_hook_failures(
                                            session,
                                            &run_id,
                                            &phase.id,
                                            &phase_execution_id,
                                            hooks,
                                        )?;
                                        session.emitter.emit(
                                            HarnessEventType::HookFailed,
                                            HarnessEventPayload::Lifecycle {
                                                message: err.clone(),
                                                fields: hook_event_fields(
                                                    &before_tool_call_hook,
                                                    &phase.id,
                                                    before_tool_call_binding_count,
                                                    BTreeMap::from([
                                                        ("tool".into(), json!(tool.clone())),
                                                        (
                                                            "argument_keys_before".into(),
                                                            json!(argument_keys_before.clone()),
                                                        ),
                                                        (
                                                            "arguments_patched".into(),
                                                            json!(patched_arguments),
                                                        ),
                                                    ]),
                                                ),
                                            },
                                            HarnessEventBuilder {
                                                run_id: Some(run_id.clone()),
                                                phase_execution_id: Some(
                                                    phase_execution_id.clone(),
                                                ),
                                                ..HarnessEventBuilder::default()
                                            },
                                        )?;
                                        return self.fail_phase(
                                        session,
                                        &phase.id,
                                        &phase_execution_id,
                                        format!(
                                            "before_tool_call hook returned invalid patch: {err}"
                                        ),
                                        None,
                                    );
                                    }
                                };
                            if let Err(err) = validate_semantic_action(&patched, &effective_phase) {
                                self.emit_nonfatal_hook_failures(
                                    session,
                                    &run_id,
                                    &phase.id,
                                    &phase_execution_id,
                                    hooks,
                                )?;
                                session.emitter.emit(
                                    HarnessEventType::HookFailed,
                                    HarnessEventPayload::Lifecycle {
                                        message: err.clone(),
                                        fields: hook_event_fields(
                                            &before_tool_call_hook,
                                            &phase.id,
                                            before_tool_call_binding_count,
                                            BTreeMap::from([
                                                ("tool".into(), json!(tool.clone())),
                                                (
                                                    "argument_keys_before".into(),
                                                    json!(argument_keys_before.clone()),
                                                ),
                                                (
                                                    "arguments_patched".into(),
                                                    json!(patched_arguments),
                                                ),
                                            ]),
                                        ),
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
                                    format!("before_tool_call hook produced invalid action: {err}"),
                                    None,
                                );
                            }
                            if before_tool_call_enabled {
                                let argument_keys_after = match &patched {
                                    SemanticAction::AgentPmTool { arguments, .. } => {
                                        argument_keys(arguments)
                                    }
                                    _ => Vec::new(),
                                };
                                self.emit_nonfatal_hook_failures(
                                    session,
                                    &run_id,
                                    &phase.id,
                                    &phase_execution_id,
                                    hooks,
                                )?;
                                self.emit_hook_completed(
                                    session,
                                    HookEventContext {
                                        run_id: &run_id,
                                        phase_id: &phase.id,
                                        phase_execution_id: &phase_execution_id,
                                        hook: &before_tool_call_hook,
                                        binding_count: before_tool_call_binding_count,
                                    },
                                    BTreeMap::from([
                                        ("tool".into(), json!(tool.clone())),
                                        (
                                            "argument_keys_before".into(),
                                            json!(argument_keys_before.clone()),
                                        ),
                                        ("argument_keys_after".into(), json!(argument_keys_after)),
                                        ("arguments_patched".into(), json!(patched_arguments)),
                                    ]),
                                )?;
                            }
                            patched
                        }
                        Err(err) => {
                            let is_rejection = err.is_rejection();
                            self.emit_nonfatal_hook_failures(
                                session,
                                &run_id,
                                &phase.id,
                                &phase_execution_id,
                                hooks,
                            )?;
                            session.emitter.emit(
                                if is_rejection {
                                    HarnessEventType::HookRejected
                                } else {
                                    HarnessEventType::HookFailed
                                },
                                HarnessEventPayload::Lifecycle {
                                    message: err.message.clone(),
                                    fields: hook_event_fields(
                                        &before_tool_call_hook,
                                        &phase.id,
                                        before_tool_call_binding_count,
                                        BTreeMap::from([
                                            ("tool".into(), json!(tool.clone())),
                                            (
                                                "argument_keys_before".into(),
                                                json!(argument_keys_before.clone()),
                                            ),
                                        ]),
                                    ),
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
                                format!(
                                    "before_tool_call hook {} action: {}",
                                    if is_rejection { "rejected" } else { "failed" },
                                    err.message
                                ),
                                None,
                            );
                        }
                    };
                    self.emit_service_lifecycle_events(session, Some(&run_id), service_events)?;
                    action
                } else if matches!(proposal.action, SemanticAction::KnowledgeRequest { .. }) {
                    let before_knowledge_hook = HarnessHookId::BeforeKnowledgeRequest;
                    let before_knowledge_binding_count =
                        hooks.binding_count(&before_knowledge_hook);
                    let before_knowledge_enabled = before_knowledge_binding_count > 0;
                    if before_knowledge_enabled {
                        self.emit_hook_started(
                            session,
                            HookEventContext {
                                run_id: &run_id,
                                phase_id: &phase.id,
                                phase_execution_id: &phase_execution_id,
                                hook: &before_knowledge_hook,
                                binding_count: before_knowledge_binding_count,
                            },
                            action_trace_fields(&proposal.action),
                        )?;
                    }
                    let hook = match before_knowledge_request_hook_from_action(
                        &phase.id,
                        &proposal.action,
                        &effective_phase,
                    ) {
                        Ok(hook) => hook,
                        Err(err) => {
                            repair_feedback = Some(err);
                            self.request_repair(
                                session,
                                &mut state,
                                &phase_execution_id,
                                repair_feedback.clone(),
                            )?;
                            continue;
                        }
                    };
                    let action = match hooks.before_knowledge_request(hook) {
                        Ok(decision) => {
                            let patched = decision.document.is_some()
                                || decision.query.is_some()
                                || decision.top_k.is_some()
                                || decision.score_threshold.is_some()
                                || decision.return_citations.is_some();
                            let patched_action = match apply_before_knowledge_request_decision(
                                &proposal.action,
                                decision,
                            ) {
                                Ok(action) => action,
                                Err(err) => {
                                    self.emit_nonfatal_hook_failures(
                                        session,
                                        &run_id,
                                        &phase.id,
                                        &phase_execution_id,
                                        hooks,
                                    )?;
                                    session.emitter.emit(
                                        HarnessEventType::HookFailed,
                                        HarnessEventPayload::Lifecycle {
                                            message: err.clone(),
                                            fields: hook_event_fields(
                                                &before_knowledge_hook,
                                                &phase.id,
                                                before_knowledge_binding_count,
                                                action_trace_fields(&proposal.action),
                                            ),
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
                                        format!("before_knowledge_request hook returned invalid patch: {err}"),
                                        None,
                                    );
                                }
                            };
                            if let Err(err) =
                                validate_semantic_action(&patched_action, &effective_phase)
                            {
                                self.emit_nonfatal_hook_failures(
                                    session,
                                    &run_id,
                                    &phase.id,
                                    &phase_execution_id,
                                    hooks,
                                )?;
                                session.emitter.emit(
                                    HarnessEventType::HookFailed,
                                    HarnessEventPayload::Lifecycle {
                                        message: err.clone(),
                                        fields: hook_event_fields(
                                            &before_knowledge_hook,
                                            &phase.id,
                                            before_knowledge_binding_count,
                                            action_trace_fields(&proposal.action),
                                        ),
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
                                    format!("before_knowledge_request hook produced invalid action: {err}"),
                                    None,
                                );
                            }
                            if before_knowledge_enabled {
                                self.emit_nonfatal_hook_failures(
                                    session,
                                    &run_id,
                                    &phase.id,
                                    &phase_execution_id,
                                    hooks,
                                )?;
                                let mut fields = action_trace_fields(&patched_action);
                                fields.insert("patched".into(), json!(patched));
                                self.emit_hook_completed(
                                    session,
                                    HookEventContext {
                                        run_id: &run_id,
                                        phase_id: &phase.id,
                                        phase_execution_id: &phase_execution_id,
                                        hook: &before_knowledge_hook,
                                        binding_count: before_knowledge_binding_count,
                                    },
                                    fields,
                                )?;
                            }
                            patched_action
                        }
                        Err(err) => {
                            let is_rejection = err.is_rejection();
                            self.emit_nonfatal_hook_failures(
                                session,
                                &run_id,
                                &phase.id,
                                &phase_execution_id,
                                hooks,
                            )?;
                            session.emitter.emit(
                                if is_rejection {
                                    HarnessEventType::HookRejected
                                } else {
                                    HarnessEventType::HookFailed
                                },
                                HarnessEventPayload::Lifecycle {
                                    message: err.message.clone(),
                                    fields: hook_event_fields(
                                        &before_knowledge_hook,
                                        &phase.id,
                                        before_knowledge_binding_count,
                                        action_trace_fields(&proposal.action),
                                    ),
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
                                format!(
                                    "before_knowledge_request hook {} action: {}",
                                    if is_rejection { "rejected" } else { "failed" },
                                    err.message
                                ),
                                None,
                            );
                        }
                    };
                    self.emit_service_lifecycle_events(session, Some(&run_id), service_events)?;
                    action
                } else if matches!(
                    proposal.action,
                    SemanticAction::MemoryRead { .. } | SemanticAction::MemoryWrite { .. }
                ) {
                    let (before_memory_hook, before_memory_binding_count) = match &proposal.action {
                        SemanticAction::MemoryRead { .. } => {
                            let hook = HarnessHookId::BeforeMemoryRead;
                            let count = hooks.binding_count(&hook);
                            (hook, count)
                        }
                        SemanticAction::MemoryWrite { .. } => {
                            let hook = HarnessHookId::BeforeMemoryWrite;
                            let count = hooks.binding_count(&hook);
                            (hook, count)
                        }
                        _ => unreachable!(),
                    };
                    let before_memory_enabled = before_memory_binding_count > 0;
                    if before_memory_enabled {
                        self.emit_hook_started(
                            session,
                            HookEventContext {
                                run_id: &run_id,
                                phase_id: &phase.id,
                                phase_execution_id: &phase_execution_id,
                                hook: &before_memory_hook,
                                binding_count: before_memory_binding_count,
                            },
                            action_trace_fields(&proposal.action),
                        )?;
                    }
                    let action = match &proposal.action {
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
                            let Some(memory) =
                                active_memory_space(&effective_phase, package, space)
                            else {
                                let err = format!(
                                    "Memory space `{}` for package `{package}` is not available in the current EffectivePhase.",
                                    space
                                );
                                repair_feedback = Some(err);
                                self.request_repair(
                                    session,
                                    &mut state,
                                    &phase_execution_id,
                                    repair_feedback.clone(),
                                )?;
                                continue;
                            };
                            let scope = match memory_actions::resolved_memory_scope(
                                memory,
                                &session.runtime_snapshot.runtime_scopes,
                            ) {
                                Ok(scope) => scope,
                                Err(err) => {
                                    return self.fail_phase(
                                        session,
                                        &phase.id,
                                        &phase_execution_id,
                                        err.to_string(),
                                        None,
                                    );
                                }
                            };
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
                                    let patched_action = match memory_actions::apply_before_memory_read_decision_to_action(&proposal.action, decision) {
                                        Ok(action) => action,
                                        Err(err) => {
                                            self.emit_nonfatal_hook_failures(
                                                session,
                                                &run_id,
                                                &phase.id,
                                                &phase_execution_id,
                                                hooks,
                                            )?;
                                            session.emitter.emit(
                                                HarnessEventType::HookFailed,
                                                HarnessEventPayload::Lifecycle {
                                                    message: err.clone(),
                                                    fields: hook_event_fields(
                                                        &before_memory_hook,
                                                        &phase.id,
                                                        before_memory_binding_count,
                                                        action_trace_fields(&proposal.action),
                                                    ),
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
                                                format!("before_memory_read hook returned invalid patch: {err}"),
                                                None,
                                            );
                                        }
                                    };
                                    if let Err(err) =
                                        validate_semantic_action(&patched_action, &effective_phase)
                                    {
                                        self.emit_nonfatal_hook_failures(
                                            session,
                                            &run_id,
                                            &phase.id,
                                            &phase_execution_id,
                                            hooks,
                                        )?;
                                        session.emitter.emit(
                                            HarnessEventType::HookFailed,
                                            HarnessEventPayload::Lifecycle {
                                                message: err.clone(),
                                                fields: hook_event_fields(
                                                    &before_memory_hook,
                                                    &phase.id,
                                                    before_memory_binding_count,
                                                    action_trace_fields(&proposal.action),
                                                ),
                                            },
                                            HarnessEventBuilder {
                                                run_id: Some(run_id.clone()),
                                                phase_execution_id: Some(
                                                    phase_execution_id.clone(),
                                                ),
                                                ..HarnessEventBuilder::default()
                                            },
                                        )?;
                                        return self.fail_phase(
                                            session,
                                            &phase.id,
                                            &phase_execution_id,
                                            format!("before_memory_read hook produced invalid action: {err}"),
                                            None,
                                        );
                                    }
                                    if before_memory_enabled {
                                        self.emit_nonfatal_hook_failures(
                                            session,
                                            &run_id,
                                            &phase.id,
                                            &phase_execution_id,
                                            hooks,
                                        )?;
                                        let mut fields = action_trace_fields(&patched_action);
                                        fields.insert("patched".into(), json!(patched));
                                        self.emit_hook_completed(
                                            session,
                                            HookEventContext {
                                                run_id: &run_id,
                                                phase_id: &phase.id,
                                                phase_execution_id: &phase_execution_id,
                                                hook: &before_memory_hook,
                                                binding_count: before_memory_binding_count,
                                            },
                                            fields,
                                        )?;
                                    }
                                    patched_action
                                }
                                Err(err) => {
                                    let is_rejection = err.is_rejection();
                                    self.emit_nonfatal_hook_failures(
                                        session,
                                        &run_id,
                                        &phase.id,
                                        &phase_execution_id,
                                        hooks,
                                    )?;
                                    session.emitter.emit(
                                        if is_rejection {
                                            HarnessEventType::HookRejected
                                        } else {
                                            HarnessEventType::HookFailed
                                        },
                                        HarnessEventPayload::Lifecycle {
                                            message: err.message.clone(),
                                            fields: hook_event_fields(
                                                &before_memory_hook,
                                                &phase.id,
                                                before_memory_binding_count,
                                                action_trace_fields(&proposal.action),
                                            ),
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
                                        format!(
                                            "before_memory_read hook {} action: {}",
                                            if is_rejection { "rejected" } else { "failed" },
                                            err.message
                                        ),
                                        None,
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
                            let Some(memory) =
                                active_memory_space(&effective_phase, package, space)
                            else {
                                let err = format!(
                                    "Memory space `{}` for package `{package}` is not available in the current EffectivePhase.",
                                    space
                                );
                                repair_feedback = Some(err);
                                self.request_repair(
                                    session,
                                    &mut state,
                                    &phase_execution_id,
                                    repair_feedback.clone(),
                                )?;
                                continue;
                            };
                            let scope = match memory_actions::resolved_memory_scope(
                                memory,
                                &session.runtime_snapshot.runtime_scopes,
                            ) {
                                Ok(scope) => scope,
                                Err(err) => {
                                    return self.fail_phase(
                                        session,
                                        &phase.id,
                                        &phase_execution_id,
                                        err.to_string(),
                                        None,
                                    );
                                }
                            };
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
                                    let patched_action = match memory_actions::apply_before_memory_write_decision_to_action(&proposal.action, decision) {
                                        Ok(action) => action,
                                        Err(err) => {
                                            self.emit_nonfatal_hook_failures(
                                                session,
                                                &run_id,
                                                &phase.id,
                                                &phase_execution_id,
                                                hooks,
                                            )?;
                                            session.emitter.emit(
                                                HarnessEventType::HookFailed,
                                                HarnessEventPayload::Lifecycle {
                                                    message: err.clone(),
                                                    fields: hook_event_fields(
                                                        &before_memory_hook,
                                                        &phase.id,
                                                        before_memory_binding_count,
                                                        action_trace_fields(&proposal.action),
                                                    ),
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
                                                format!("before_memory_write hook returned invalid patch: {err}"),
                                                None,
                                            );
                                        }
                                    };
                                    if let Err(err) =
                                        validate_semantic_action(&patched_action, &effective_phase)
                                    {
                                        self.emit_nonfatal_hook_failures(
                                            session,
                                            &run_id,
                                            &phase.id,
                                            &phase_execution_id,
                                            hooks,
                                        )?;
                                        session.emitter.emit(
                                            HarnessEventType::HookFailed,
                                            HarnessEventPayload::Lifecycle {
                                                message: err.clone(),
                                                fields: hook_event_fields(
                                                    &before_memory_hook,
                                                    &phase.id,
                                                    before_memory_binding_count,
                                                    action_trace_fields(&proposal.action),
                                                ),
                                            },
                                            HarnessEventBuilder {
                                                run_id: Some(run_id.clone()),
                                                phase_execution_id: Some(
                                                    phase_execution_id.clone(),
                                                ),
                                                ..HarnessEventBuilder::default()
                                            },
                                        )?;
                                        return self.fail_phase(
                                            session,
                                            &phase.id,
                                            &phase_execution_id,
                                            format!("before_memory_write hook produced invalid action: {err}"),
                                            None,
                                        );
                                    }
                                    if before_memory_enabled {
                                        self.emit_nonfatal_hook_failures(
                                            session,
                                            &run_id,
                                            &phase.id,
                                            &phase_execution_id,
                                            hooks,
                                        )?;
                                        let mut fields = action_trace_fields(&patched_action);
                                        fields.insert("patched".into(), json!(patched));
                                        self.emit_hook_completed(
                                            session,
                                            HookEventContext {
                                                run_id: &run_id,
                                                phase_id: &phase.id,
                                                phase_execution_id: &phase_execution_id,
                                                hook: &before_memory_hook,
                                                binding_count: before_memory_binding_count,
                                            },
                                            fields,
                                        )?;
                                    }
                                    patched_action
                                }
                                Err(err) => {
                                    let is_rejection = err.is_rejection();
                                    self.emit_nonfatal_hook_failures(
                                        session,
                                        &run_id,
                                        &phase.id,
                                        &phase_execution_id,
                                        hooks,
                                    )?;
                                    session.emitter.emit(
                                        if is_rejection {
                                            HarnessEventType::HookRejected
                                        } else {
                                            HarnessEventType::HookFailed
                                        },
                                        HarnessEventPayload::Lifecycle {
                                            message: err.message.clone(),
                                            fields: hook_event_fields(
                                                &before_memory_hook,
                                                &phase.id,
                                                before_memory_binding_count,
                                                action_trace_fields(&proposal.action),
                                            ),
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
                                        format!(
                                            "before_memory_write hook {} action: {}",
                                            if is_rejection { "rejected" } else { "failed" },
                                            err.message
                                        ),
                                        None,
                                    );
                                }
                            }
                        }
                        _ => unreachable!(),
                    };
                    self.emit_service_lifecycle_events(session, Some(&run_id), service_events)?;
                    action
                } else {
                    proposal.action.clone()
                };
                let action_source = action_source(&action, &effective_phase);
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
                        phase_execution_id: Some(phase_execution_id.clone()),
                        ..HarnessEventBuilder::default()
                    },
                )?;
                let result = if matches!(action, SemanticAction::KnowledgeRequest { .. }) {
                    self.dispatch_knowledge(
                        session,
                        knowledge,
                        &action,
                        action_source.as_deref(),
                        &phase_execution_id,
                    )?
                } else if matches!(
                    action,
                    SemanticAction::MemoryRead { .. } | SemanticAction::MemoryWrite { .. }
                ) {
                    if matches!(action, SemanticAction::MemoryWrite { .. }) {
                        match self.relieve_memory_capacity_before_write(
                            session,
                            &effective_phase,
                            &mut state,
                            phase,
                            model,
                            hooks,
                            memory,
                            &action,
                        ) {
                            Ok(memory_lifecycle::MemoryCapacityReliefResult::Proceed)
                            | Ok(memory_lifecycle::MemoryCapacityReliefResult::StillAtCapacity) => {
                            }
                            Err(err) => {
                                return self.fail_phase(
                                    session,
                                    &phase.id,
                                    &phase_execution_id,
                                    err.to_string(),
                                    None,
                                );
                            }
                        }
                    }
                    self.dispatch_memory(
                        session,
                        &effective_phase,
                        &action,
                        memory,
                        embedding_provider,
                        action_source.as_deref(),
                        &phase_execution_id,
                    )?
                } else {
                    self.dispatch_with_retry(
                        session,
                        dispatcher,
                        &action,
                        action_source.as_deref(),
                        &phase_execution_id,
                    )?
                };
                self.emit_service_lifecycle_events(session, Some(&run_id), service_events)?;
                let result = if matches!(action, SemanticAction::KnowledgeRequest { .. })
                    && result.ok
                    && result.output.get("package").is_some()
                {
                    self.apply_after_knowledge_retrieval_hook(
                        session,
                        hooks,
                        &phase.id,
                        &phase_execution_id,
                        &run_id,
                        result,
                    )?
                } else {
                    result
                };
                if !result.ok {
                    if matches!(action, SemanticAction::MemoryWrite { .. })
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
                                phase_execution_id: Some(phase_execution_id.clone()),
                                ..HarnessEventBuilder::default()
                            },
                        )?;
                        repair_feedback = Some(error);
                        self.request_repair(
                            session,
                            &mut state,
                            &phase_execution_id,
                            repair_feedback.clone(),
                        )?;
                        continue;
                    }
                    let error = result.error.unwrap_or_else(|| "action failed".to_string());
                    let terminal_status = result.terminal_status;
                    self.active_run_mut(session)?
                        .action_summaries
                        .push(ActionReportSummary {
                            action_kind: action.kind().into(),
                            identity: action.identity(),
                            status: "failed".into(),
                            error: Some(error.clone()),
                        });
                    if action.is_tool_call() {
                        self.active_run_mut(session)?.operation_summaries.push(
                            OperationReportSummary {
                                operation_kind: action.kind().into(),
                                identity: action.identity(),
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
                if matches!(action, SemanticAction::KnowledgeRequest { .. })
                    && result
                        .output
                        .get("ok")
                        .and_then(Value::as_bool)
                        .is_some_and(|ok| !ok)
                {
                    self.active_run_mut(session)?
                        .action_summaries
                        .push(ActionReportSummary {
                            action_kind: action.kind().into(),
                            identity: action.identity(),
                            status: "failed".into(),
                            error: result
                                .output
                                .get("error")
                                .and_then(|error| error.get("message"))
                                .and_then(Value::as_str)
                                .map(str::to_string),
                        });
                    state.transcript.push(TranscriptEntry {
                        kind: TranscriptEntryKind::ActionResult,
                        content: action_result_transcript_content(&action, result.output),
                        action_succeeded: Some(false),
                    });
                    continue;
                }
                if matches!(
                    action,
                    SemanticAction::MemoryRead { .. } | SemanticAction::MemoryWrite { .. }
                ) && result
                    .output
                    .get("ok")
                    .and_then(Value::as_bool)
                    .is_some_and(|ok| !ok)
                {
                    self.active_run_mut(session)?
                        .action_summaries
                        .push(ActionReportSummary {
                            action_kind: action.kind().into(),
                            identity: action.identity(),
                            status: "failed".into(),
                            error: result
                                .output
                                .get("error")
                                .and_then(|error| error.get("message"))
                                .and_then(Value::as_str)
                                .map(str::to_string),
                        });
                    state.transcript.push(TranscriptEntry {
                        kind: TranscriptEntryKind::ActionResult,
                        content: action_result_transcript_content(&action, result.output),
                        action_succeeded: Some(false),
                    });
                    continue;
                }
                if let SemanticAction::MemoryWrite { package, space, .. } = &action {
                    self.evaluate_memory_lifecycle_after_write(
                        session,
                        &effective_phase,
                        &mut state,
                        phase,
                        model,
                        hooks,
                        memory,
                        memory_lifecycle::MemoryChangeContext {
                            package: package.clone(),
                            space: space.clone(),
                            capacity_relief: false,
                        },
                    )?;
                }
                state.transcript.push(TranscriptEntry {
                    kind: TranscriptEntryKind::ActionResult,
                    content: action_result_transcript_content(&action, result.output),
                    action_succeeded: Some(true),
                });
                self.active_run_mut(session)?
                    .action_summaries
                    .push(ActionReportSummary {
                        action_kind: action.kind().into(),
                        identity: action.identity(),
                        status: "completed".into(),
                        error: None,
                    });
                if action.is_tool_call() {
                    self.active_run_mut(session)?.operation_summaries.push(
                        OperationReportSummary {
                            operation_kind: action.kind().into(),
                            identity: action.identity(),
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
        let repair_attempt = state.structured_repairs;
        self.record_repair(
            session,
            state,
            phase_execution_id,
            feedback,
            repair_attempt,
            "structured_output",
        )
    }

    fn request_tool_call_repair(
        &self,
        session: &mut HarnessSession,
        state: &mut PhaseExecutionState,
        phase_execution_id: &str,
        feedback: Option<String>,
    ) -> Result<()> {
        if state.tool_call_repairs >= self.options.runtime_limits.max_tool_call_repairs {
            return Err(anyhow!("tool call repair limit exhausted"));
        }
        state.tool_call_repairs += 1;
        let repair_attempt = state.tool_call_repairs;
        self.record_repair(
            session,
            state,
            phase_execution_id,
            feedback,
            repair_attempt,
            "tool_call",
        )
    }

    fn record_repair(
        &self,
        session: &mut HarnessSession,
        state: &mut PhaseExecutionState,
        phase_execution_id: &str,
        feedback: Option<String>,
        repair_attempt: u64,
        repair_kind: &str,
    ) -> Result<()> {
        self.active_run_mut(session)?.repair_count += 1;
        let message = feedback.unwrap_or_else(|| "Repair requested.".to_string());
        state.transcript.push(TranscriptEntry {
            kind: TranscriptEntryKind::RepairFeedback,
            content: json!(message),
            action_succeeded: None,
        });
        let run_id = self.active_run(session)?.run_id().to_string();
        session.emitter.emit(
            HarnessEventType::ModelRepairRequested,
            HarnessEventPayload::Lifecycle {
                message,
                fields: BTreeMap::from([
                    ("repair_attempt".into(), json!(repair_attempt)),
                    ("repair_kind".into(), json!(repair_kind)),
                ]),
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
        action_source: Option<&str>,
        phase_execution_id: &str,
    ) -> Result<ActionDispatchResult> {
        let mut attempts = 0;
        loop {
            attempts += 1;
            let run_id = self.active_run(session)?.run_id().to_string();
            if let Some(event_type) = action_request_event_type(action) {
                let mut fields = action_trace_fields(action);
                fields.insert("attempt".into(), json!(attempts));
                if let Some(source) = action_source {
                    fields.insert("source".into(), json!(source));
                }
                session.emitter.emit(
                    event_type,
                    HarnessEventPayload::Action {
                        action_kind: action.kind().into(),
                        identity: action.identity(),
                        status: "requested".into(),
                        fields,
                    },
                    HarnessEventBuilder {
                        run_id: Some(run_id.clone()),
                        phase_execution_id: Some(phase_execution_id.to_string()),
                        ..HarnessEventBuilder::default()
                    },
                )?;
            }
            let result = dispatcher.dispatch(action);
            let event_type = action_dispatch_event_type(action, result.ok);
            session.emitter.emit(
                event_type,
                HarnessEventPayload::Action {
                    action_kind: action.kind().into(),
                    identity: action.identity(),
                    status: if result.ok { "completed" } else { "failed" }.into(),
                    fields: action_result_trace_fields(action, &result, attempts, action_source),
                },
                HarnessEventBuilder {
                    run_id: Some(run_id.clone()),
                    phase_execution_id: Some(phase_execution_id.to_string()),
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
            if !should_retry_tool_failure(&result) {
                return Ok(result);
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
            let mut fields = action_trace_fields(action);
            fields.insert("attempt".into(), json!(attempts + 1));
            if let Some(source) = action_source {
                fields.insert("source".into(), json!(source));
            }
            session.emitter.emit(
                HarnessEventType::ToolRetrying,
                HarnessEventPayload::Action {
                    action_kind: action.kind().into(),
                    identity: action.identity(),
                    status: "retrying".into(),
                    fields,
                },
                HarnessEventBuilder {
                    run_id: Some(run_id),
                    phase_execution_id: Some(phase_execution_id.to_string()),
                    ..HarnessEventBuilder::default()
                },
            )?;
        }
    }
}

enum CheckpointFlow {
    ContinueTo(String),
    Pending(PendingApprovalState),
}
