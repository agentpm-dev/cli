use super::super::{
    HarnessArgs, activate_custom_knowledge_runtime_for_plan,
    activate_custom_memory_runtime_for_plan, activate_mcp_import_runtime_for_plan,
    apply_custom_knowledge_activation_to_runtime, apply_custom_memory_activation_to_runtime,
    apply_mcp_import_activation_to_runtime, approval_controller_from_plan,
    embedding_provider_for_plan, emit_mcp_import_activation_events,
    harness_engine_options_from_plan, knowledge_runtime_for_headless_plan, load_plan_loop,
    model_runtime_from_plan, model_selection, runtime_snapshot_from_plan,
    validate_model_capabilities,
};
use crate::harness_config::{
    HarnessConfigSource, HarnessConfigSourceKind, HarnessModelConfig, HarnessTraceContent,
};
use crate::harness_engine::{
    HarnessEngine, HarnessRunResult, HarnessRuntimeServices, HarnessSession,
    MemoryOperationInvocationResult, RuntimeTerminalResult, RuntimeTerminalStatus,
};
use crate::harness_observability::{
    HarnessEventBuilder, HarnessEventEnvelope, HarnessEventPayload, HarnessEventSink,
    HarnessEventType, HarnessTerminalStatus, JsonlTraceSink, RunOutputPaths, RunReport, RunUsage,
    SessionUsage, apply_content_policy,
};
use crate::harness_plan::{
    CapabilityState, HarnessBootstrapOptions, HarnessExecutionSurface, HarnessPlanProgress,
    HarnessPlanProgressStage, PreflightDiagnosticSeverity, ResolvedHarnessPlan,
    resolve_harness_plan_with_progress,
};
use crate::harness_runtime::{
    AgentPmActionDispatcher, ConfiguredHookRuntime, HookRuntime, ModelRuntime,
    ServiceLifecycleEvents,
};
use crate::prelude::*;
use anyhow::{anyhow, bail};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::{
    any::Any,
    collections::{BTreeMap, VecDeque},
    panic::{self, AssertUnwindSafe},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
};

type BootstrapResult = Result<ResolvedHarnessPlan>;

pub(super) enum BootstrapMessage {
    Progress(TuiBootstrapProgress),
    Ready(Box<BootstrapResult>),
}

pub(super) enum TuiRunMessage {
    Progress(TuiRunProgress),
    Finished(Box<TuiRunWorkerResult>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TuiRunProgress {
    pub(super) message: String,
}

pub(super) struct TuiRunWorkerResult {
    pub(super) controller: TuiSessionController,
    pub(super) error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TuiBootstrapStage {
    Bootstrap,
    Preflight,
    Runtime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TuiBootstrapProgress {
    pub(super) stage: TuiBootstrapStage,
    pub(super) message: String,
}

impl TuiBootstrapProgress {
    pub(super) fn new(stage: TuiBootstrapStage, message: impl Into<String>) -> Self {
        Self {
            stage,
            message: message.into(),
        }
    }

    pub(super) fn from_plan_progress(progress: HarnessPlanProgress) -> Self {
        let stage = match progress.stage {
            HarnessPlanProgressStage::Workspace
            | HarnessPlanProgressStage::Config
            | HarnessPlanProgressStage::Lockfile => TuiBootstrapStage::Bootstrap,
            HarnessPlanProgressStage::PackageGraph
            | HarnessPlanProgressStage::AgentSelection
            | HarnessPlanProgressStage::Validation
            | HarnessPlanProgressStage::RuntimeSummary
            | HarnessPlanProgressStage::Complete => TuiBootstrapStage::Preflight,
        };
        Self::new(stage, progress.message)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct TuiSessionSnapshot {
    pub(super) session_id: String,
    pub(super) workspace: TuiWorkspaceReadiness,
    pub(super) run: TuiRunSnapshot,
    pub(super) usage: SessionUsage,
    pub(super) trace: TuiTraceSnapshot,
    pub(super) reports: TuiReportSnapshot,
    pub(super) services: TuiServiceSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct TuiWorkspaceReadiness {
    pub(super) categories: Vec<TuiReadinessCategory>,
    pub(super) diagnostics: Vec<TuiDiagnosticSummary>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct TuiReadinessCategory {
    pub(super) label: String,
    pub(super) state: CapabilityState,
    pub(super) summary: String,
    pub(super) source: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct TuiDiagnosticSummary {
    pub(super) severity: PreflightDiagnosticSeverity,
    pub(super) code: String,
    pub(super) message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct TuiRunSnapshot {
    pub(super) status: TuiRunStatus,
    pub(super) run_id: Option<String>,
    pub(super) phase_id: Option<String>,
    pub(super) started_at: Option<DateTime<Utc>>,
    pub(super) phase_objective: Option<String>,
    pub(super) terminal_status: Option<HarnessTerminalStatus>,
    pub(super) latest_output: Option<Value>,
    pub(super) transcript: Vec<TuiRunTranscriptItem>,
    pub(super) usage: RunUsage,
    pub(super) approval: Option<TuiApprovalSnapshot>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct TuiRunTranscriptItem {
    pub(super) phase_label: Option<String>,
    pub(super) kind: TuiRunTranscriptKind,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum TuiRunTranscriptKind {
    Assistant {
        content: String,
    },
    Repair {
        message: String,
    },
    PhaseResult {
        outcome: Option<String>,
        output: Option<Value>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TuiRunStatus {
    Idle,
    Active,
    PendingApproval,
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TuiApprovalSnapshot {
    pub(super) checkpoint_id: String,
    pub(super) before_phase: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct TuiTraceSnapshot {
    pub(super) events: Vec<HarnessEventEnvelope>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct TuiReportSnapshot {
    pub(super) current_report_path: Option<PathBuf>,
    pub(super) current_trace_path: Option<PathBuf>,
    pub(super) current_report: Option<RunReport>,
    pub(super) current_trace_events: Vec<HarnessEventEnvelope>,
    pub(super) current_report_error: Option<String>,
    pub(super) current_trace_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TuiServiceSnapshot {
    pub(super) mcp_exports: Vec<TuiServiceSurfaceSnapshot>,
    pub(super) mcp_imports: Vec<TuiServiceSurfaceSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TuiServiceSurfaceSnapshot {
    pub(super) identity: String,
    pub(super) state: String,
    pub(super) detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(super) enum TuiApprovalDecision {
    Approve,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(super) struct TuiControlAck {
    pub(super) accepted: bool,
    pub(super) message: String,
}

pub(super) fn spawn_bootstrap_worker(
    args: HarnessArgs,
    workspace_root: PathBuf,
    model_override: Option<HarnessModelConfig>,
) -> Receiver<BootstrapMessage> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(BootstrapMessage::Progress(TuiBootstrapProgress::new(
            TuiBootstrapStage::Bootstrap,
            "Resolving Harness workspace.",
        )));
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            resolve_harness_plan_with_progress(
                &workspace_root,
                &HarnessBootstrapOptions {
                    agent_selector: args.agent.clone(),
                    config_path: args.config.clone(),
                    state_dir_override: args.state_dir.clone(),
                    model_override: model_override.clone(),
                    model_override_source: model_override
                        .as_ref()
                        .map(|_| HarnessConfigSource::interactive_override()),
                    runtime_scopes: args.scopes.iter().cloned().collect(),
                    surface: HarnessExecutionSurface::Tui,
                },
                |progress| {
                    let _ = sender.send(BootstrapMessage::Progress(
                        TuiBootstrapProgress::from_plan_progress(progress),
                    ));
                },
            )
        }))
        .unwrap_or_else(|payload| {
            Err(anyhow!(
                "bootstrap worker panicked: {}",
                panic_payload_message(payload)
            ))
        });
        let _ = sender.send(BootstrapMessage::Ready(Box::new(result)));
    });
    receiver
}

pub(super) fn spawn_tui_run_worker(
    controller: TuiSessionController,
    run_id: String,
    input: String,
) -> Receiver<TuiRunMessage> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            execute_tui_run_worker(controller, run_id, input, &sender)
        }))
        .unwrap_or_else(|payload| TuiRunWorkerResult {
            controller: TuiSessionController::failed_placeholder(format!(
                "run worker panicked: {}",
                panic_payload_message(payload)
            )),
            error: Some("run worker panicked before controller recovery was possible".into()),
        });
        let _ = sender.send(TuiRunMessage::Finished(Box::new(result)));
    });
    receiver
}

fn execute_tui_run_worker(
    mut controller: TuiSessionController,
    run_id: String,
    input: String,
    sender: &Sender<TuiRunMessage>,
) -> TuiRunWorkerResult {
    let error = match execute_tui_run_worker_inner(&mut controller, run_id, input, sender) {
        Ok(()) => None,
        Err(err) => {
            controller.refresh_snapshot();
            Some(format!("{err:#}"))
        }
    };
    TuiRunWorkerResult { controller, error }
}

fn execute_tui_run_worker_inner(
    controller: &mut TuiSessionController,
    run_id: String,
    input: String,
    sender: &Sender<TuiRunMessage>,
) -> Result<()> {
    let plan = controller.plan().clone();
    send_run_progress(sender, "Preparing model runtime.");
    let selection = model_selection(&plan)?;
    let mut service_events = ServiceLifecycleEvents::new();
    let mut model = model_runtime_from_plan(&plan, selection, None, Some(&service_events))?;
    validate_model_capabilities(model.as_ref())?;

    send_run_progress(
        sender,
        "Activating Knowledge, Memory, and MCP import services.",
    );
    let mut runtime = runtime_snapshot_from_plan(&plan);
    let custom_knowledge =
        activate_custom_knowledge_runtime_for_plan(&plan, &runtime, None, Some(&service_events));
    apply_custom_knowledge_activation_to_runtime(&mut runtime, &custom_knowledge);
    let custom_memory =
        activate_custom_memory_runtime_for_plan(&plan, &runtime, None, Some(&service_events));
    apply_custom_memory_activation_to_runtime(&mut runtime, &custom_memory);
    let mcp_import_activation = activate_mcp_import_runtime_for_plan(&plan);
    apply_mcp_import_activation_to_runtime(&mut runtime, &mcp_import_activation);
    let mcp_import_runtime = Arc::new(Mutex::new(mcp_import_activation.runtime));
    let mut dispatcher = AgentPmActionDispatcher::from_runtime(&runtime)?
        .with_mcp_import_runtime(Arc::clone(&mcp_import_runtime))
        .with_cancellation_token(Arc::clone(&controller.cancellation_requested));
    let mut knowledge = knowledge_runtime_for_headless_plan(
        &plan,
        &runtime,
        custom_knowledge.runtime,
        Some(&service_events),
    );
    let memory_embedding_provider = embedding_provider_for_plan(&plan, None, Some(&service_events));
    let mut approvals = approval_controller_from_plan(&plan, None, Some(&service_events))?;
    let mut hooks = ConfiguredHookRuntime::from_config(
        &plan.workspace_root,
        &plan.config.config.hooks.bindings,
        &plan.config.config.hooks.implementations,
        Some(service_events.emitter()),
    )?;

    let output_paths = RunOutputPaths::resolve(&plan.state_dir, &run_id, None)?;
    if plan.config.config.trace.enabled {
        controller
            .session
            .emitter
            .add_sink(Box::new(JsonlTraceSink::create(
                &output_paths.events_path,
                plan.config.config.trace.clone(),
            )?));
    }
    controller.session.runtime_snapshot = runtime;
    let mcp_import_snapshots = controller.session.runtime_snapshot.mcp_imports.clone();
    emit_mcp_import_activation_events(&mut controller.session, &mcp_import_snapshots)?;
    controller.refresh_snapshot();

    send_run_progress(sender, "Executing Run.");
    let mut services = HarnessRuntimeServices {
        model: model.as_mut(),
        dispatcher: &mut dispatcher,
        knowledge: knowledge.as_mut(),
        memory: custom_memory.runtime,
        embedding_provider: memory_embedding_provider,
        approvals: approvals.as_mut(),
        hooks: &mut hooks,
        service_events: Some(&mut service_events),
    };
    let result = controller.start_run_with_services(run_id, input, &output_paths, &mut services)?;
    if let HarnessRunResult::Terminal(terminal) = result {
        let mut terminal = *terminal;
        if plan.config.config.trace.enabled {
            terminal.report.trace_path = Some(output_paths.events_path.display().to_string());
        }
        terminal
            .report
            .write_pretty(&output_paths.report_path, &plan.config.config.trace.content)?;
        controller.apply_terminal_result(&terminal, &output_paths);
        controller.session.emitter.flush()?;
    }
    Ok(())
}

fn send_run_progress(sender: &Sender<TuiRunMessage>, message: impl Into<String>) {
    let _ = sender.send(TuiRunMessage::Progress(TuiRunProgress {
        message: message.into(),
    }));
}

pub(super) fn panic_payload_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).into()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".into()
    }
}

#[allow(dead_code)]
pub(super) struct TuiSessionController {
    pub(super) plan: Box<ResolvedHarnessPlan>,
    pub(super) session: HarnessSession,
    pub(super) engine: Option<HarnessEngine>,
    pub(super) events: TuiEventBuffer,
    pub(super) cancellation_requested: Arc<AtomicBool>,
    pub(super) snapshot: TuiSessionSnapshot,
}

#[allow(dead_code)]
impl TuiSessionController {
    pub(super) fn new(plan: ResolvedHarnessPlan) -> Result<Self> {
        let runtime = runtime_snapshot_from_plan(&plan);
        let mut session = HarnessSession::with_runtime_snapshot(runtime);
        let events = TuiEventBuffer::new(plan.config.config.trace.content.clone(), 256);
        session.emitter.add_sink(Box::new(events.sink()));
        let engine = if plan.loop_package.is_some() {
            let loop_manifest = load_plan_loop(&plan)?;
            Some(HarnessEngine::new(
                loop_manifest,
                harness_engine_options_from_plan(&plan),
            ))
        } else {
            None
        };
        session.emitter.emit(
            HarnessEventType::PreflightCompleted,
            HarnessEventPayload::Preflight {
                status: plan.report.status,
                fatal_count: plan
                    .report
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| {
                        matches!(diagnostic.severity, PreflightDiagnosticSeverity::Fatal)
                    })
                    .count(),
                warning_count: plan
                    .report
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| {
                        matches!(diagnostic.severity, PreflightDiagnosticSeverity::Warning)
                    })
                    .count(),
                suppressed_count: plan
                    .report
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| {
                        matches!(diagnostic.severity, PreflightDiagnosticSeverity::Suppressed)
                    })
                    .count(),
                pending_count: plan
                    .report
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| {
                        matches!(diagnostic.severity, PreflightDiagnosticSeverity::Pending)
                    })
                    .count(),
            },
            HarnessEventBuilder::default(),
        )?;
        let snapshot = build_session_snapshot(&plan, &session, &events, None, None);
        Ok(Self {
            plan: Box::new(plan),
            session,
            engine,
            events,
            cancellation_requested: Arc::new(AtomicBool::new(false)),
            snapshot,
        })
    }

    fn failed_placeholder(message: String) -> Self {
        let plan = ResolvedHarnessPlan {
            workspace_root: PathBuf::new(),
            lock_path: PathBuf::new(),
            state_dir: PathBuf::new(),
            config: crate::harness_config::ResolvedHarnessConfig {
                workspace_root: PathBuf::new(),
                config_path: None,
                config: crate::harness_config::HarnessConfig::default(),
                state_dir: PathBuf::new(),
                state_dir_source: HarnessConfigSource {
                    kind: HarnessConfigSourceKind::HarnessDefault,
                    path: None,
                },
                model_source: HarnessConfigSource {
                    kind: HarnessConfigSourceKind::HarnessDefault,
                    path: None,
                },
            },
            selected_agent: None,
            loop_package: None,
            package_graph: BTreeMap::new(),
            runtime_scopes: BTreeMap::new(),
            consumer_context: crate::harness_plan::ConsumerContextReadiness {
                state: CapabilityState::NotConfigured,
                file: None,
                path: None,
                byte_size: None,
                approximate_tokens: None,
                sha256: None,
            },
            profile_bindings: Default::default(),
            profiles: BTreeMap::new(),
            capabilities: Vec::new(),
            report: crate::harness_plan::PreflightReport {
                status: crate::harness_plan::PreflightStatus::Failed,
                diagnostics: vec![crate::harness_plan::PreflightDiagnostic {
                    severity: PreflightDiagnosticSeverity::Fatal,
                    code: "tui_run_worker_failed".into(),
                    message,
                    path: None,
                }],
                mcp_exports: crate::harness_plan::PreflightMcpExports {
                    enabled: false,
                    host: "127.0.0.1".into(),
                    restart: crate::harness_config::HarnessRestartPolicy::default(),
                    surfaces: Vec::new(),
                },
                mcp_imports: crate::harness_plan::PreflightMcpImports {
                    enabled: false,
                    servers: Vec::new(),
                },
            },
        };
        Self::new(plan).expect("placeholder TUI controller should build")
    }

    pub(super) fn plan(&self) -> &ResolvedHarnessPlan {
        &self.plan
    }

    pub(super) fn snapshot(&self) -> &TuiSessionSnapshot {
        &self.snapshot
    }

    pub(super) fn refresh_snapshot(&mut self) {
        let reports = self.snapshot.reports.clone();
        self.snapshot = build_session_snapshot(
            &self.plan,
            &self.session,
            &self.events,
            reports.current_report_path.clone(),
            reports.current_trace_path.clone(),
        );
        self.snapshot.reports = reports;
    }

    pub(super) fn start_run_with_services(
        &mut self,
        run_id: String,
        input: impl Into<String>,
        output_paths: &RunOutputPaths,
        services: &mut HarnessRuntimeServices<'_>,
    ) -> Result<HarnessRunResult> {
        if matches!(
            self.snapshot.run.status,
            TuiRunStatus::Active | TuiRunStatus::PendingApproval
        ) {
            bail!("TUI Session already has an active Run");
        }
        self.cancellation_requested.store(false, Ordering::SeqCst);
        let result = self
            .engine
            .as_mut()
            .context("TUI Run control requires a resolved Loop package")?
            .execute_run_with_id(&mut self.session, run_id, input, services)?;
        self.apply_run_result(&result, output_paths);
        Ok(result)
    }

    pub(super) fn request_cancel(&mut self) -> Result<TuiControlAck> {
        self.cancellation_requested.store(true, Ordering::SeqCst);
        self.session.emitter.emit(
            HarnessEventType::CancellationRequested,
            HarnessEventPayload::Lifecycle {
                message: "TUI Run cancellation requested.".into(),
                fields: Default::default(),
            },
            HarnessEventBuilder::default(),
        )?;
        self.refresh_snapshot();
        Ok(TuiControlAck {
            accepted: true,
            message: "Cancellation requested.".into(),
        })
    }

    pub(super) fn record_approval_decision(
        &mut self,
        decision: TuiApprovalDecision,
    ) -> Result<TuiControlAck> {
        let Some(run) = self.session.active_run() else {
            bail!("TUI approval control requires an active Run");
        };
        let Some(approval) = run.pending_approval() else {
            bail!("TUI approval control requires a pending approval");
        };
        bail!(
            "TUI approval decision {:?} for checkpoint `{}` is not available until approval controls are routed through the Engine ApprovalController",
            decision,
            approval.checkpoint_id
        )
    }

    pub(super) fn invoke_memory_operation(
        &mut self,
        package: &str,
        operation: &str,
        current_resolved_scope: BTreeMap<String, String>,
        model: &mut dyn ModelRuntime,
        hooks: &mut dyn HookRuntime,
    ) -> Result<MemoryOperationInvocationResult> {
        let result = self
            .engine
            .as_mut()
            .context("TUI Memory operation control requires a resolved Loop package")?
            .invoke_memory_operation(
                &mut self.session,
                package,
                operation,
                current_resolved_scope,
                model,
                hooks,
            )?;
        self.refresh_snapshot();
        Ok(result)
    }

    fn apply_run_result(&mut self, result: &HarnessRunResult, output_paths: &RunOutputPaths) {
        match result {
            HarnessRunResult::Terminal(terminal) => {
                self.apply_terminal_result(terminal, output_paths);
            }
            HarnessRunResult::PendingApproval { run_id, checkpoint } => {
                self.snapshot.run.status = TuiRunStatus::PendingApproval;
                self.snapshot.run.run_id = Some(run_id.clone());
                self.snapshot.run.approval = Some(TuiApprovalSnapshot {
                    checkpoint_id: checkpoint.checkpoint_id.clone(),
                    before_phase: checkpoint.before_phase.clone(),
                });
                self.snapshot.trace.events = self.events.events();
            }
        }
    }

    pub(super) fn apply_terminal_result(
        &mut self,
        terminal: &RuntimeTerminalResult,
        output_paths: &RunOutputPaths,
    ) {
        self.snapshot.run.status = TuiRunStatus::Terminal;
        self.snapshot.run.run_id = Some(terminal.report.run_id.clone());
        self.snapshot.run.started_at = Some(terminal.report.started_at);
        self.snapshot.run.phase_id = terminal
            .report
            .phase_summaries
            .last()
            .map(|phase| phase.phase_id.clone());
        self.snapshot.run.phase_objective =
            run_phase_objective(&self.plan, self.snapshot.run.phase_id.as_deref());
        self.snapshot.run.terminal_status = Some(terminal.status);
        self.snapshot.run.latest_output = terminal
            .output
            .clone()
            .or_else(|| self.events.latest_phase_output());
        self.snapshot.run.transcript = self.events.transcript();
        self.snapshot.run.usage = terminal.report.usage.clone();
        self.snapshot.reports.current_report_path = Some(output_paths.report_path.clone());
        self.snapshot.reports.current_trace_path = terminal
            .report
            .trace_path
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| Some(output_paths.events_path.clone()));
        self.snapshot.reports.current_report = Some(terminal.report.clone());
        self.snapshot.reports.current_report_error = None;
        self.snapshot.reports.current_trace_events = self.events.events();
        self.snapshot.reports.current_trace_error = None;
        self.snapshot.usage = self.session.usage.clone();
        self.snapshot.trace.events = self.events.events();
    }
}

#[derive(Clone)]
pub(super) struct TuiEventBuffer {
    pub(super) events: Arc<Mutex<VecDeque<HarnessEventEnvelope>>>,
    latest_phase_output: Arc<Mutex<Option<Value>>>,
    transcript: Arc<Mutex<VecDeque<TuiRunTranscriptItem>>>,
    phase_labels: Arc<Mutex<BTreeMap<String, String>>>,
    current_phase_id: Arc<Mutex<Option<String>>>,
    live_run_usage: Arc<Mutex<RunUsage>>,
    pub(super) policy: HarnessTraceContent,
    pub(super) capacity: usize,
}

impl TuiEventBuffer {
    pub(super) fn new(policy: HarnessTraceContent, capacity: usize) -> Self {
        Self {
            events: Arc::new(Mutex::new(VecDeque::new())),
            latest_phase_output: Arc::new(Mutex::new(None)),
            transcript: Arc::new(Mutex::new(VecDeque::new())),
            phase_labels: Arc::new(Mutex::new(BTreeMap::new())),
            current_phase_id: Arc::new(Mutex::new(None)),
            live_run_usage: Arc::new(Mutex::new(RunUsage::default())),
            policy,
            capacity,
        }
    }

    pub(super) fn sink(&self) -> TuiEventSink {
        TuiEventSink {
            events: Arc::clone(&self.events),
            latest_phase_output: Arc::clone(&self.latest_phase_output),
            transcript: Arc::clone(&self.transcript),
            phase_labels: Arc::clone(&self.phase_labels),
            current_phase_id: Arc::clone(&self.current_phase_id),
            live_run_usage: Arc::clone(&self.live_run_usage),
            policy: self.policy.clone(),
            capacity: self.capacity,
        }
    }

    pub(super) fn events(&self) -> Vec<HarnessEventEnvelope> {
        self.events
            .lock()
            .expect("TUI event buffer poisoned")
            .iter()
            .cloned()
            .collect()
    }

    pub(super) fn latest_phase_output(&self) -> Option<Value> {
        self.latest_phase_output
            .lock()
            .expect("TUI latest phase output poisoned")
            .clone()
    }

    pub(super) fn transcript(&self) -> Vec<TuiRunTranscriptItem> {
        self.transcript
            .lock()
            .expect("TUI run transcript poisoned")
            .iter()
            .cloned()
            .collect()
    }

    pub(super) fn current_phase_id(&self) -> Option<String> {
        self.current_phase_id
            .lock()
            .expect("TUI current phase id poisoned")
            .clone()
    }

    pub(super) fn live_run_usage(&self) -> RunUsage {
        self.live_run_usage
            .lock()
            .expect("TUI live run usage poisoned")
            .clone()
    }

    pub(super) fn reset_run_output(&self) {
        *self
            .latest_phase_output
            .lock()
            .expect("TUI latest phase output poisoned") = None;
        *self
            .current_phase_id
            .lock()
            .expect("TUI current phase id poisoned") = None;
        *self
            .live_run_usage
            .lock()
            .expect("TUI live run usage poisoned") = RunUsage::default();
        self.transcript
            .lock()
            .expect("TUI run transcript poisoned")
            .clear();
        self.phase_labels
            .lock()
            .expect("TUI phase labels poisoned")
            .clear();
    }
}

pub(super) struct TuiEventSink {
    events: Arc<Mutex<VecDeque<HarnessEventEnvelope>>>,
    latest_phase_output: Arc<Mutex<Option<Value>>>,
    transcript: Arc<Mutex<VecDeque<TuiRunTranscriptItem>>>,
    phase_labels: Arc<Mutex<BTreeMap<String, String>>>,
    current_phase_id: Arc<Mutex<Option<String>>>,
    live_run_usage: Arc<Mutex<RunUsage>>,
    policy: HarnessTraceContent,
    capacity: usize,
}

impl HarnessEventSink for TuiEventSink {
    fn record(&mut self, event: &HarnessEventEnvelope) -> Result<()> {
        if matches!(
            event.event_type,
            HarnessEventType::PhaseEnterRequested | HarnessEventType::PhaseStarted
        ) && let Some(phase_id) = event_phase_id(event)
        {
            *self
                .current_phase_id
                .lock()
                .expect("TUI current phase id poisoned") = Some(phase_id);
        }
        if let (Some(phase_execution_id), Some(phase_id)) =
            (event.phase_execution_id.as_ref(), event_phase_id(event))
        {
            self.phase_labels
                .lock()
                .expect("TUI phase labels poisoned")
                .insert(phase_execution_id.clone(), phase_id);
        }
        if event.event_type == HarnessEventType::PhaseResultReady
            && let HarnessEventPayload::Phase {
                outcome, output, ..
            } = &event.payload
        {
            if let Some(output) = output {
                *self
                    .latest_phase_output
                    .lock()
                    .expect("TUI latest phase output poisoned") = Some(output.clone());
            }
            self.push_transcript(
                event,
                TuiRunTranscriptKind::PhaseResult {
                    outcome: outcome.clone(),
                    output: output.clone(),
                },
            );
        }
        if event.event_type == HarnessEventType::ModelRequestCompleted
            && let HarnessEventPayload::Lifecycle { fields, .. } = &event.payload
            && let Some(content) = fields
                .get("assistant_content")
                .and_then(serde_json::Value::as_str)
            && !content.trim().is_empty()
        {
            self.push_transcript(
                event,
                TuiRunTranscriptKind::Assistant {
                    content: content.to_string(),
                },
            );
        }
        if event.event_type == HarnessEventType::ModelRequestCompleted {
            self.live_run_usage
                .lock()
                .expect("TUI live run usage poisoned")
                .model_calls += 1;
        }
        if event.event_type == HarnessEventType::ModelRepairRequested
            && let HarnessEventPayload::Lifecycle { message, .. } = &event.payload
        {
            self.push_transcript(
                event,
                TuiRunTranscriptKind::Repair {
                    message: message.clone(),
                },
            );
        }
        if event.event_type == HarnessEventType::SessionUsageUpdated
            && let HarnessEventPayload::Usage { run_usage, .. } = &event.payload
        {
            *self
                .live_run_usage
                .lock()
                .expect("TUI live run usage poisoned") = (**run_usage).clone();
        }
        let event = tui_safe_event_for_policy(event, &self.policy)?;
        let mut events = self.events.lock().expect("TUI event buffer poisoned");
        if events.len() == self.capacity {
            events.pop_front();
        }
        events.push_back(event);
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl TuiEventSink {
    fn push_transcript(&mut self, event: &HarnessEventEnvelope, kind: TuiRunTranscriptKind) {
        let phase_label = self.phase_label_for_event(event);
        let mut transcript = self.transcript.lock().expect("TUI run transcript poisoned");
        if transcript.len() == self.capacity {
            transcript.pop_front();
        }
        transcript.push_back(TuiRunTranscriptItem { phase_label, kind });
    }

    fn phase_label_for_event(&self, event: &HarnessEventEnvelope) -> Option<String> {
        event_phase_id(event).or_else(|| {
            event
                .phase_execution_id
                .as_ref()
                .and_then(|phase_execution_id| {
                    self.phase_labels
                        .lock()
                        .expect("TUI phase labels poisoned")
                        .get(phase_execution_id)
                        .cloned()
                        .or_else(|| Some(phase_execution_id.clone()))
                })
        })
    }
}

fn event_phase_id(event: &HarnessEventEnvelope) -> Option<String> {
    match &event.payload {
        HarnessEventPayload::Phase { phase_id, .. } => Some(phase_id.clone()),
        HarnessEventPayload::Lifecycle { fields, .. } => fields
            .get("phase_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        _ => None,
    }
}

fn tui_safe_event_for_policy(
    event: &HarnessEventEnvelope,
    policy: &HarnessTraceContent,
) -> Result<HarnessEventEnvelope> {
    match apply_content_policy(event, policy) {
        Ok(event) => Ok(event),
        Err(_err) if *policy == HarnessTraceContent::None => {
            let mut event = event.clone();
            event.payload = HarnessEventPayload::Empty;
            Ok(event)
        }
        Err(err) => Err(err),
    }
}

pub(super) fn build_session_snapshot(
    plan: &ResolvedHarnessPlan,
    session: &HarnessSession,
    events: &TuiEventBuffer,
    report_path: Option<PathBuf>,
    trace_path: Option<PathBuf>,
) -> TuiSessionSnapshot {
    let mut snapshot = TuiSessionSnapshot {
        session_id: session.session_id.clone(),
        workspace: workspace_readiness_from_plan(plan),
        run: run_snapshot_from_session(session),
        usage: session.usage.clone(),
        trace: TuiTraceSnapshot {
            events: events.events(),
        },
        reports: TuiReportSnapshot {
            current_report_path: report_path,
            current_trace_path: trace_path,
            current_report: None,
            current_trace_events: Vec::new(),
            current_report_error: None,
            current_trace_error: None,
        },
        services: service_snapshot_from_session(session),
    };
    snapshot.run.phase_objective = run_phase_objective(plan, snapshot.run.phase_id.as_deref());
    if snapshot.run.latest_output.is_none() {
        snapshot.run.latest_output = events.latest_phase_output();
    }
    snapshot.run.transcript = events.transcript();
    snapshot
}

fn run_snapshot_from_session(session: &HarnessSession) -> TuiRunSnapshot {
    let Some(run) = session.active_run() else {
        return TuiRunSnapshot {
            status: TuiRunStatus::Idle,
            run_id: None,
            phase_id: None,
            started_at: None,
            phase_objective: run_phase_objective_for_idle(session),
            terminal_status: None,
            latest_output: None,
            transcript: Vec::new(),
            usage: RunUsage::default(),
            approval: None,
        };
    };
    let status = match run.status() {
        RuntimeTerminalStatus::Running => TuiRunStatus::Active,
        RuntimeTerminalStatus::PendingApproval => TuiRunStatus::PendingApproval,
        RuntimeTerminalStatus::Ended
        | RuntimeTerminalStatus::HandedOff
        | RuntimeTerminalStatus::Aborted
        | RuntimeTerminalStatus::Failed
        | RuntimeTerminalStatus::Cancelled
        | RuntimeTerminalStatus::LimitReached
        | RuntimeTerminalStatus::ApprovalRequired => TuiRunStatus::Terminal,
    };
    TuiRunSnapshot {
        status,
        run_id: Some(run.run_id().to_string()),
        phase_id: run.current_phase_id().map(str::to_string),
        started_at: Some(run.started_at()),
        phase_objective: None,
        terminal_status: run.status().harness_status(),
        latest_output: run.terminal_output().cloned(),
        transcript: Vec::new(),
        usage: run.usage().clone(),
        approval: run.pending_approval().map(|approval| TuiApprovalSnapshot {
            checkpoint_id: approval.checkpoint_id.clone(),
            before_phase: approval.before_phase.clone(),
        }),
    }
}

fn run_phase_objective(
    plan: &ResolvedHarnessPlan,
    current_phase_id: Option<&str>,
) -> Option<String> {
    let loop_manifest = load_plan_loop(plan).ok()?;
    let phase_id = current_phase_id.unwrap_or(&loop_manifest.r#loop.entry_phase);
    loop_manifest
        .r#loop
        .phases
        .iter()
        .find(|phase| phase.id == phase_id)
        .map(|phase| phase.objective.clone())
}

fn run_phase_objective_for_idle(_session: &HarnessSession) -> Option<String> {
    None
}

fn service_snapshot_from_session(session: &HarnessSession) -> TuiServiceSnapshot {
    TuiServiceSnapshot {
        mcp_exports: session
            .runtime_snapshot
            .mcp_exports
            .iter()
            .map(|surface| TuiServiceSurfaceSnapshot {
                identity: surface.id.clone(),
                state: surface.state.clone(),
                detail: format!("{} tools", surface.tools.len()),
            })
            .collect(),
        mcp_imports: session
            .runtime_snapshot
            .mcp_imports
            .iter()
            .map(|tool| TuiServiceSurfaceSnapshot {
                identity: tool.identity.clone(),
                state: tool.state.clone(),
                detail: format!("{} / {}", tool.server_id, tool.tool_name),
            })
            .collect(),
    }
}

pub(super) fn workspace_readiness_from_plan(plan: &ResolvedHarnessPlan) -> TuiWorkspaceReadiness {
    let mut categories = vec![
        TuiReadinessCategory {
            label: "Agent".into(),
            state: if plan.selected_agent.is_some() {
                CapabilityState::Available
            } else {
                CapabilityState::NotConfigured
            },
            summary: plan
                .selected_agent
                .as_ref()
                .map(|agent| format!("{}@{}", agent.name, agent.version))
                .unwrap_or_else(|| "not selected".into()),
            source: plan
                .selected_agent
                .as_ref()
                .and_then(|agent| agent.package_key.clone())
                .or_else(|| Some("workspace".into())),
        },
        TuiReadinessCategory {
            label: "Loop".into(),
            state: if plan.loop_package.is_some() {
                CapabilityState::Available
            } else {
                CapabilityState::NotConfigured
            },
            summary: plan
                .loop_package
                .as_ref()
                .map(|package| format!("{}@{}", package.name, package.version))
                .unwrap_or_else(|| "not resolved".into()),
            source: Some("agent_dependency".into()),
        },
        TuiReadinessCategory {
            label: "Consumer Context".into(),
            state: plan.consumer_context.state,
            summary: format!(
                "{} · {}",
                consumer_context_file_summary(plan),
                consumer_context_token_summary(plan)
            ),
            source: Some("agent_binding".into()),
        },
        TuiReadinessCategory {
            label: "Model".into(),
            state: model_state(plan),
            summary: model_summary(plan),
            source: plan
                .config
                .config
                .model
                .as_ref()
                .map(|_| config_source_label(plan.config.model_source.kind).into()),
        },
    ];
    categories.extend([
        capability_readiness_category(plan, "Tools", "tool", "tool"),
        capability_readiness_category(plan, "Skills", "skill", "skill"),
        capability_readiness_category(plan, "Knowledge", "knowledge", "source"),
        memory_readiness_category(plan),
        capability_readiness_category(plan, "Profiles", "profile", "profile"),
        capability_readiness_category(plan, "Hooks", "hook", "bound"),
        TuiReadinessCategory {
            label: "MCP Exports".into(),
            state: bool_readiness_state(plan.report.mcp_exports.enabled),
            summary: format!("{} surfaces", plan.report.mcp_exports.surfaces.len()),
            source: Some("harness_config".into()),
        },
        TuiReadinessCategory {
            label: "MCP Imports".into(),
            state: bool_readiness_state(plan.report.mcp_imports.enabled),
            summary: format!("{} servers", plan.report.mcp_imports.servers.len()),
            source: Some("harness_config".into()),
        },
    ]);
    let diagnostics = plan
        .report
        .diagnostics
        .iter()
        .map(|diagnostic| TuiDiagnosticSummary {
            severity: diagnostic.severity,
            code: diagnostic.code.clone(),
            message: diagnostic.message.clone(),
        })
        .collect();
    TuiWorkspaceReadiness {
        categories,
        diagnostics,
    }
}

fn capability_readiness_category(
    plan: &ResolvedHarnessPlan,
    label: &str,
    kind: &str,
    noun: &str,
) -> TuiReadinessCategory {
    let source = capability_source_summary(plan, kind);
    TuiReadinessCategory {
        label: label.into(),
        state: grouped_capability_state(plan, kind),
        summary: capability_summary(plan, kind, noun),
        source,
    }
}

fn capability_source_summary(plan: &ResolvedHarnessPlan, kind: &str) -> Option<String> {
    let mut sources: Vec<_> = plan
        .capabilities
        .iter()
        .filter(|capability| !capability.kind.contains("runtime"))
        .filter(|capability| capability.kind.contains(kind))
        .map(|capability| capability.source.as_str())
        .collect();
    sources.sort_unstable();
    sources.dedup();
    match sources.as_slice() {
        [] => None,
        [source] => Some((*source).into()),
        _ => Some(format!("{} sources", sources.len())),
    }
}

fn memory_readiness_category(plan: &ResolvedHarnessPlan) -> TuiReadinessCategory {
    let source = capability_source_summary(plan, "memory");
    let counts = grouped_capability_counts(plan, "memory");
    let pending_requires_attention = plan
        .report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "unresolved_runtime_scope");
    let state = if counts.unavailable > 0 {
        CapabilityState::Unavailable
    } else if counts.suppressed > 0 {
        CapabilityState::Suppressed
    } else if counts.pending > 0 && pending_requires_attention {
        CapabilityState::Pending
    } else if counts.available + counts.pending > 0 {
        CapabilityState::Available
    } else {
        CapabilityState::NotConfigured
    };
    let summary = if counts.total() == 0 {
        "none configured".into()
    } else if state == CapabilityState::Available && counts.pending > 0 {
        let ready = counts.available + counts.pending;
        format!("{} {} ready", ready, pluralize("space", ready))
    } else {
        capability_summary(plan, "memory", "space")
    };
    TuiReadinessCategory {
        label: "Memory".into(),
        state,
        summary,
        source,
    }
}

fn bool_readiness_state(ready: bool) -> CapabilityState {
    if ready {
        CapabilityState::Available
    } else {
        CapabilityState::NotConfigured
    }
}

pub(super) fn config_source_label(source: HarnessConfigSourceKind) -> &'static str {
    match source {
        HarnessConfigSourceKind::HarnessDefault => "default",
        HarnessConfigSourceKind::ConfigFile => "config_file",
        HarnessConfigSourceKind::CliOverride => "cli_override",
        HarnessConfigSourceKind::SdkOverride => "sdk_override",
        HarnessConfigSourceKind::InteractiveOverride => "interactive",
        HarnessConfigSourceKind::Environment => "environment",
    }
}

#[derive(Default)]
pub(super) struct CapabilityCounts {
    pub(super) available: usize,
    pub(super) pending: usize,
    pub(super) suppressed: usize,
    pub(super) unavailable: usize,
}

impl CapabilityCounts {
    fn total(&self) -> usize {
        self.available + self.pending + self.suppressed + self.unavailable
    }
}

pub(super) fn grouped_capability_counts(
    plan: &ResolvedHarnessPlan,
    kind: &str,
) -> CapabilityCounts {
    let mut counts = CapabilityCounts::default();
    for capability in &plan.capabilities {
        if !capability.kind.contains(kind) {
            continue;
        }
        match capability.state {
            CapabilityState::Available => counts.available += 1,
            CapabilityState::Pending => counts.pending += 1,
            CapabilityState::Suppressed => counts.suppressed += 1,
            CapabilityState::Unavailable => counts.unavailable += 1,
            CapabilityState::NotConfigured => {}
        }
    }
    counts
}

fn grouped_capability_state(plan: &ResolvedHarnessPlan, kind: &str) -> CapabilityState {
    let counts = grouped_capability_counts(plan, kind);
    if counts.unavailable > 0 {
        CapabilityState::Unavailable
    } else if counts.suppressed > 0 {
        CapabilityState::Suppressed
    } else if counts.pending > 0 {
        CapabilityState::Pending
    } else if counts.available > 0 {
        CapabilityState::Available
    } else {
        CapabilityState::NotConfigured
    }
}

fn model_state(plan: &ResolvedHarnessPlan) -> CapabilityState {
    if plan.config.config.model.is_some() {
        CapabilityState::Available
    } else {
        CapabilityState::NotConfigured
    }
}

fn model_summary(plan: &ResolvedHarnessPlan) -> String {
    plan.config
        .config
        .model
        .as_ref()
        .map(|model| format!("{} / {}", model.provider, model.model))
        .unwrap_or_else(|| "not configured".into())
}

fn consumer_context_file_summary(plan: &ResolvedHarnessPlan) -> String {
    let file = plan
        .consumer_context
        .file
        .clone()
        .unwrap_or_else(|| "not configured".into());
    match plan.consumer_context.byte_size {
        Some(bytes) => format!("{file} · {}", format_byte_size(bytes)),
        None => file,
    }
}

fn consumer_context_token_summary(plan: &ResolvedHarnessPlan) -> String {
    match plan.consumer_context.approximate_tokens {
        Some(tokens) => format!("~{} tok", compact_count(tokens)),
        None => "tokens unknown".into(),
    }
}

fn capability_summary(plan: &ResolvedHarnessPlan, kind: &str, noun: &str) -> String {
    let counts = grouped_capability_counts(plan, kind);
    if counts.total() == 0 {
        return "none configured".into();
    }
    let mut parts = Vec::new();
    push_count_part(&mut parts, counts.available, "ready");
    push_count_part(&mut parts, counts.pending, "pending");
    push_count_part(&mut parts, counts.suppressed, "suppressed");
    push_count_part(&mut parts, counts.unavailable, "unavailable");
    if parts.is_empty() {
        format!("0 {noun}s ready")
    } else if parts.len() == 1 && counts.available > 0 && noun == "bound" {
        format!("{} bound", counts.available)
    } else if parts.len() == 1 && counts.available > 0 {
        format!(
            "{} {} ready",
            counts.available,
            pluralize(noun, counts.available)
        )
    } else {
        parts.join(", ")
    }
}

fn push_count_part(parts: &mut Vec<String>, count: usize, label: &str) {
    if count > 0 {
        parts.push(format!("{count} {label}"));
    }
}

fn pluralize(noun: &str, count: usize) -> String {
    if count == 1 {
        noun.into()
    } else if noun == "memory" {
        "memory surfaces".into()
    } else {
        format!("{noun}s")
    }
}

fn format_byte_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("~{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("~{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn compact_count(value: u64) -> String {
    if value >= 1000 {
        format!("{:.1}k", value as f64 / 1000.0)
    } else {
        value.to_string()
    }
}
