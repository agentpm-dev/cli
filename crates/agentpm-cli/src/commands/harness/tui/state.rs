use super::super::{
    HarnessArgs, harness_engine_options_from_plan, load_plan_loop, runtime_snapshot_from_plan,
};
use crate::harness_config::{HarnessConfigSourceKind, HarnessTraceContent};
use crate::harness_engine::{
    HarnessEngine, HarnessRunResult, HarnessRuntimeServices, HarnessSession,
    MemoryOperationInvocationResult, RuntimeTerminalResult, RuntimeTerminalStatus,
};
use crate::harness_observability::{
    HarnessEventBuilder, HarnessEventEnvelope, HarnessEventPayload, HarnessEventSink,
    HarnessEventType, HarnessTerminalStatus, RunOutputPaths, RunReport, RunUsage, SessionUsage,
    apply_content_policy,
};
use crate::harness_plan::{
    CapabilityState, HarnessBootstrapOptions, HarnessExecutionSurface, HarnessPlanProgress,
    HarnessPlanProgressStage, PreflightDiagnosticSeverity, ResolvedHarnessPlan,
    resolve_harness_plan_with_progress,
};
use crate::harness_runtime::{HookRuntime, ModelRuntime};
use crate::prelude::*;
use anyhow::{Context, anyhow, bail};
use serde_json::Value;
use std::{
    any::Any,
    collections::{BTreeMap, VecDeque},
    fs,
    panic::{self, AssertUnwindSafe},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
};

type BootstrapResult = Result<ResolvedHarnessPlan>;

pub(super) enum BootstrapMessage {
    Progress(TuiBootstrapProgress),
    Ready(Box<BootstrapResult>),
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
    pub(super) terminal_status: Option<HarnessTerminalStatus>,
    pub(super) latest_output: Option<Value>,
    pub(super) usage: RunUsage,
    pub(super) approval: Option<TuiApprovalSnapshot>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TuiReportSnapshot {
    pub(super) current_report_path: Option<PathBuf>,
    pub(super) current_trace_path: Option<PathBuf>,
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
    pub(super) engine: HarnessEngine,
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
        let loop_manifest = load_plan_loop(&plan)?;
        let engine = HarnessEngine::new(loop_manifest, harness_engine_options_from_plan(&plan));
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

    pub(super) fn plan(&self) -> &ResolvedHarnessPlan {
        &self.plan
    }

    pub(super) fn snapshot(&self) -> &TuiSessionSnapshot {
        &self.snapshot
    }

    pub(super) fn refresh_snapshot(&mut self) {
        let report_path = self.snapshot.reports.current_report_path.clone();
        let trace_path = self.snapshot.reports.current_trace_path.clone();
        self.snapshot = build_session_snapshot(
            &self.plan,
            &self.session,
            &self.events,
            report_path,
            trace_path,
        );
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
        let result = self.engine.invoke_memory_operation(
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
        self.snapshot.run.terminal_status = Some(terminal.status);
        self.snapshot.run.latest_output = terminal.output.clone();
        self.snapshot.run.usage = terminal.report.usage.clone();
        self.snapshot.reports.current_report_path = Some(output_paths.report_path.clone());
        self.snapshot.reports.current_trace_path = terminal
            .report
            .trace_path
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| Some(output_paths.events_path.clone()));
        self.snapshot.usage = self.session.usage.clone();
        self.snapshot.trace.events = self.events.events();
    }
}

#[derive(Clone)]
pub(super) struct TuiEventBuffer {
    pub(super) events: Arc<Mutex<VecDeque<HarnessEventEnvelope>>>,
    pub(super) policy: HarnessTraceContent,
    pub(super) capacity: usize,
}

impl TuiEventBuffer {
    pub(super) fn new(policy: HarnessTraceContent, capacity: usize) -> Self {
        Self {
            events: Arc::new(Mutex::new(VecDeque::new())),
            policy,
            capacity,
        }
    }

    pub(super) fn sink(&self) -> TuiEventSink {
        TuiEventSink {
            events: Arc::clone(&self.events),
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
}

pub(super) struct TuiEventSink {
    events: Arc<Mutex<VecDeque<HarnessEventEnvelope>>>,
    policy: HarnessTraceContent,
    capacity: usize,
}

impl HarnessEventSink for TuiEventSink {
    fn record(&mut self, event: &HarnessEventEnvelope) -> Result<()> {
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
    TuiSessionSnapshot {
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
        },
        services: service_snapshot_from_session(session),
    }
}

fn run_snapshot_from_session(session: &HarnessSession) -> TuiRunSnapshot {
    let Some(run) = session.active_run() else {
        return TuiRunSnapshot {
            status: TuiRunStatus::Idle,
            run_id: None,
            phase_id: None,
            terminal_status: None,
            latest_output: None,
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
        terminal_status: run.status().harness_status(),
        latest_output: run.terminal_output().cloned(),
        usage: run.usage().clone(),
        approval: run.pending_approval().map(|approval| TuiApprovalSnapshot {
            checkpoint_id: approval.checkpoint_id.clone(),
            before_phase: approval.before_phase.clone(),
        }),
    }
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
                .map(|_| config_source_label(plan.config.state_dir_source.kind).into()),
        },
    ];
    categories.extend([
        capability_readiness_category(plan, "Tools", "tool", "tool"),
        capability_readiness_category(plan, "Skills", "skill", "skill"),
        capability_readiness_category(plan, "Knowledge", "knowledge", "source"),
        capability_readiness_category(plan, "Memory", "memory", "space"),
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

fn bool_readiness_state(ready: bool) -> CapabilityState {
    if ready {
        CapabilityState::Available
    } else {
        CapabilityState::NotConfigured
    }
}

fn config_source_label(source: HarnessConfigSourceKind) -> &'static str {
    match source {
        HarnessConfigSourceKind::HarnessDefault => "default",
        HarnessConfigSourceKind::ConfigFile => "config_file",
        HarnessConfigSourceKind::CliOverride => "cli_override",
        HarnessConfigSourceKind::SdkOverride => "sdk_override",
        HarnessConfigSourceKind::Environment => "environment",
    }
}

pub(super) fn read_tui_report(snapshot: &TuiReportSnapshot) -> Result<Option<RunReport>> {
    let Some(path) = &snapshot.current_report_path else {
        return Ok(None);
    };
    let bytes = fs::read(path).with_context(|| format!("reading TUI report {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("parsing TUI report {}", path.display()))
        .map(Some)
}

pub(super) fn read_tui_trace(
    snapshot: &TuiReportSnapshot,
    limit: usize,
) -> Result<Vec<HarnessEventEnvelope>> {
    let Some(path) = &snapshot.current_trace_path else {
        return Ok(Vec::new());
    };
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading TUI trace {}", path.display()))?;
    let mut events = Vec::new();
    for line in content
        .lines()
        .rev()
        .take(limit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        events.push(
            serde_json::from_str(line)
                .with_context(|| format!("parsing TUI trace event from {}", path.display()))?,
        );
    }
    Ok(events)
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
