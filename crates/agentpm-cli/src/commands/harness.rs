use crate::adapter::list_locked_tool_descriptors;
use crate::harness_plan::{
    CapabilityState, HarnessBootstrapOptions, HarnessExecutionSurface, PreflightDiagnosticSeverity,
    PreflightStatus, ResolvedHarnessPlan, ResolvedPackageInfo, resolve_harness_plan,
    tool_readiness_state,
};
use crate::harness_runtime::{
    ActionDispatcher, AgentPmActionDispatcher, ApprovalController, BuiltInModelRuntime,
    CompositeKnowledgeRuntime, ConfiguredApprovalController, ConfiguredHookRuntime,
    ConsumerContextSnapshot, CustomKnowledgeRuntime, CustomMemoryRuntime, HookRuntime,
    HostServiceInvoker, KnowledgeEmbeddingSnapshot, KnowledgeRuntime, KnowledgeRuntimeSnapshot,
    LocalKnowledgeRuntime, McpExportRuntimeSnapshot, McpImportRuntimeActivation,
    McpImportRuntimeSnapshot, MemoryOperationRefRuntimeSnapshot, MemoryOperationRuntimeSnapshot,
    MemoryRecordTypeRuntimeSnapshot, MemorySpaceRuntimeSnapshot, ModelCapabilityAdvertisement,
    ModelProviderSelection, ModelRequest, ModelRuntime, ModelRuntimeFailure,
    ModelRuntimeRequestSnapshot, ModelTurn, PackageSnapshot, ProcessModelRuntime,
    RoutingEmbeddingProvider, RuntimeCapabilitySnapshot, RuntimeSnapshot, ServiceEmbeddingProvider,
    ServiceLifecycleEmitter, ServiceLifecycleEvents, ServiceReadinessSnapshot,
    SkillResourceSnapshot, SkillRuntimeSnapshot, ToolRuntimeSnapshot,
};
use crate::manifest::{
    AgentManifest, AgentMemoryBinding, MemoryManifest, MemoryOperation, MemoryOperationRef,
    MemoryOperationTarget, MemoryRetrievalMode, MemorySourceHandling, MemoryTransformOutputMode,
    MemoryTrigger, load_manifest_value, parse_knowledge_manifest, parse_loop_manifest,
    parse_memory_manifest, parse_skill_manifest, parse_tool_manifest,
};
use crate::prelude::*;
use crate::{
    harness_config::{HarnessHookId, HarnessTraceLevel},
    harness_engine::{
        EngineControlIngress, HarnessEngine, HarnessEngineOptions, HarnessRunResult,
        HarnessRuntimeServices, HarnessSession, MemoryOperationControlError,
        MemoryOperationInvocationResult, RuntimeTerminalResult,
    },
    harness_observability::{
        HarnessEventEnvelope, HarnessEventSink, HarnessTerminalStatus, JsonlTraceSink,
        MemoryWriteReviewReportSummary, RunOutputPaths, RunReport, allocate_harness_run_id,
        apply_content_policy, apply_content_policy_to_value,
    },
    harness_runtime::SdkHostHookRegistration,
};
use anyhow::{Context, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::io::{BufRead, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

use crate::semver::types::PackageKind;

mod custom_memory;
mod host_services;
mod runtime_snapshot;
mod tui;

use custom_memory::{
    activate_custom_memory_runtime_for_plan, apply_custom_memory_activation_to_runtime,
    custom_memory_routes,
};
use host_services::{
    host_service_registration_response, missing_required_host_services, register_host_service,
    required_host_services,
};
#[cfg(test)]
use runtime_snapshot::knowledge_snapshots_from_plan;
use runtime_snapshot::runtime_snapshot_from_plan;

#[derive(Args, Debug, Clone, Default)]
pub struct HarnessArgs {
    /// Agent package identity to run, for example @owner/name or @owner/name@version
    #[arg(value_name = "AGENT")]
    pub agent: Option<String>,

    /// Path to agentpm.harness.json
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Override runtime.state_dir for this Harness session
    #[arg(long, value_name = "DIR")]
    pub state_dir: Option<PathBuf>,

    /// Resolve a runtime scope value, for example --scope user=user_123
    #[arg(long = "scope", value_name = "KEY=VALUE", value_parser = parse_scope)]
    pub scopes: Vec<(String, String)>,

    /// Use the machine/SDK protocol surface for readiness classification
    #[arg(long, conflicts_with = "headless")]
    pub machine: bool,

    /// Use the plain non-TUI script surface for readiness classification
    #[arg(long, conflicts_with = "machine")]
    pub headless: bool,

    /// Emit the preflight report as JSON
    #[arg(long)]
    pub json: bool,

    /// Include detailed human-readable preflight sections
    #[arg(long)]
    pub verbose: bool,

    /// Run input text for one-shot --headless execution
    #[arg(long, value_name = "TEXT", conflicts_with = "input_file")]
    pub input: Option<String>,

    /// File containing Run input for one-shot --headless execution
    #[arg(long = "input-file", value_name = "FILE", conflicts_with = "input")]
    pub input_file: Option<PathBuf>,

    /// Override the generated run report path for one-shot --headless execution
    #[arg(long = "report", value_name = "FILE")]
    pub report: Option<PathBuf>,
}

impl HarnessArgs {
    pub async fn run(self, _base_url: String) -> Result<()> {
        let workspace_root = std::env::current_dir().context("reading current directory")?;
        let surface = self.surface();
        validate_surface_flags(surface, &self)?;
        if surface == HarnessExecutionSurface::Tui && !self.json {
            return tui::run_tui_surface(self, workspace_root);
        }
        let plan = resolve_harness_plan(
            &workspace_root,
            &HarnessBootstrapOptions {
                agent_selector: self.agent.clone(),
                config_path: self.config.clone(),
                state_dir_override: self.state_dir.clone(),
                model_override: None,
                model_override_source: None,
                runtime_scopes: self.scopes.iter().cloned().collect(),
                surface,
            },
        )?;

        if surface == HarnessExecutionSurface::Machine {
            // Machine stdout is reserved exclusively for protocol JSONL frames.
        } else if self.json {
            println!("{}", serde_json::to_string_pretty(&plan.report)?);
        } else {
            let stream = PreflightOutputStream::for_surface(surface);
            print_harness_preflight(
                &plan,
                surface,
                stream,
                human_preflight_verbose_enabled(self.verbose, &plan),
            )?;
        }

        match plan.report.status {
            PreflightStatus::Ready | PreflightStatus::ReadyWithWarnings => {}
            PreflightStatus::SelectionRequired => {
                bail!("Harness preflight requires an explicit Agent selector")
            }
            PreflightStatus::Failed => bail!("Harness preflight failed"),
        }

        if surface == HarnessExecutionSurface::Tui {
            return Ok(());
        }

        run_surface(surface, plan, self)
    }

    fn surface(&self) -> HarnessExecutionSurface {
        if self.machine {
            HarnessExecutionSurface::Machine
        } else if self.headless {
            HarnessExecutionSurface::Headless
        } else {
            HarnessExecutionSurface::Tui
        }
    }
}

fn validate_surface_flags(surface: HarnessExecutionSurface, args: &HarnessArgs) -> Result<()> {
    if surface == HarnessExecutionSurface::Headless && args.json {
        bail!(
            "--json cannot be combined with --headless; headless stdout is reserved for final output"
        );
    }
    if surface == HarnessExecutionSurface::Machine && args.json {
        bail!(
            "--json cannot be combined with --machine; machine stdout is reserved for protocol frames"
        );
    }
    Ok(())
}

impl HarnessExecutionSurface {
    fn run(self, plan: &ResolvedHarnessPlan, args: &HarnessArgs) -> Result<()> {
        match self {
            HarnessExecutionSurface::Headless => run_headless_surface(plan, args),
            HarnessExecutionSurface::Machine => run_machine_surface(plan, args),
            HarnessExecutionSurface::Tui => {
                bail!("internal error: TUI surface must be routed before preflight")
            }
        }
    }
}

fn run_surface(
    surface: HarnessExecutionSurface,
    plan: ResolvedHarnessPlan,
    args: HarnessArgs,
) -> Result<()> {
    if requires_blocking_surface_worker(surface) {
        return run_blocking_surface_worker(move || surface.run(&plan, &args));
    }
    surface.run(&plan, &args)
}

fn requires_blocking_surface_worker(surface: HarnessExecutionSurface) -> bool {
    matches!(
        surface,
        HarnessExecutionSurface::Headless | HarnessExecutionSurface::Machine
    )
}

fn run_blocking_surface_worker(run: impl FnOnce() -> Result<()> + Send + 'static) -> Result<()> {
    std::thread::spawn(run)
        .join()
        .map_err(|_| anyhow!("Harness execution worker panicked"))?
}

#[derive(Debug)]
struct ManagedMcpExportSurface {
    snapshot: McpExportRuntimeSnapshot,
    child: Child,
    binding: crate::manifest::AgentMcpBinding,
    restart_attempts: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct McpExportSuppressedTool {
    tool: String,
    reason: String,
}

#[derive(Debug, Clone)]
struct McpExportSurfaceSelection {
    binding: crate::manifest::AgentMcpBinding,
    suppressed_tools: Vec<McpExportSuppressedTool>,
}

#[derive(Debug, Default)]
struct ManagedMcpExports {
    surfaces: Vec<ManagedMcpExportSurface>,
    activity: McpExportActivity,
}

#[derive(Debug, Clone, Default)]
struct McpExportActivity {
    counts: Arc<Mutex<McpExportActivityCounts>>,
}

type McpExportActivityKey = (String, String, String);
type McpExportActivityCounts = BTreeMap<McpExportActivityKey, u64>;

impl ManagedMcpExports {
    fn start(plan: &ResolvedHarnessPlan, session: &mut HarnessSession) -> Result<Self> {
        Self::start_with_machine_writer(plan, session, None)
    }

    fn start_with_machine_writer(
        plan: &ResolvedHarnessPlan,
        session: &mut HarnessSession,
        machine_writer: Option<MachineProtocolWriter>,
    ) -> Result<Self> {
        if !plan.config.config.mcp.exports.enabled {
            return Ok(Self::default());
        }
        let bindings = mcp_export_bindings(plan)?;
        if bindings.is_empty() {
            return Ok(Self::default());
        }
        let mut exports = Self::default();
        for binding in bindings {
            let selection = mcp_export_surface_selection(plan, &binding)?;
            if selection.binding.tools.is_empty() {
                session.emitter.emit(
                    crate::harness_observability::HarnessEventType::McpSurfaceFailed,
                    crate::harness_observability::HarnessEventPayload::Lifecycle {
                        message: "MCP export surface has no ready Tools.".into(),
                        fields: BTreeMap::from([
                            ("surface".into(), json!(binding.id)),
                            ("requested_tools".into(), json!(binding.tools)),
                            ("suppressed_tools".into(), json!(selection.suppressed_tools)),
                            ("reason".into(), json!("empty_ready_tool_subset")),
                        ]),
                    },
                    Default::default(),
                )?;
                continue;
            }
            session.emitter.emit(
                crate::harness_observability::HarnessEventType::McpSurfaceStarting,
                crate::harness_observability::HarnessEventPayload::Lifecycle {
                    message: "MCP export surface starting.".into(),
                    fields: BTreeMap::from([
                        ("surface".into(), json!(selection.binding.id)),
                        ("tools".into(), json!(selection.binding.tools)),
                        ("suppressed_tools".into(), json!(selection.suppressed_tools)),
                    ]),
                },
                Default::default(),
            )?;
            match start_mcp_export_surface(
                plan,
                &selection.binding,
                machine_writer.clone(),
                exports.activity.clone(),
            ) {
                Ok(surface) => {
                    session.emitter.emit(
                        crate::harness_observability::HarnessEventType::McpSurfaceReady,
                        crate::harness_observability::HarnessEventPayload::Lifecycle {
                            message: "MCP export surface is ready.".into(),
                            fields: BTreeMap::from([
                                ("surface".into(), json!(surface.snapshot.id)),
                                ("host".into(), json!(surface.snapshot.host)),
                                ("port".into(), json!(surface.snapshot.port)),
                                ("endpoint".into(), json!(surface.snapshot.endpoint)),
                                ("tools".into(), json!(surface.snapshot.tools)),
                            ]),
                        },
                        Default::default(),
                    )?;
                    exports.surfaces.push(surface);
                }
                Err(err) => {
                    session.emitter.emit(
                        crate::harness_observability::HarnessEventType::McpSurfaceFailed,
                        crate::harness_observability::HarnessEventPayload::Lifecycle {
                            message: "MCP export surface failed to start.".into(),
                            fields: BTreeMap::from([
                                ("surface".into(), json!(selection.binding.id)),
                                ("error".into(), json!(err.to_string())),
                            ]),
                        },
                        Default::default(),
                    )?;
                }
            }
        }
        Ok(exports)
    }

    fn snapshots(&self) -> Vec<McpExportRuntimeSnapshot> {
        self.surfaces
            .iter()
            .map(|surface| surface.snapshot.clone())
            .collect()
    }

    fn refresh(
        &mut self,
        plan: &ResolvedHarnessPlan,
        session: &mut HarnessSession,
        machine_writer: Option<MachineProtocolWriter>,
    ) -> Result<()> {
        self.refresh_with_restart(plan, session, machine_writer, true)
    }

    fn refresh_without_restart(
        &mut self,
        plan: &ResolvedHarnessPlan,
        session: &mut HarnessSession,
        machine_writer: Option<MachineProtocolWriter>,
    ) -> Result<()> {
        self.refresh_with_restart(plan, session, machine_writer, false)
    }

    fn refresh_with_restart(
        &mut self,
        plan: &ResolvedHarnessPlan,
        session: &mut HarnessSession,
        machine_writer: Option<MachineProtocolWriter>,
        allow_restart: bool,
    ) -> Result<()> {
        for surface in &mut self.surfaces {
            if surface.child.try_wait()?.is_none() {
                continue;
            }
            let _ = surface.child.wait();
            session.emitter.emit(
                crate::harness_observability::HarnessEventType::McpSurfaceFailed,
                crate::harness_observability::HarnessEventPayload::Lifecycle {
                    message: "MCP export surface process exited.".into(),
                    fields: BTreeMap::from([
                        ("surface".into(), json!(surface.snapshot.id)),
                        ("endpoint".into(), json!(surface.snapshot.endpoint)),
                        ("reason".into(), json!("process_exited")),
                    ]),
                },
                Default::default(),
            )?;
            if !allow_restart {
                surface.snapshot.state = "failed".into();
                continue;
            }
            let restart_policy = &plan.config.config.mcp.exports.restart;
            if surface.restart_attempts >= restart_policy.max_attempts {
                surface.snapshot.state = "failed".into();
                session.emitter.emit(
                    crate::harness_observability::HarnessEventType::McpSurfaceFailed,
                    crate::harness_observability::HarnessEventPayload::Lifecycle {
                        message: "MCP export surface restart attempts exhausted.".into(),
                        fields: BTreeMap::from([
                            ("surface".into(), json!(surface.snapshot.id)),
                            ("endpoint".into(), json!(surface.snapshot.endpoint)),
                            ("reason".into(), json!("restart_exhausted")),
                        ]),
                    },
                    Default::default(),
                )?;
                continue;
            }

            surface.restart_attempts += 1;
            surface.snapshot.state = "restarting".into();
            session.emitter.emit(
                crate::harness_observability::HarnessEventType::McpSurfaceStarting,
                crate::harness_observability::HarnessEventPayload::Lifecycle {
                    message: "MCP export surface restarting.".into(),
                    fields: BTreeMap::from([
                        ("surface".into(), json!(surface.snapshot.id)),
                        ("restart_attempt".into(), json!(surface.restart_attempts)),
                    ]),
                },
                Default::default(),
            )?;
            std::thread::sleep(Duration::from_millis(restart_policy.backoff_ms));
            match start_mcp_export_surface(
                plan,
                &surface.binding,
                machine_writer.clone(),
                self.activity.clone(),
            ) {
                Ok(restarted) => {
                    surface.snapshot = restarted.snapshot;
                    surface.child = restarted.child;
                    session.emitter.emit(
                        crate::harness_observability::HarnessEventType::McpSurfaceReady,
                        crate::harness_observability::HarnessEventPayload::Lifecycle {
                            message: "MCP export surface restarted.".into(),
                            fields: BTreeMap::from([
                                ("surface".into(), json!(surface.snapshot.id)),
                                ("host".into(), json!(surface.snapshot.host)),
                                ("port".into(), json!(surface.snapshot.port)),
                                ("endpoint".into(), json!(surface.snapshot.endpoint)),
                                ("tools".into(), json!(surface.snapshot.tools)),
                                ("restart_attempt".into(), json!(surface.restart_attempts)),
                            ]),
                        },
                        Default::default(),
                    )?;
                }
                Err(err) => {
                    surface.snapshot.state = "failed".into();
                    session.emitter.emit(
                        crate::harness_observability::HarnessEventType::McpSurfaceFailed,
                        crate::harness_observability::HarnessEventPayload::Lifecycle {
                            message: "MCP export surface restart failed.".into(),
                            fields: BTreeMap::from([
                                ("surface".into(), json!(surface.snapshot.id)),
                                ("error".into(), json!(err.to_string())),
                                ("restart_attempt".into(), json!(surface.restart_attempts)),
                            ]),
                        },
                        Default::default(),
                    )?;
                }
            }
        }
        Ok(())
    }

    fn report_summaries(&self) -> Vec<crate::harness_observability::OperationReportSummary> {
        let mut summaries = self
            .snapshots()
            .into_iter()
            .map(
                |surface| crate::harness_observability::OperationReportSummary {
                    operation_kind: "mcp_export".into(),
                    identity: surface.id,
                    status: surface.state,
                    count: surface.tools.len().try_into().unwrap_or(0),
                },
            )
            .collect::<Vec<_>>();
        summaries.extend(self.activity.report_summaries());
        summaries
    }

    fn stop(&mut self, session: &mut HarnessSession) -> Result<()> {
        for surface in &mut self.surfaces {
            stop_mcp_export_child(&mut surface.child);
            session.emitter.emit(
                crate::harness_observability::HarnessEventType::McpSurfaceStopped,
                crate::harness_observability::HarnessEventPayload::Lifecycle {
                    message: "MCP export surface stopped.".into(),
                    fields: BTreeMap::from([
                        ("surface".into(), json!(surface.snapshot.id)),
                        ("endpoint".into(), json!(surface.snapshot.endpoint)),
                    ]),
                },
                Default::default(),
            )?;
        }
        self.surfaces.clear();
        Ok(())
    }
}

fn refresh_mcp_exports_for_session(
    plan: &ResolvedHarnessPlan,
    session: &mut HarnessSession,
    mcp_exports: &mut ManagedMcpExports,
    machine_writer: Option<MachineProtocolWriter>,
) -> Result<()> {
    mcp_exports.refresh(plan, session, machine_writer)?;
    session.runtime_snapshot.mcp_exports = mcp_exports.snapshots();
    Ok(())
}

fn refresh_mcp_exports_for_session_without_restart(
    plan: &ResolvedHarnessPlan,
    session: &mut HarnessSession,
    mcp_exports: &mut ManagedMcpExports,
    machine_writer: Option<MachineProtocolWriter>,
) -> Result<()> {
    mcp_exports.refresh_without_restart(plan, session, machine_writer)?;
    session.runtime_snapshot.mcp_exports = mcp_exports.snapshots();
    Ok(())
}

fn activate_mcp_import_runtime_for_plan(plan: &ResolvedHarnessPlan) -> McpImportRuntimeActivation {
    crate::harness_runtime::ConfiguredMcpImportRuntime::start(
        &plan.workspace_root,
        &plan.config.config.mcp.imports,
    )
}

fn apply_mcp_import_activation_to_runtime(
    runtime: &mut RuntimeSnapshot,
    activation: &McpImportRuntimeActivation,
) {
    runtime
        .mcp_imports
        .extend(activation.snapshots.iter().cloned());
    runtime
        .capability_candidates
        .extend(activation.capability_candidates.iter().cloned());
}

fn emit_mcp_import_activation_events(
    session: &mut HarnessSession,
    snapshots: &[McpImportRuntimeSnapshot],
) -> Result<()> {
    for snapshot in snapshots {
        let event_type = if snapshot.state == "available" {
            crate::harness_observability::HarnessEventType::McpImportConnected
        } else {
            crate::harness_observability::HarnessEventType::McpImportFailed
        };
        let mut fields = BTreeMap::from([
            ("server".into(), json!(snapshot.server_id)),
            ("transport".into(), json!(snapshot.transport)),
            ("state".into(), json!(snapshot.state)),
            ("scopes".into(), json!(snapshot.scopes)),
        ]);
        if let Some(endpoint) = &snapshot.endpoint {
            fields.insert("endpoint".into(), json!(endpoint));
        }
        if !snapshot.tool_name.is_empty() {
            fields.insert("tool".into(), json!(snapshot.tool_name));
            fields.insert("identity".into(), json!(snapshot.identity));
        }
        if let Some(reason) = &snapshot.readiness_reason {
            fields.insert("reason".into(), json!(reason));
        }
        session.emitter.emit(
            event_type,
            crate::harness_observability::HarnessEventPayload::Lifecycle {
                message: if snapshot.state == "available" {
                    "MCP import Tool is ready.".into()
                } else {
                    "MCP import is unavailable.".into()
                },
                fields,
            },
            Default::default(),
        )?;
    }
    Ok(())
}

fn merge_mcp_report_summaries(
    report_summaries: &mut Vec<crate::harness_observability::OperationReportSummary>,
    managed_summaries: Vec<crate::harness_observability::OperationReportSummary>,
) {
    for managed in managed_summaries {
        let managed_key = mcp_report_summary_merge_key(&managed);
        if let Some(existing) = report_summaries
            .iter_mut()
            .find(|summary| mcp_report_summary_merge_key(summary) == managed_key)
        {
            *existing = managed;
        } else {
            report_summaries.push(managed);
        }
    }
}

fn mcp_report_summary_merge_key(
    summary: &crate::harness_observability::OperationReportSummary,
) -> (&str, &str, Option<&str>) {
    let status_key = match summary.operation_kind.as_str() {
        "mcp_export" => None,
        "mcp_tool_call" => Some(summary.status.as_str()),
        _ => Some(summary.status.as_str()),
    };
    (
        summary.operation_kind.as_str(),
        summary.identity.as_str(),
        status_key,
    )
}

impl McpExportActivity {
    fn record_child_event(&self, surface: &str, event: &Value) {
        let Some(event_type) = event.get("event").and_then(Value::as_str) else {
            return;
        };
        let status = match event_type {
            "tool_call_started" => "started",
            "tool_call_completed" => "completed",
            "tool_call_failed" => "failed",
            _ => return,
        };
        let identity = event
            .get("fields")
            .and_then(|fields| fields.get("identity"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| surface.to_string());
        *self
            .counts
            .lock()
            .expect("MCP export activity poisoned")
            .entry(("mcp_tool_call".into(), identity, status.into()))
            .or_insert(0) += 1;
    }

    fn report_summaries(&self) -> Vec<crate::harness_observability::OperationReportSummary> {
        self.counts
            .lock()
            .expect("MCP export activity poisoned")
            .iter()
            .map(|((operation_kind, identity, status), count)| {
                crate::harness_observability::OperationReportSummary {
                    operation_kind: operation_kind.clone(),
                    identity: identity.clone(),
                    status: status.clone(),
                    count: *count,
                }
            })
            .collect()
    }
}

struct McpImportActionDispatcher<'a> {
    delegate: &'a mut dyn ActionDispatcher,
    mcp_imports: Arc<Mutex<crate::harness_runtime::ConfiguredMcpImportRuntime>>,
}

impl ActionDispatcher for McpImportActionDispatcher<'_> {
    fn dispatch(
        &mut self,
        action: &crate::harness_runtime::SemanticAction,
    ) -> crate::harness_runtime::ActionDispatchResult {
        match action {
            crate::harness_runtime::SemanticAction::ExternalMcpTool {
                server,
                tool,
                arguments,
            } => self
                .mcp_imports
                .lock()
                .expect("MCP import runtime poisoned")
                .call_tool(server, tool, arguments),
            _ => self.delegate.dispatch(action),
        }
    }
}

impl Drop for ManagedMcpExports {
    fn drop(&mut self) {
        for surface in &mut self.surfaces {
            stop_mcp_export_child(&mut surface.child);
        }
    }
}

fn mcp_export_bindings(
    plan: &ResolvedHarnessPlan,
) -> Result<Vec<crate::manifest::AgentMcpBinding>> {
    let Some(agent) = &plan.selected_agent else {
        return Ok(Vec::new());
    };
    if !agent.manifest_path.exists() {
        return Ok(Vec::new());
    }
    let (value, _) = load_manifest_value(&agent.manifest_path)
        .with_context(|| format!("loading selected Agent {}", agent.manifest_path.display()))?;
    let manifest: AgentManifest =
        serde_json::from_value(value).context("parsing selected Agent manifest")?;
    Ok(manifest
        .bindings
        .map(|bindings| bindings.mcp)
        .unwrap_or_default())
}

fn mcp_export_surface_selection(
    plan: &ResolvedHarnessPlan,
    binding: &crate::manifest::AgentMcpBinding,
) -> Result<McpExportSurfaceSelection> {
    let descriptors = list_locked_tool_descriptors(&plan.workspace_root)?;
    let available_tools = descriptors
        .into_iter()
        .map(|descriptor| descriptor.package_ref)
        .collect::<BTreeSet<_>>();
    let top_level_tools = plan
        .selected_agent
        .as_ref()
        .map(|agent| {
            agent
                .tools
                .iter()
                .map(|tool| package_identity_for_mcp_export(tool))
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();

    let mut ready_tools = Vec::new();
    let mut suppressed_tools = Vec::new();
    for tool in &binding.tools {
        let identity = package_identity_for_mcp_export(tool);
        if !top_level_tools.contains(&identity) {
            suppressed_tools.push(McpExportSuppressedTool {
                tool: tool.clone(),
                reason: "not_top_level_agent_tool".into(),
            });
            continue;
        }
        if !available_tools.contains(tool) {
            suppressed_tools.push(McpExportSuppressedTool {
                tool: tool.clone(),
                reason: "not_ready_in_agent_lock".into(),
            });
            continue;
        }
        let mut diagnostics = Vec::new();
        if tool_readiness_state(
            &plan.workspace_root,
            &plan.package_graph,
            &identity,
            &mut diagnostics,
        ) != CapabilityState::Available
        {
            let reason = diagnostics
                .first()
                .map(|diagnostic| diagnostic.code.clone())
                .unwrap_or_else(|| "tool_unavailable".into());
            suppressed_tools.push(McpExportSuppressedTool {
                tool: tool.clone(),
                reason,
            });
            continue;
        }
        ready_tools.push(tool.clone());
    }

    Ok(McpExportSurfaceSelection {
        binding: crate::manifest::AgentMcpBinding {
            id: binding.id.clone(),
            tools: ready_tools,
        },
        suppressed_tools,
    })
}

fn package_identity_for_mcp_export(reference: &str) -> String {
    let without_kind = reference
        .split_once(':')
        .map_or(reference, |(_, value)| value);
    if let Some(stripped) = without_kind.strip_prefix('@') {
        let mut parts = stripped.split('@');
        let scope_and_name = parts.next().unwrap_or_default();
        if let Some((scope, name)) = scope_and_name.split_once('/') {
            return format!("@{scope}/{name}");
        }
        return format!("@{scope_and_name}");
    }
    without_kind
        .split_once('@')
        .map_or(without_kind, |(name, _)| name)
        .to_string()
}

fn start_mcp_export_surface(
    plan: &ResolvedHarnessPlan,
    binding: &crate::manifest::AgentMcpBinding,
    machine_writer: Option<MachineProtocolWriter>,
    activity: McpExportActivity,
) -> Result<ManagedMcpExportSurface> {
    if binding.tools.is_empty() {
        bail!(
            "MCP export surface `{}` has no selected Tools and will not be started",
            binding.id
        );
    }
    let executable = std::env::current_exe().context("locating current agentpm executable")?;
    let mut command = Command::new(executable);
    command
        .arg("serve")
        .arg("--mcp")
        .arg("--machine")
        .arg("--host")
        .arg(&plan.config.config.mcp.exports.host)
        .arg("--port")
        .arg("0")
        .current_dir(&plan.workspace_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for tool in &binding.tools {
        command.arg("--tool").arg(tool);
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("starting MCP export surface `{}`", binding.id))?;
    let stdout = child
        .stdout
        .take()
        .context("capturing MCP export machine stdout")?;
    let reader = std::io::BufReader::new(stdout);
    let ready = wait_for_mcp_ready_frame(binding.id.clone(), reader, machine_writer, activity)
        .with_context(|| format!("waiting for MCP export surface `{}` readiness", binding.id));
    let ready = match ready {
        Ok(ready) => ready,
        Err(err) => {
            stop_mcp_export_child(&mut child);
            let mut stderr = String::new();
            if let Some(mut child_stderr) = child.stderr.take() {
                let _ = child_stderr.read_to_string(&mut stderr);
            }
            let stderr = stderr.trim();
            if stderr.is_empty() {
                return Err(err);
            }
            return Err(err).with_context(|| {
                format!(
                    "MCP export surface `{}` stderr: {}",
                    binding.id,
                    stderr.lines().next().unwrap_or(stderr)
                )
            });
        }
    };
    let endpoint = ready
        .get("endpoint")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let host = ready
        .get("host")
        .and_then(Value::as_str)
        .unwrap_or(plan.config.config.mcp.exports.host.as_str())
        .to_string();
    let port = ready
        .get("port")
        .and_then(Value::as_u64)
        .and_then(|port| u16::try_from(port).ok())
        .unwrap_or(0);
    Ok(ManagedMcpExportSurface {
        snapshot: McpExportRuntimeSnapshot {
            id: binding.id.clone(),
            host,
            port,
            endpoint,
            tools: binding.tools.clone(),
            state: "ready".into(),
        },
        child,
        binding: binding.clone(),
        restart_attempts: 0,
    })
}

fn wait_for_mcp_ready_frame<R>(
    surface_id: String,
    reader: std::io::BufReader<R>,
    machine_writer: Option<MachineProtocolWriter>,
    activity: McpExportActivity,
) -> Result<Value>
where
    R: Read + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut ready_sent = false;
        for line in reader.lines() {
            let Ok(line) = line else {
                break;
            };
            let Ok(value) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if !ready_sent && value.get("event").and_then(Value::as_str) == Some("ready") {
                let _ = sender.send(value.get("fields").cloned().unwrap_or_else(|| json!({})));
                ready_sent = true;
            } else if ready_sent {
                activity.record_child_event(&surface_id, &value);
                if let Some(writer) = &machine_writer {
                    let _ = writer.write_event_payload(
                        None,
                        "mcp_export_event",
                        json!({
                            "surface": surface_id,
                            "event": value,
                        }),
                    );
                }
            }
        }
    });
    receiver
        .recv_timeout(Duration::from_secs(5))
        .context("timed out waiting for MCP machine ready event")
}

fn stop_mcp_export_child(child: &mut Child) {
    if child.try_wait().ok().flatten().is_none() {
        terminate_mcp_export_child(child);
    }
    let _ = child.wait();
}

#[cfg(unix)]
fn terminate_mcp_export_child(child: &mut Child) {
    let _ = unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
    for _ in 0..20 {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let _ = child.kill();
}

#[cfg(not(unix))]
fn terminate_mcp_export_child(child: &mut Child) {
    let _ = child.kill();
}

fn run_machine_surface(plan: &ResolvedHarnessPlan, args: &HarnessArgs) -> Result<()> {
    let writer = MachineProtocolWriter::stdout(plan.config.config.trace.content.clone());
    let cancellation_requested = Arc::new(AtomicBool::new(false));
    let active_run = Arc::new(AtomicBool::new(false));
    let reader = spawn_machine_stdin_reader(
        writer.clone(),
        active_run.clone(),
        cancellation_requested.clone(),
    );
    let bridge =
        MachineHostBridgeHandle::new(writer.clone(), reader, cancellation_requested, active_run);
    bridge.write_event_payload(
        None,
        "preflight",
        json!({
            "status": plan.report.status,
            "report": plan.report,
        }),
    )?;
    let mut mcp_export_session =
        HarnessSession::with_runtime_snapshot(runtime_snapshot_from_plan(plan));
    mcp_export_session
        .emitter
        .add_sink(Box::new(MachineEventSink::new(writer.clone())));
    let mut run_session = HarnessSession::with_runtime_snapshot(runtime_snapshot_from_plan(plan));
    run_session
        .emitter
        .add_sink(Box::new(MachineEventSink::new(writer.clone())));
    let mut mcp_exports = if matches!(
        plan.report.status,
        PreflightStatus::Ready | PreflightStatus::ReadyWithWarnings
    ) {
        ManagedMcpExports::start_with_machine_writer(
            plan,
            &mut mcp_export_session,
            Some(writer.clone()),
        )?
    } else {
        ManagedMcpExports::default()
    };
    refresh_mcp_exports_for_session(
        plan,
        &mut mcp_export_session,
        &mut mcp_exports,
        Some(writer.clone()),
    )?;
    let mut initialized = false;
    while let Some(request) = bridge.recv_control_request()? {
        let id = request.id.clone();
        if let Err(err) = validate_machine_request(&request) {
            bridge.write_error(id.as_deref(), "protocol_error", err)?;
            continue;
        }
        let method = request.method.as_deref().unwrap_or_default();
        match method {
            "initialize" => {
                refresh_mcp_exports_for_session(
                    plan,
                    &mut mcp_export_session,
                    &mut mcp_exports,
                    Some(writer.clone()),
                )?;
                initialized = true;
                bridge.write_response(
                    id.as_deref(),
                    json!({
                        "session": {
                            "protocol": AGENTPM_HARNESS_MACHINE_PROTOCOL,
                            "version": AGENTPM_HARNESS_MACHINE_VERSION,
                        },
                        "preflight": plan.report,
                        "mcp_exports": mcp_exports.snapshots(),
                        "required_host_services": required_host_services(plan),
                    }),
                )?;
            }
            "register_host_service" => {
                match register_host_service(plan, &bridge, &request.payload) {
                    Ok(service) => bridge.write_response(
                        id.as_deref(),
                        host_service_registration_response(&service),
                    )?,
                    Err(err) => {
                        bridge.write_error(id.as_deref(), "host_registration_failed", err)?
                    }
                }
            }
            "preflight" => {
                refresh_mcp_exports_for_session(
                    plan,
                    &mut mcp_export_session,
                    &mut mcp_exports,
                    Some(writer.clone()),
                )?;
                bridge.write_response(
                    id.as_deref(),
                    json!({
                        "report": plan.report,
                        "mcp_exports": mcp_exports.snapshots(),
                    }),
                )?;
            }
            "start_run" => {
                refresh_mcp_exports_for_session(
                    plan,
                    &mut mcp_export_session,
                    &mut mcp_exports,
                    Some(writer.clone()),
                )?;
                if !initialized {
                    bridge.write_error(
                        id.as_deref(),
                        "not_initialized",
                        "initialize must complete before start_run",
                    )?;
                    continue;
                }
                if !matches!(
                    plan.report.status,
                    PreflightStatus::Ready | PreflightStatus::ReadyWithWarnings
                ) {
                    bridge.write_error(
                        id.as_deref(),
                        "preflight_not_ready",
                        "preflight is not ready",
                    )?;
                    continue;
                }
                let missing = missing_required_host_services(plan, &bridge);
                if !missing.is_empty() {
                    bridge.write_error(
                        id.as_deref(),
                        "host_service_not_registered",
                        format!("missing required host service registrations: {missing:?}"),
                    )?;
                    continue;
                }
                let input = request
                    .payload
                    .get("input")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| args.input.clone())
                    .ok_or_else(|| anyhow!("machine start_run requires payload.input"))?;
                bridge.set_active_run(true);
                let terminal =
                    match execute_machine_run(plan, input, &bridge, &mcp_exports, &mut run_session)
                    {
                        Ok(terminal) => terminal,
                        Err(err) => {
                            bridge.set_active_run(false);
                            bridge.write_error(id.as_deref(), "run_failed", err.to_string())?;
                            continue;
                        }
                    };
                bridge.set_active_run(false);
                bridge.write_response(
                    id.as_deref(),
                    json!({
                        "status": terminal.status,
                        "output": terminal.output,
                        "report": terminal.report,
                    }),
                )?;
            }
            "cancel_run" => {
                bridge.request_cancellation();
                bridge.write_response(
                    id.as_deref(),
                    json!({
                        "status": HarnessTerminalStatus::Cancelled,
                        "accepted": true,
                        "note": "Cancellation will be observed by active machine host-service waits."
                    }),
                )?;
            }
            "memory_operation" => {
                bridge.write_error(
                    id.as_deref(),
                    "memory_operation_no_active_run",
                    "external Memory-operation control requires an active Harness Run",
                )?;
            }
            "shutdown" => {
                mcp_exports.stop(&mut mcp_export_session)?;
                mcp_export_session.emitter.flush()?;
                bridge.write_response(id.as_deref(), json!({ "shutdown": true }))?;
                break;
            }
            other => bridge.write_error(
                id.as_deref(),
                "unknown_method",
                format!("unknown machine method `{other}`"),
            )?,
        }
    }
    mcp_exports.stop(&mut mcp_export_session)?;
    mcp_export_session.emitter.flush()?;
    Ok(())
}

fn execute_machine_run(
    plan: &ResolvedHarnessPlan,
    input: String,
    bridge: &MachineHostBridgeHandle,
    mcp_exports: &ManagedMcpExports,
    session: &mut HarnessSession,
) -> Result<RuntimeTerminalResult> {
    let selection = model_selection(plan)?;
    let mut service_events = ServiceLifecycleEvents::new();
    bridge.set_host_service_lifecycle_emitter(service_events.emitter());
    let mut model = model_runtime_from_plan(
        plan,
        selection,
        Some(Box::new(bridge.clone())),
        Some(&service_events),
    )?;
    validate_model_capabilities(model.as_ref())?;
    let mut runtime = runtime_snapshot_from_plan(plan);
    let custom_knowledge = activate_custom_knowledge_runtime_for_plan(
        plan,
        &runtime,
        Some(bridge.clone()),
        Some(&service_events),
    );
    apply_custom_knowledge_activation_to_runtime(&mut runtime, &custom_knowledge);
    let custom_memory = activate_custom_memory_runtime_for_plan(
        plan,
        &runtime,
        Some(bridge.clone()),
        Some(&service_events),
    );
    apply_custom_memory_activation_to_runtime(&mut runtime, &custom_memory);
    let mcp_import_activation = activate_mcp_import_runtime_for_plan(plan);
    apply_mcp_import_activation_to_runtime(&mut runtime, &mcp_import_activation);
    let mcp_import_runtime = Arc::new(Mutex::new(mcp_import_activation.runtime));
    let mut dispatcher = AgentPmActionDispatcher::from_runtime(&runtime)?
        .with_mcp_import_runtime(Arc::clone(&mcp_import_runtime))
        .with_cancellation_token(bridge.cancellation_token());
    let mut knowledge = knowledge_runtime_for_machine_plan(
        plan,
        &runtime,
        custom_knowledge.runtime,
        bridge,
        Some(&service_events),
    );
    let mut hooks = ConfiguredHookRuntime::from_config(
        &plan.workspace_root,
        &plan.config.config.hooks.bindings,
        &plan.config.config.hooks.implementations,
        Some(service_events.emitter()),
    )?
    .with_host_invoker(Box::new(bridge.clone()));
    hooks.add_sdk_host_registrations(bridge.sdk_host_hooks());
    let run_id = allocate_harness_run_id();
    let output_paths = RunOutputPaths::resolve(&plan.state_dir, &run_id, None)?;
    let trace_sink_id = if plan.config.config.trace.enabled {
        Some(session.emitter.add_sink(Box::new(JsonlTraceSink::create(
            &output_paths.events_path,
            plan.config.config.trace.clone(),
        )?)))
    } else {
        None
    };
    let result = (|| -> Result<RuntimeTerminalResult> {
        runtime.session_id = session.session_id.clone();
        session.runtime_snapshot = runtime;
        let mcp_import_snapshots = session.runtime_snapshot.mcp_imports.clone();
        emit_mcp_import_activation_events(session, &mcp_import_snapshots)?;
        session.runtime_snapshot.mcp_exports = mcp_exports.snapshots();
        let mut approvals = if bridge.has_sdk_approval_controller() {
            Box::new(SdkHostApprovalController {
                invoker: Box::new(bridge.clone()),
                request_timeout_ms: plan
                    .config
                    .config
                    .approvals
                    .timeout_ms
                    .unwrap_or(SDK_HOST_REQUEST_TIMEOUT_MS),
            }) as Box<dyn ApprovalController>
        } else {
            approval_controller_from_plan(
                plan,
                Some(Box::new(bridge.clone())),
                Some(&service_events),
            )?
        };
        let loop_manifest = load_plan_loop(plan)?;
        let engine_options = harness_engine_options_from_plan(plan);
        let mut engine = HarnessEngine::new(loop_manifest, engine_options);
        engine.set_control_ingress(Box::new(bridge.clone()));
        let memory_embedding_provider =
            embedding_provider_for_plan(plan, Some(bridge.clone()), Some(&service_events));
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
        let result = engine.execute_run_with_id(session, run_id, input, &mut services)?;
        let HarnessRunResult::Terminal(result) = result else {
            bail!(
                "machine surface cannot retain pending approval without an interactive host controller"
            );
        };
        let mut terminal = *result;
        merge_mcp_report_summaries(
            &mut terminal.report.mcp_summaries,
            mcp_exports.report_summaries(),
        );
        if plan.config.config.trace.enabled {
            terminal.report.trace_path = Some(output_paths.events_path.display().to_string());
        }
        terminal
            .report
            .write_pretty(&output_paths.report_path, &plan.config.config.trace.content)?;
        session.emitter.flush()?;
        Ok(terminal)
    })();
    if let Some(trace_sink_id) = trace_sink_id {
        session.emitter.remove_sink(trace_sink_id)?;
    }
    result
}

const AGENTPM_HARNESS_MACHINE_PROTOCOL: &str = "agentpm-harness-machine";
const AGENTPM_HARNESS_MACHINE_VERSION: u8 = 1;
const SDK_HOST_REQUEST_TIMEOUT_MS: u64 = 120_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MachineEnvelope {
    protocol: String,
    version: u8,
    kind: MachineFrameKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    method: Option<String>,
    #[serde(default)]
    payload: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<MachineError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MachineFrameKind {
    Request,
    Response,
    Event,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MachineError {
    code: String,
    message: String,
}

#[derive(Debug, Clone, Deserialize)]
struct MachineMemoryOperationRequest {
    package: String,
    operation: String,
    #[serde(default)]
    current_resolved_scope: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct HostServiceRegistration {
    role: String,
    registry_id: String,
}

fn harness_engine_options_from_plan(plan: &ResolvedHarnessPlan) -> HarnessEngineOptions {
    let mut options = HarnessEngineOptions::new(plan.config.config.runtime.limits.clone());
    if let Some(write_review) = &plan.config.config.memory.write_review {
        options = options.with_memory_write_review_points(write_review.points.clone());
    }
    options
}

#[derive(Clone)]
struct MachineHostBridgeHandle {
    inner: Arc<Mutex<MachineHostBridge>>,
}

struct MachineHostBridge {
    writer: MachineProtocolWriter,
    receiver: mpsc::Receiver<std::result::Result<MachineEnvelope, String>>,
    pending: VecDeque<MachineEnvelope>,
    registered_host_services: BTreeSet<(String, String)>,
    host_service_capabilities: BTreeMap<(String, String), Value>,
    host_service_lifecycle: Option<ServiceLifecycleEmitter>,
    sdk_host_hooks: Vec<SdkHostHookRegistration>,
    sdk_approval_controller: bool,
    pending_memory_operation_control: Option<MachineEnvelope>,
    memory_operation_control_in_flight: bool,
    request_counter: u64,
    active_run: Arc<AtomicBool>,
    cancellation_requested: Arc<AtomicBool>,
}

impl MachineHostBridgeHandle {
    fn new(
        writer: MachineProtocolWriter,
        receiver: mpsc::Receiver<std::result::Result<MachineEnvelope, String>>,
        cancellation_requested: Arc<AtomicBool>,
        active_run: Arc<AtomicBool>,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(MachineHostBridge {
                writer,
                receiver,
                pending: VecDeque::new(),
                registered_host_services: BTreeSet::new(),
                host_service_capabilities: BTreeMap::new(),
                host_service_lifecycle: None,
                sdk_host_hooks: Vec::new(),
                sdk_approval_controller: false,
                pending_memory_operation_control: None,
                memory_operation_control_in_flight: false,
                request_counter: 0,
                active_run,
                cancellation_requested,
            })),
        }
    }

    fn recv_control_request(&self) -> Result<Option<MachineEnvelope>> {
        self.inner
            .lock()
            .expect("machine bridge poisoned")
            .recv_control_request()
    }

    fn register_host_service(&self, service: &HostServiceRegistration, capabilities: Value) {
        let mut bridge = self.inner.lock().expect("machine bridge poisoned");
        let key = (service.role.clone(), service.registry_id.clone());
        bridge.registered_host_services.insert(key.clone());
        bridge.host_service_capabilities.insert(key, capabilities);
    }

    fn set_host_service_lifecycle_emitter(&self, emitter: ServiceLifecycleEmitter) {
        self.inner
            .lock()
            .expect("machine bridge poisoned")
            .host_service_lifecycle = Some(emitter);
    }

    fn register_sdk_host_hooks(&self, registrations: Vec<SdkHostHookRegistration>) {
        self.inner
            .lock()
            .expect("machine bridge poisoned")
            .sdk_host_hooks
            .extend(registrations);
    }

    fn register_sdk_approval_controller(&self) {
        self.inner
            .lock()
            .expect("machine bridge poisoned")
            .sdk_approval_controller = true;
    }

    fn sdk_host_hooks(&self) -> Vec<SdkHostHookRegistration> {
        self.inner
            .lock()
            .expect("machine bridge poisoned")
            .sdk_host_hooks
            .clone()
    }

    fn has_sdk_approval_controller(&self) -> bool {
        self.inner
            .lock()
            .expect("machine bridge poisoned")
            .sdk_approval_controller
    }

    fn has_host_service(&self, service: &HostServiceRegistration) -> bool {
        self.inner
            .lock()
            .expect("machine bridge poisoned")
            .registered_host_services
            .contains(&(service.role.clone(), service.registry_id.clone()))
    }

    fn host_service_capabilities(&self, role: &str, registry_id: &str) -> Option<Value> {
        self.inner
            .lock()
            .expect("machine bridge poisoned")
            .host_service_capabilities
            .get(&(role.to_string(), registry_id.to_string()))
            .cloned()
    }

    fn set_active_run(&self, active_run: bool) {
        let bridge = self.inner.lock().expect("machine bridge poisoned");
        bridge.active_run.store(active_run, Ordering::SeqCst);
        if active_run {
            bridge.cancellation_requested.store(false, Ordering::SeqCst);
        }
    }

    fn request_cancellation(&self) {
        self.inner
            .lock()
            .expect("machine bridge poisoned")
            .cancellation_requested
            .store(true, Ordering::SeqCst);
    }

    fn cancellation_token(&self) -> Arc<AtomicBool> {
        self.inner
            .lock()
            .expect("machine bridge poisoned")
            .cancellation_requested
            .clone()
    }

    fn write_response(&self, id: Option<&str>, payload: Value) -> Result<()> {
        self.inner
            .lock()
            .expect("machine bridge poisoned")
            .writer
            .write_response(id, payload)
    }

    fn write_event_payload(&self, id: Option<&str>, label: &str, payload: Value) -> Result<()> {
        self.inner
            .lock()
            .expect("machine bridge poisoned")
            .writer
            .write_event_payload(id, label, payload)
    }

    fn write_error(
        &self,
        id: Option<&str>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<()> {
        self.inner
            .lock()
            .expect("machine bridge poisoned")
            .writer
            .write_error(id, code, message)
    }

    fn take_memory_operation_control(&self) -> Result<Option<MachineEnvelope>> {
        self.inner
            .lock()
            .expect("machine bridge poisoned")
            .take_memory_operation_control()
    }

    fn complete_memory_operation_control(
        &self,
        id: Option<&str>,
        result: Result<MemoryOperationInvocationResult>,
    ) -> Result<()> {
        self.inner
            .lock()
            .expect("machine bridge poisoned")
            .complete_memory_operation_control(id, result)
    }

    fn flush_memory_operation_controls(&self, code: &str, message: &str) -> Result<()> {
        self.inner
            .lock()
            .expect("machine bridge poisoned")
            .flush_memory_operation_controls(code, message)
    }
}

impl HostServiceInvoker for MachineHostBridgeHandle {
    fn invoke_host_service(
        &mut self,
        role: &str,
        registry_id: &str,
        method: &str,
        payload: Value,
        timeout_ms: u64,
    ) -> Result<Value> {
        self.inner
            .lock()
            .expect("machine bridge poisoned")
            .invoke_host_service(role, registry_id, method, payload, timeout_ms)
    }

    fn host_service_capabilities(&self, role: &str, registry_id: &str) -> Option<Value> {
        self.host_service_capabilities(role, registry_id)
    }

    fn emits_lifecycle_events(&self) -> bool {
        true
    }
}

impl EngineControlIngress for MachineHostBridgeHandle {
    fn service_memory_operation_controls(
        &mut self,
        engine: &mut HarnessEngine,
        session: &mut HarnessSession,
        model: &mut dyn ModelRuntime,
        hooks: &mut dyn HookRuntime,
    ) -> Result<()> {
        while let Some(frame) = self.take_memory_operation_control()? {
            let result = match serde_json::from_value::<MachineMemoryOperationRequest>(
                frame.payload.clone(),
            ) {
                Ok(request) => engine.invoke_memory_operation(
                    session,
                    &request.package,
                    &request.operation,
                    request.current_resolved_scope,
                    model,
                    hooks,
                ),
                Err(err) => Err(MemoryOperationControlError {
                    code: "memory_operation_invalid_request",
                    message: format!("invalid memory_operation request payload: {err}"),
                }
                .into()),
            };
            self.complete_memory_operation_control(frame.id.as_deref(), result)?;
        }
        Ok(())
    }

    fn flush_memory_operation_controls(&mut self, code: &str, message: &str) -> Result<()> {
        MachineHostBridgeHandle::flush_memory_operation_controls(self, code, message)
    }
}

impl MachineHostBridge {
    fn recv_control_request(&mut self) -> Result<Option<MachineEnvelope>> {
        loop {
            let Some(frame) = self.recv_next_frame(None)? else {
                return Ok(None);
            };
            let id = frame.id.clone();
            if let Err(err) = validate_machine_request(&frame) {
                self.writer
                    .write_error(id.as_deref(), "protocol_error", err)?;
                continue;
            }
            return Ok(Some(frame));
        }
    }

    fn take_memory_operation_control(&mut self) -> Result<Option<MachineEnvelope>> {
        if self.pending_memory_operation_control.is_none() {
            self.collect_available_active_control_frames()?;
        }
        if self.memory_operation_control_in_flight {
            return Ok(None);
        }
        let Some(frame) = self.pending_memory_operation_control.take() else {
            return Ok(None);
        };
        self.memory_operation_control_in_flight = true;
        Ok(Some(frame))
    }

    fn complete_memory_operation_control(
        &mut self,
        id: Option<&str>,
        result: Result<MemoryOperationInvocationResult>,
    ) -> Result<()> {
        self.memory_operation_control_in_flight = false;
        match result {
            Ok(result) => self.writer.write_response(id, json!(result)),
            Err(err) => {
                if self.cancellation_requested.load(Ordering::SeqCst) {
                    return self.writer.write_error(
                        id,
                        "memory_operation_cancelled",
                        "external Memory operation was cancelled with the active Run",
                    );
                }
                if let Some(control_error) = err.downcast_ref::<MemoryOperationControlError>() {
                    self.writer
                        .write_error(id, control_error.code, control_error.message.clone())
                } else {
                    self.writer.write_error(
                        id,
                        "memory_operation_failed",
                        format!("external Memory operation failed: {err}"),
                    )
                }
            }
        }
    }

    fn flush_memory_operation_controls(&mut self, code: &str, message: &str) -> Result<()> {
        if let Some(frame) = self.pending_memory_operation_control.take() {
            self.writer
                .write_error(frame.id.as_deref(), code, message)?;
        }
        if self.memory_operation_control_in_flight
            && self.cancellation_requested.load(Ordering::SeqCst)
        {
            self.memory_operation_control_in_flight = false;
        }
        Ok(())
    }

    fn collect_available_active_control_frames(&mut self) -> Result<()> {
        loop {
            let frame = match self.receiver.try_recv() {
                Ok(Ok(frame)) => frame,
                Ok(Err(message)) => {
                    self.writer.write_error(None, "malformed_json", message)?;
                    continue;
                }
                Err(mpsc::TryRecvError::Empty) => return Ok(()),
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            };
            if let Err(err) = validate_machine_frame_base(&frame) {
                self.writer
                    .write_error(frame.id.as_deref(), "protocol_error", err)?;
                continue;
            }
            if frame.kind == MachineFrameKind::Request && self.active_run.load(Ordering::SeqCst) {
                self.handle_control_request_during_active_run(frame)?;
            } else {
                self.pending.push_back(frame);
            }
        }
    }

    fn invoke_host_service(
        &mut self,
        role: &str,
        registry_id: &str,
        method: &str,
        payload: Value,
        timeout_ms: u64,
    ) -> Result<Value> {
        if !self
            .registered_host_services
            .contains(&(role.to_string(), registry_id.to_string()))
        {
            bail!("host service `{role}:{registry_id}` is not registered");
        }
        self.request_counter += 1;
        let request_id = format!("host-{role}-{registry_id}-{}", self.request_counter);
        self.writer.write_unredacted(MachineEnvelope {
            protocol: AGENTPM_HARNESS_MACHINE_PROTOCOL.into(),
            version: AGENTPM_HARNESS_MACHINE_VERSION,
            kind: MachineFrameKind::Request,
            id: Some(request_id.clone()),
            method: Some("host_service".into()),
            payload: json!({
                "role": role,
                "registry_id": registry_id,
                "method": method,
                "payload": payload,
            }),
            error: None,
        })?;

        let timeout = Duration::from_millis(timeout_ms);
        let started = Instant::now();
        loop {
            if self.cancellation_requested.load(Ordering::SeqCst) {
                bail!("run cancellation requested");
            }
            let remaining = match timeout.checked_sub(started.elapsed()) {
                Some(remaining) => remaining,
                None => {
                    let message = format!(
                        "host service request `{request_id}` timed out after {} ms",
                        timeout.as_millis()
                    );
                    self.emit_host_service_failure(role, registry_id, message.clone());
                    bail!(message);
                }
            };
            let Some(frame) = self.recv_next_frame(Some(remaining))? else {
                let message =
                    format!("machine protocol stdin closed while waiting for `{request_id}`");
                self.emit_host_service_failure(role, registry_id, message.clone());
                bail!(message);
            };
            if let Err(err) = validate_machine_frame_base(&frame) {
                self.writer
                    .write_error(frame.id.as_deref(), "protocol_error", err)?;
                continue;
            }
            if frame.id.as_deref() == Some(request_id.as_str()) {
                return match frame.kind {
                    MachineFrameKind::Response => Ok(frame.payload),
                    MachineFrameKind::Error => {
                        let error = frame.error.unwrap_or(MachineError {
                            code: "host_service_error".into(),
                            message: "host service returned an error frame without payload".into(),
                        });
                        let message = format!("{}: {}", error.code, error.message);
                        self.emit_host_service_failure(role, registry_id, message.clone());
                        Err(anyhow!(message))
                    }
                    other => {
                        let message = format!(
                            "host service response `{request_id}` used invalid frame kind `{other:?}`"
                        );
                        self.emit_host_service_failure(role, registry_id, message.clone());
                        Err(anyhow!(message))
                    }
                };
            }
            if frame.kind == MachineFrameKind::Request && self.active_run.load(Ordering::SeqCst) {
                self.handle_control_request_during_active_run(frame)?;
            } else {
                self.pending.push_back(frame);
            }
        }
    }

    fn emit_host_service_failure(&self, role: &str, registry_id: &str, message: impl Into<String>) {
        let message = message.into();
        if let Some(events) = &self.host_service_lifecycle {
            events.emit(
                crate::harness_observability::HarnessEventType::ServiceUnhealthy,
                role,
                registry_id,
                "unhealthy",
                format!("Host service request failed: {message}"),
            );
            events.emit(
                crate::harness_observability::HarnessEventType::ServiceFailed,
                role,
                registry_id,
                "failed",
                format!("Host service request failed: {message}"),
            );
        }
    }

    fn handle_control_request_during_active_run(&mut self, frame: MachineEnvelope) -> Result<()> {
        let id = frame.id.clone();
        match frame.method.as_deref().unwrap_or_default() {
            "cancel_run" => {
                self.cancellation_requested.store(true, Ordering::SeqCst);
                self.flush_memory_operation_controls(
                    "memory_operation_cancelled",
                    "external Memory operation was cancelled with the active Run",
                )?;
                self.writer.write_response(
                    id.as_deref(),
                    json!({
                        "accepted": true,
                        "status": HarnessTerminalStatus::Cancelled,
                    }),
                )?;
            }
            "start_run" => {
                self.writer.write_error(
                    id.as_deref(),
                    "session_busy",
                    "a Harness Run is already active in this Session",
                )?;
            }
            "preflight" => {
                self.writer.write_error(
                    id.as_deref(),
                    "run_active",
                    "preflight control is unavailable while a Run is active",
                )?;
            }
            "memory_operation" => {
                self.enqueue_memory_operation_control(frame)?;
            }
            other => {
                self.writer.write_error(
                    id.as_deref(),
                    "run_active",
                    format!("machine request `{other}` is unavailable while a Run is active"),
                )?;
            }
        }
        Ok(())
    }

    fn enqueue_memory_operation_control(&mut self, frame: MachineEnvelope) -> Result<()> {
        if self.memory_operation_control_in_flight
            || self.pending_memory_operation_control.is_some()
        {
            return self.writer.write_error(
                frame.id.as_deref(),
                "memory_operation_busy",
                "another external Memory operation is already pending or running in this Session",
            );
        }
        self.pending_memory_operation_control = Some(frame);
        Ok(())
    }

    fn recv_next_frame(&mut self, timeout: Option<Duration>) -> Result<Option<MachineEnvelope>> {
        if let Some(frame) = self.pending.pop_front() {
            return Ok(Some(frame));
        }
        loop {
            let received = match timeout {
                Some(timeout) => match self.receiver.recv_timeout(timeout) {
                    Ok(received) => received,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        bail!("timed out waiting for machine frame")
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(None),
                },
                None => match self.receiver.recv() {
                    Ok(received) => received,
                    Err(_) => return Ok(None),
                },
            };
            match received {
                Ok(frame) => return Ok(Some(frame)),
                Err(message) => {
                    self.writer.write_error(None, "malformed_json", message)?;
                    if timeout.is_some() {
                        continue;
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
struct MachineProtocolWriter {
    output: MachineProtocolOutput,
    content: crate::harness_config::HarnessTraceContent,
}

#[derive(Clone)]
enum MachineProtocolOutput {
    Stdout(Arc<Mutex<std::io::Stdout>>),
    #[cfg(test)]
    Buffer(Arc<Mutex<Vec<u8>>>),
}

impl MachineProtocolWriter {
    fn stdout(content: crate::harness_config::HarnessTraceContent) -> Self {
        Self {
            output: MachineProtocolOutput::Stdout(Arc::new(Mutex::new(std::io::stdout()))),
            content,
        }
    }

    #[cfg(test)]
    fn buffer(content: crate::harness_config::HarnessTraceContent) -> (Self, Arc<Mutex<Vec<u8>>>) {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                output: MachineProtocolOutput::Buffer(Arc::clone(&buffer)),
                content,
            },
            buffer,
        )
    }

    fn write_response(&self, id: Option<&str>, payload: Value) -> Result<()> {
        self.write(MachineEnvelope {
            protocol: AGENTPM_HARNESS_MACHINE_PROTOCOL.into(),
            version: AGENTPM_HARNESS_MACHINE_VERSION,
            kind: MachineFrameKind::Response,
            id: id.map(str::to_string),
            method: None,
            payload,
            error: None,
        })
    }

    fn write_event_payload(&self, id: Option<&str>, label: &str, payload: Value) -> Result<()> {
        self.write(MachineEnvelope {
            protocol: AGENTPM_HARNESS_MACHINE_PROTOCOL.into(),
            version: AGENTPM_HARNESS_MACHINE_VERSION,
            kind: MachineFrameKind::Event,
            id: id.map(str::to_string),
            method: Some(label.into()),
            payload,
            error: None,
        })
    }

    fn write_error(
        &self,
        id: Option<&str>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<()> {
        self.write(MachineEnvelope {
            protocol: AGENTPM_HARNESS_MACHINE_PROTOCOL.into(),
            version: AGENTPM_HARNESS_MACHINE_VERSION,
            kind: MachineFrameKind::Error,
            id: id.map(str::to_string),
            method: None,
            payload: Value::Null,
            error: Some(MachineError {
                code: code.into(),
                message: message.into(),
            }),
        })
    }

    fn write(&self, envelope: MachineEnvelope) -> Result<()> {
        let envelope = self.frame_value(envelope, true)?;
        self.write_value(envelope)
    }

    fn write_unredacted(&self, envelope: MachineEnvelope) -> Result<()> {
        let envelope = self.frame_value(envelope, false)?;
        self.write_value(envelope)
    }

    fn frame_value(&self, envelope: MachineEnvelope, redact: bool) -> Result<Value> {
        let mut envelope = serde_json::to_value(envelope).context("serializing machine frame")?;
        if redact {
            apply_content_policy_to_value(&mut envelope, &self.content);
        }
        Ok(envelope)
    }

    fn write_value(&self, envelope: Value) -> Result<()> {
        match &self.output {
            MachineProtocolOutput::Stdout(stdout) => {
                let mut stdout = stdout.lock().expect("machine stdout poisoned");
                serde_json::to_writer(&mut *stdout, &envelope).context("writing machine frame")?;
                stdout
                    .write_all(b"\n")
                    .context("writing machine frame newline")?;
                stdout.flush().context("flushing machine stdout")
            }
            #[cfg(test)]
            MachineProtocolOutput::Buffer(buffer) => {
                let mut buffer = buffer.lock().expect("machine buffer poisoned");
                serde_json::to_writer(&mut *buffer, &envelope).context("writing machine frame")?;
                buffer
                    .write_all(b"\n")
                    .context("writing machine frame newline")
            }
        }
    }
}

struct MachineEventSink {
    writer: MachineProtocolWriter,
}

impl MachineEventSink {
    fn new(writer: MachineProtocolWriter) -> Self {
        Self { writer }
    }
}

impl HarnessEventSink for MachineEventSink {
    fn record(&mut self, event: &HarnessEventEnvelope) -> Result<()> {
        let event = apply_content_policy(event, &self.writer.content)?;
        self.writer.write(MachineEnvelope {
            protocol: AGENTPM_HARNESS_MACHINE_PROTOCOL.into(),
            version: AGENTPM_HARNESS_MACHINE_VERSION,
            kind: MachineFrameKind::Event,
            id: event.correlation_id.clone(),
            method: Some("harness_event".into()),
            payload: serde_json::to_value(event).context("serializing machine event")?,
            error: None,
        })
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

fn spawn_machine_stdin_reader(
    writer: MachineProtocolWriter,
    active_run: Arc<AtomicBool>,
    cancellation_requested: Arc<AtomicBool>,
) -> mpsc::Receiver<std::result::Result<MachineEnvelope, String>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            let line = match line {
                Ok(line) => line,
                Err(err) => {
                    let _ = sender.send(Err(format!("reading machine protocol stdin: {err}")));
                    break;
                }
            };
            if line.trim().is_empty() {
                continue;
            }
            let parsed = serde_json::from_str::<MachineEnvelope>(&line);
            if let Ok(frame) = &parsed {
                if validate_machine_request(frame).is_ok()
                    && frame.method.as_deref() == Some("cancel_run")
                {
                    cancellation_requested.store(true, Ordering::SeqCst);
                    let _ = writer.write_response(
                        frame.id.as_deref(),
                        json!({
                            "status": HarnessTerminalStatus::Cancelled,
                            "accepted": true,
                        }),
                    );
                    continue;
                }
                if active_run.load(Ordering::SeqCst)
                    && validate_machine_request(frame).is_ok()
                    && frame.method.as_deref() == Some("start_run")
                {
                    let _ = writer.write_error(
                        frame.id.as_deref(),
                        "session_busy",
                        "a Harness Run is already active in this Session",
                    );
                    continue;
                }
            }
            let parsed = parsed.map_err(|err| format!("invalid JSON frame: {err}"));
            if sender.send(parsed).is_err() {
                break;
            }
        }
    });
    receiver
}

fn validate_machine_frame_base(frame: &MachineEnvelope) -> std::result::Result<(), String> {
    if frame.protocol != AGENTPM_HARNESS_MACHINE_PROTOCOL {
        return Err(format!(
            "unsupported protocol `{}`; expected `{AGENTPM_HARNESS_MACHINE_PROTOCOL}`",
            frame.protocol
        ));
    }
    if frame.version != AGENTPM_HARNESS_MACHINE_VERSION {
        return Err(format!(
            "unsupported protocol version {}; expected {AGENTPM_HARNESS_MACHINE_VERSION}",
            frame.version
        ));
    }
    Ok(())
}

fn validate_machine_request(request: &MachineEnvelope) -> std::result::Result<(), String> {
    validate_machine_frame_base(request)?;
    if request.kind != MachineFrameKind::Request {
        return Err("machine input frames must use kind `request`".into());
    }
    if request.method.is_none() {
        return Err("machine request is missing method".into());
    }
    Ok(())
}

fn run_headless_surface(plan: &ResolvedHarnessPlan, args: &HarnessArgs) -> Result<()> {
    let input = read_run_input(args.input.as_deref(), args.input_file.as_ref())?;
    let selection = model_selection(plan)?;
    let mut service_events = ServiceLifecycleEvents::new();
    let mut model = model_runtime_from_plan(plan, selection, None, Some(&service_events))?;
    validate_model_capabilities(model.as_ref())?;
    let runtime = runtime_snapshot_from_plan(plan);
    let mut dispatcher = AgentPmActionDispatcher::from_runtime(&runtime)?;
    let mut hooks = ConfiguredHookRuntime::from_config(
        &plan.workspace_root,
        &plan.config.config.hooks.bindings,
        &plan.config.config.hooks.implementations,
        Some(service_events.emitter()),
    )?;
    let terminal = execute_headless_plan_with_hooks(
        plan,
        input,
        args.report.as_ref(),
        model.as_mut(),
        &mut dispatcher,
        &mut hooks,
        Some(&mut service_events),
    )?;
    print_memory_write_review_warnings(&terminal);
    match terminal.status {
        crate::harness_observability::HarnessTerminalStatus::Ended
        | crate::harness_observability::HarnessTerminalStatus::HandedOff => {
            print_terminal_output(&terminal)?;
            Ok(())
        }
        status => bail!("{}", terminal_status_error_message(&terminal, status)?),
    }
}

fn model_runtime_from_plan(
    plan: &ResolvedHarnessPlan,
    selection: ModelProviderSelection,
    host_invoker: Option<Box<dyn HostServiceInvoker>>,
    service_events: Option<&ServiceLifecycleEvents>,
) -> Result<Box<dyn ModelRuntime>> {
    if let Some(entry) = plan.config.config.providers.models.get(&selection.provider) {
        return match &entry.implementation {
            crate::harness_config::HarnessImplementation::Process { .. } => {
                let runtime = ProcessModelRuntime::start(
                    selection,
                    entry.implementation.clone(),
                    plan.workspace_root.clone(),
                    service_events.map(ServiceLifecycleEvents::emitter),
                )
                .map_err(|err| anyhow!(err.message))?;
                Ok(Box::new(runtime))
            }
            crate::harness_config::HarnessImplementation::Host { request_timeout_ms } => {
                let Some(invoker) = host_invoker else {
                    bail!(
                        "model provider `{}` uses a host implementation and requires `agentpm harness --machine`",
                        selection.provider
                    );
                };
                let capabilities = host_model_capabilities_from_registration(
                    &invoker
                        .host_service_capabilities("model", &selection.provider)
                        .unwrap_or_else(|| json!({})),
                    &selection.provider,
                    &selection.model,
                )?;
                Ok(Box::new(HostModelRuntime {
                    selection,
                    invoker,
                    capabilities,
                    request_timeout_ms: *request_timeout_ms,
                }))
            }
        };
    }
    BuiltInModelRuntime::from_selection(selection)
        .map(|runtime| Box::new(runtime) as Box<dyn ModelRuntime>)
        .map_err(|err| anyhow!(err.message))
}

struct HostModelRuntime {
    selection: ModelProviderSelection,
    invoker: Box<dyn HostServiceInvoker>,
    capabilities: ModelCapabilityAdvertisement,
    request_timeout_ms: u64,
}

impl ModelRuntime for HostModelRuntime {
    fn capabilities(&self) -> ModelCapabilityAdvertisement {
        self.capabilities.clone()
    }

    fn inspect_request(&self, request: &ModelRequest) -> Option<ModelRuntimeRequestSnapshot> {
        let selection = request
            .model
            .clone()
            .unwrap_or_else(|| self.selection.clone());
        let prompt = request.prompt.render_text();
        Some(ModelRuntimeRequestSnapshot {
            runtime_kind: "host".into(),
            request_kind: "canonical_model_request".into(),
            provider: selection.provider,
            model: selection.model,
            action_descriptors: request.prompt.action_aliases.len(),
            structured_actions: None,
            capability_catalog_in_prompt: request.prompt.has_capability_catalog_section(),
            action_aliases: request.prompt.action_aliases.clone(),
            turn_strategy: "canonical_request".into(),
            ordered_turns: request.ordered_turns.clone(),
            diagnostics: request.prompt.diagnostics.clone(),
            prompt,
        })
    }

    fn generate(
        &mut self,
        request: ModelRequest,
    ) -> std::result::Result<ModelTurn, ModelRuntimeFailure> {
        let selection = request
            .model
            .clone()
            .unwrap_or_else(|| self.selection.clone());
        let payload = self
            .invoker
            .invoke_host_service(
                "model",
                &self.selection.provider,
                "generate",
                json!({
                    "selection": selection,
                    "request": request,
                }),
                self.request_timeout_ms,
            )
            .map_err(|err| ModelRuntimeFailure::new(err.to_string()))?;
        serde_json::from_value(payload)
            .map_err(|err| ModelRuntimeFailure::new(format!("invalid host model response: {err}")))
    }
}

#[derive(Default, Deserialize)]
struct HostModelCapabilityAdvertisement {
    provider: Option<String>,
    model: Option<String>,
    semantic_actions: Option<bool>,
    structured_output: Option<bool>,
    multimodal_input: Option<bool>,
    context_window_tokens: Option<u64>,
    usage_reporting: Option<bool>,
}

fn host_model_capabilities_from_registration(
    capabilities: &Value,
    expected_provider: &str,
    expected_model: &str,
) -> Result<ModelCapabilityAdvertisement> {
    if capabilities.is_null() {
        return Ok(ModelCapabilityAdvertisement::default());
    }
    let partial: HostModelCapabilityAdvertisement = serde_json::from_value(capabilities.clone())
        .map_err(|err| anyhow!("invalid host model capabilities: {err}"))?;
    if let Some(provider) = partial.provider
        && provider != expected_provider
    {
        bail!("host model provider advertised `{provider}`, expected `{expected_provider}`");
    }
    if let Some(model) = partial.model
        && model != expected_model
    {
        bail!("host model advertised model `{model}`, expected `{expected_model}`");
    }
    let mut advertisement = ModelCapabilityAdvertisement::default();
    if let Some(semantic_actions) = partial.semantic_actions {
        advertisement.semantic_actions = semantic_actions;
    }
    if let Some(structured_output) = partial.structured_output {
        advertisement.structured_output = structured_output;
    }
    if let Some(multimodal_input) = partial.multimodal_input {
        advertisement.multimodal_input = multimodal_input;
    }
    if let Some(context_window_tokens) = partial.context_window_tokens {
        advertisement.context_window_tokens = Some(context_window_tokens);
    }
    if let Some(usage_reporting) = partial.usage_reporting {
        advertisement.usage_reporting = usage_reporting;
    }
    Ok(advertisement)
}

fn validate_model_capabilities(model: &dyn ModelRuntime) -> Result<()> {
    let capabilities = model.capabilities();
    if !capabilities.semantic_actions {
        bail!("selected model runtime does not advertise Harness semantic action support");
    }
    if !capabilities.structured_output {
        bail!(
            "selected model runtime does not advertise structured output support required by Harness"
        );
    }
    Ok(())
}

fn approval_controller_from_plan(
    plan: &ResolvedHarnessPlan,
    host_invoker: Option<Box<dyn HostServiceInvoker>>,
    service_events: Option<&ServiceLifecycleEvents>,
) -> Result<Box<dyn ApprovalController>> {
    let Some(controller) = &plan.config.config.approvals.controller else {
        return Ok(Box::new(HeadlessApprovalController));
    };
    match &controller.implementation {
        crate::harness_config::HarnessImplementation::Process { .. } => {
            let Some(controller) = ConfiguredApprovalController::process(
                &plan.workspace_root,
                controller,
                plan.config.config.approvals.timeout_ms,
                service_events.map(ServiceLifecycleEvents::emitter),
            )?
            else {
                unreachable!("process approval controller returned no process runtime");
            };
            Ok(Box::new(controller))
        }
        crate::harness_config::HarnessImplementation::Host { .. } => {
            let Some(invoker) = host_invoker else {
                bail!(
                    "approval controller uses a host implementation and requires `agentpm harness --machine`"
                );
            };
            let Some(controller) = ConfiguredApprovalController::host(
                controller,
                plan.config.config.approvals.timeout_ms,
                invoker,
            )?
            else {
                unreachable!("host approval controller returned no host runtime");
            };
            Ok(Box::new(controller))
        }
    }
}

fn print_terminal_output(terminal: &RuntimeTerminalResult) -> Result<()> {
    let Some(output) = &terminal.output else {
        return Ok(());
    };
    if let Some(text) = output.as_str() {
        println!("{text}");
    } else {
        println!("{}", serde_json::to_string_pretty(output)?);
    }
    Ok(())
}

fn print_memory_write_review_warnings(terminal: &RuntimeTerminalResult) {
    for warning in terminal_memory_write_review_warning_lines(terminal) {
        eprintln!("{warning}");
    }
}

fn terminal_memory_write_review_warning_lines(terminal: &RuntimeTerminalResult) -> Vec<String> {
    memory_write_review_warning_lines(&terminal.report)
}

fn memory_write_review_warning_lines(report: &RunReport) -> Vec<String> {
    report
        .memory_write_review_summaries
        .iter()
        .filter(|summary| summary.status == "failed")
        .map(memory_write_review_warning_line)
        .collect()
}

fn memory_write_review_warning_line(summary: &MemoryWriteReviewReportSummary) -> String {
    format!(
        "Warning: Memory write review at {} {}: {}. Memory writes attempted/completed: {}/{}. Intended Memory may not have been written.",
        summary.point,
        summary.status,
        summary.reason,
        summary.memory_writes_attempted,
        summary.memory_writes_completed
    )
}

fn terminal_status_error_message(
    terminal: &RuntimeTerminalResult,
    status: crate::harness_observability::HarnessTerminalStatus,
) -> Result<String> {
    let base = format!("Harness Run finished with terminal status {status:?}");
    let Some(output) = &terminal.output else {
        return Ok(base);
    };
    if let Some(error) = output.get("error").and_then(|value| value.as_str()) {
        return Ok(format!("{base}: {error}"));
    }
    if let Some(text) = output.as_str() {
        return Ok(format!("{base}: {text}"));
    }
    Ok(format!("{base}: {}", serde_json::to_string(output)?))
}

fn read_run_input(input: Option<&str>, input_file: Option<&PathBuf>) -> Result<String> {
    if let Some(input) = input {
        return Ok(input.to_string());
    }
    if let Some(path) = input_file {
        return std::fs::read_to_string(path)
            .with_context(|| format!("reading Harness input file {}", path.display()));
    }
    let mut buffer = String::new();
    std::io::stdin()
        .read_to_string(&mut buffer)
        .context("reading Harness input from stdin")?;
    if buffer.trim().is_empty() {
        bail!("Harness --headless requires --input, --input-file, or stdin input");
    }
    Ok(buffer)
}

fn model_selection(plan: &ResolvedHarnessPlan) -> Result<ModelProviderSelection> {
    let Some(model) = &plan.config.config.model else {
        bail!("Harness --headless requires model.provider and model.model in agentpm.harness.json");
    };
    Ok(ModelProviderSelection {
        provider: model.provider.clone(),
        model: model.model.clone(),
        options: model.options.clone(),
    })
}

#[cfg(test)]
fn execute_headless_plan(
    plan: &ResolvedHarnessPlan,
    input: String,
    report_override: Option<&PathBuf>,
    model: &mut dyn ModelRuntime,
    dispatcher: &mut dyn ActionDispatcher,
) -> Result<RuntimeTerminalResult> {
    let mut hooks = crate::harness_runtime::NoopHookRuntime;
    execute_headless_plan_with_hooks(
        plan,
        input,
        report_override,
        model,
        dispatcher,
        &mut hooks,
        None,
    )
}

fn execute_headless_plan_with_hooks(
    plan: &ResolvedHarnessPlan,
    input: String,
    report_override: Option<&PathBuf>,
    model: &mut dyn ModelRuntime,
    dispatcher: &mut dyn ActionDispatcher,
    hooks: &mut dyn HookRuntime,
    service_events: Option<&mut ServiceLifecycleEvents>,
) -> Result<RuntimeTerminalResult> {
    let mut approvals = approval_controller_from_plan(plan, None, None)?;
    let mut runtime = runtime_snapshot_from_plan(plan);
    let custom_knowledge = {
        let service_events_ref = service_events.as_deref();
        activate_custom_knowledge_runtime_for_plan(plan, &runtime, None, service_events_ref)
    };
    apply_custom_knowledge_activation_to_runtime(&mut runtime, &custom_knowledge);
    let custom_memory = {
        let service_events_ref = service_events.as_deref();
        activate_custom_memory_runtime_for_plan(plan, &runtime, None, service_events_ref)
    };
    apply_custom_memory_activation_to_runtime(&mut runtime, &custom_memory);
    let mcp_import_activation = activate_mcp_import_runtime_for_plan(plan);
    apply_mcp_import_activation_to_runtime(&mut runtime, &mcp_import_activation);
    let mcp_import_runtime = Arc::new(Mutex::new(mcp_import_activation.runtime));
    let mut knowledge = {
        let service_events_ref = service_events.as_deref();
        knowledge_runtime_for_headless_plan(
            plan,
            &runtime,
            custom_knowledge.runtime,
            service_events_ref,
        )
    };
    let memory_embedding_provider = {
        let service_events_ref = service_events.as_deref();
        embedding_provider_for_plan(plan, None, service_events_ref)
    };
    let mut mcp_dispatcher = McpImportActionDispatcher {
        delegate: dispatcher,
        mcp_imports: Arc::clone(&mcp_import_runtime),
    };
    let mut services = HarnessRuntimeServices {
        model,
        dispatcher: &mut mcp_dispatcher,
        knowledge: knowledge.as_mut(),
        memory: custom_memory.runtime,
        embedding_provider: memory_embedding_provider,
        approvals: approvals.as_mut(),
        hooks,
        service_events,
    };
    execute_headless_plan_with_services(plan, input, report_override, runtime, &mut services)
}

fn execute_headless_plan_with_services(
    plan: &ResolvedHarnessPlan,
    input: String,
    report_override: Option<&PathBuf>,
    runtime: RuntimeSnapshot,
    services: &mut HarnessRuntimeServices<'_>,
) -> Result<RuntimeTerminalResult> {
    let loop_manifest = load_plan_loop(plan)?;
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let run_id = allocate_harness_run_id();
    let output_paths = RunOutputPaths::resolve(
        &plan.state_dir,
        &run_id,
        report_override.map(PathBuf::as_path),
    )?;
    if plan.config.config.trace.enabled {
        session.emitter.add_sink(Box::new(JsonlTraceSink::create(
            &output_paths.events_path,
            plan.config.config.trace.clone(),
        )?));
    }
    let mcp_import_snapshots = session.runtime_snapshot.mcp_imports.clone();
    emit_mcp_import_activation_events(&mut session, &mcp_import_snapshots)?;
    let mut mcp_exports = ManagedMcpExports::start(plan, &mut session)?;
    refresh_mcp_exports_for_session(plan, &mut session, &mut mcp_exports, None)?;
    let engine_options = harness_engine_options_from_plan(plan);
    let mut engine = HarnessEngine::new(loop_manifest, engine_options);
    let result = engine.execute_run_with_id(&mut session, run_id, input, services)?;
    let HarnessRunResult::Terminal(result) = result else {
        bail!("Harness --headless cannot wait for interactive approval");
    };
    let mut terminal = *result;
    refresh_mcp_exports_for_session_without_restart(plan, &mut session, &mut mcp_exports, None)?;
    merge_mcp_report_summaries(
        &mut terminal.report.mcp_summaries,
        mcp_exports.report_summaries(),
    );
    mcp_exports.stop(&mut session)?;
    if plan.config.config.trace.enabled {
        terminal.report.trace_path = Some(output_paths.events_path.display().to_string());
    }
    session.emitter.flush()?;
    terminal
        .report
        .write_pretty(&output_paths.report_path, &plan.config.config.trace.content)?;
    Ok(terminal)
}

fn knowledge_runtime_for_headless_plan(
    plan: &ResolvedHarnessPlan,
    runtime: &RuntimeSnapshot,
    custom: Option<CustomKnowledgeRuntime>,
    service_events: Option<&ServiceLifecycleEvents>,
) -> Box<dyn KnowledgeRuntime> {
    let embedding_provider = embedding_provider_for_plan(plan, None, service_events);
    let local = LocalKnowledgeRuntime::from_runtime(runtime, embedding_provider);
    Box::new(CompositeKnowledgeRuntime::new(
        local,
        custom,
        custom_knowledge_routes(plan),
    ))
}

fn knowledge_runtime_for_machine_plan(
    plan: &ResolvedHarnessPlan,
    runtime: &RuntimeSnapshot,
    custom: Option<CustomKnowledgeRuntime>,
    bridge: &MachineHostBridgeHandle,
    service_events: Option<&ServiceLifecycleEvents>,
) -> Box<dyn KnowledgeRuntime> {
    let embedding_provider =
        embedding_provider_for_plan(plan, Some(bridge.clone()), service_events);
    let local = LocalKnowledgeRuntime::from_runtime(runtime, embedding_provider);
    Box::new(CompositeKnowledgeRuntime::new(
        local,
        custom,
        custom_knowledge_routes(plan),
    ))
}

struct CustomKnowledgeRuntimeActivation {
    runtime: Option<CustomKnowledgeRuntime>,
    unavailable_packages: BTreeMap<String, String>,
}

fn activate_custom_knowledge_runtime_for_plan(
    plan: &ResolvedHarnessPlan,
    runtime: &RuntimeSnapshot,
    host_bridge: Option<MachineHostBridgeHandle>,
    service_events: Option<&ServiceLifecycleEvents>,
) -> CustomKnowledgeRuntimeActivation {
    let routes = custom_knowledge_routes(plan);
    if routes.is_empty() {
        return CustomKnowledgeRuntimeActivation {
            runtime: None,
            unavailable_packages: BTreeMap::new(),
        };
    }
    let mapped_available_packages = runtime
        .knowledge
        .iter()
        .filter(|package| routes.contains_key(&package.name) && package.state == "available")
        .cloned()
        .collect::<Vec<_>>();
    if mapped_available_packages.is_empty() {
        return CustomKnowledgeRuntimeActivation {
            runtime: None,
            unavailable_packages: BTreeMap::new(),
        };
    }
    let mut active_packages = Vec::new();
    let mut unavailable_packages = BTreeMap::new();
    let mut runtimes = HashMap::new();
    for runtime_id in routes.values().cloned().collect::<BTreeSet<_>>() {
        let mapped_packages = mapped_available_packages
            .iter()
            .filter(|package| routes.get(&package.name) == Some(&runtime_id))
            .cloned()
            .collect::<Vec<_>>();
        if mapped_packages.is_empty() {
            continue;
        }
        let Some(entry) = plan.config.config.knowledge.runtimes.get(&runtime_id) else {
            mark_custom_knowledge_runtime_unavailable(
                &mut unavailable_packages,
                &mapped_packages,
                format!("knowledge.packages references undefined KnowledgeRuntime `{runtime_id}`"),
            );
            continue;
        };
        let activation = match &entry.implementation {
            crate::harness_config::HarnessImplementation::Process { .. } => {
                let mut initialize_payload = serde_json::Map::new();
                initialize_payload.insert("packages".into(), json!(&mapped_packages));
                crate::harness_runtime::knowledge::ServiceRuntime::process(
                    "knowledge",
                    &runtime_id,
                    entry,
                    &plan.workspace_root,
                    initialize_payload,
                    service_events.map(ServiceLifecycleEvents::emitter),
                )
                .map(|service_runtime| {
                    let capabilities = service_runtime
                        .initialization_result()
                        .cloned()
                        .unwrap_or_else(|| json!({}));
                    (service_runtime, capabilities)
                })
                .map_err(|err| {
                    anyhow!("configured KnowledgeRuntime `{runtime_id}` could not start: {err}")
                })
            }
            crate::harness_config::HarnessImplementation::Host { request_timeout_ms } => {
                let bridge = host_bridge.clone().ok_or_else(|| {
                    anyhow!(
                        "configured KnowledgeRuntime `{runtime_id}` requires a machine host service"
                    )
                });
                bridge.and_then(|bridge| {
                    let capabilities = bridge
                        .host_service_capabilities("knowledge", &runtime_id)
                        .ok_or_else(|| {
                            anyhow!(
                                "configured KnowledgeRuntime `{runtime_id}` host service is not registered"
                            )
                        })?;
                    Ok((
                        crate::harness_runtime::knowledge::ServiceRuntime::host(
                            Box::new(bridge),
                            *request_timeout_ms,
                        ),
                        capabilities,
                    ))
                })
            }
        };
        let (service_runtime, capabilities) = match activation {
            Ok(activation) => activation,
            Err(err) => {
                mark_custom_knowledge_runtime_unavailable(
                    &mut unavailable_packages,
                    &mapped_packages,
                    err.to_string(),
                );
                continue;
            }
        };
        let mut runtime_active_packages = Vec::new();
        for package in mapped_packages {
            match crate::harness_runtime::knowledge::validate_knowledge_runtime_capabilities(
                &capabilities,
                &runtime_id,
                std::slice::from_ref(&package),
            ) {
                Ok(()) => runtime_active_packages.push(package),
                Err(err) => {
                    unavailable_packages.insert(
                        package.name.clone(),
                        format!(
                            "configured KnowledgeRuntime `{runtime_id}` could not realize {}@{}: {err}",
                            package.name, package.version
                        ),
                    );
                }
            }
        }
        if runtime_active_packages.is_empty() {
            continue;
        }
        active_packages.extend(runtime_active_packages);
        runtimes.insert(runtime_id, service_runtime);
    }
    let runtime = (!active_packages.is_empty())
        .then(|| CustomKnowledgeRuntime::new(active_packages, runtimes, routes));
    CustomKnowledgeRuntimeActivation {
        runtime,
        unavailable_packages,
    }
}

fn mark_custom_knowledge_runtime_unavailable(
    unavailable_packages: &mut BTreeMap<String, String>,
    packages: &[KnowledgeRuntimeSnapshot],
    reason: String,
) {
    for package in packages {
        unavailable_packages.insert(package.name.clone(), reason.clone());
    }
}

fn apply_custom_knowledge_activation_to_runtime(
    runtime: &mut RuntimeSnapshot,
    activation: &CustomKnowledgeRuntimeActivation,
) {
    for package in &mut runtime.knowledge {
        if let Some(reason) = activation.unavailable_packages.get(&package.name) {
            package.state = "unavailable".into();
            package.readiness_reason = Some(reason.clone());
        }
    }
}

fn custom_knowledge_routes(plan: &ResolvedHarnessPlan) -> BTreeMap<String, String> {
    plan.config
        .config
        .knowledge
        .packages
        .iter()
        .map(|(package, mapping)| (package.clone(), mapping.runtime.clone()))
        .collect()
}

fn embedding_provider_for_plan(
    plan: &ResolvedHarnessPlan,
    host_bridge: Option<MachineHostBridgeHandle>,
    service_events: Option<&ServiceLifecycleEvents>,
) -> Option<Box<dyn crate::harness_runtime::EmbeddingProvider>> {
    let mut providers: BTreeMap<String, Box<dyn crate::harness_runtime::EmbeddingProvider>> =
        BTreeMap::new();
    let mut routes = BTreeMap::new();
    let mut route_specs = plan
        .config
        .config
        .knowledge
        .embedding_matches
        .iter()
        .map(|item| {
            (
                item.embedding_provider.clone(),
                item.r#match.provider.clone(),
                item.r#match.model.clone(),
                item.r#match.dimensions,
                item.r#match.normalized,
            )
        })
        .collect::<Vec<_>>();
    if let Some(semantic) = &plan.config.config.memory.local.semantic {
        route_specs.push((
            semantic.embedding_provider.clone(),
            semantic.embedding_provider.clone(),
            semantic.model.clone(),
            semantic.dimensions,
            true,
        ));
    }
    for (embedding_provider_id, provider, model, dimensions, normalized) in route_specs {
        let Some(entry) = plan
            .config
            .config
            .providers
            .embeddings
            .get(&embedding_provider_id)
        else {
            continue;
        };
        routes.insert(
            format!("{provider}\n{model}\n{dimensions}\n{normalized}"),
            embedding_provider_id.clone(),
        );
        if providers.contains_key(&embedding_provider_id) {
            continue;
        }
        let provider: Result<Box<dyn crate::harness_runtime::EmbeddingProvider>> =
            match &entry.implementation {
                crate::harness_config::HarnessImplementation::Process { .. } => {
                    ServiceEmbeddingProvider::process(
                        &plan.workspace_root,
                        &embedding_provider_id,
                        entry,
                        service_events.map(ServiceLifecycleEvents::emitter),
                    )
                    .map(|provider| {
                        Box::new(provider) as Box<dyn crate::harness_runtime::EmbeddingProvider>
                    })
                }
                crate::harness_config::HarnessImplementation::Host { .. } => {
                    let Some(bridge) = host_bridge.clone() else {
                        continue;
                    };
                    ServiceEmbeddingProvider::host(
                        &embedding_provider_id,
                        entry,
                        Box::new(bridge),
                        service_events.map(ServiceLifecycleEvents::emitter),
                    )
                    .map(|provider| {
                        Box::new(provider) as Box<dyn crate::harness_runtime::EmbeddingProvider>
                    })
                }
            };
        if let Ok(provider) = provider {
            providers.insert(embedding_provider_id, provider);
        }
    }
    if providers.is_empty() {
        None
    } else {
        Some(Box::new(RoutingEmbeddingProvider::new(routes, providers)))
    }
}

struct HeadlessApprovalController;

impl ApprovalController for HeadlessApprovalController {
    fn request_approval(
        &mut self,
        _checkpoint: &crate::manifest::LoopCheckpoint,
    ) -> crate::harness_runtime::ApprovalDecision {
        crate::harness_runtime::ApprovalDecision::Pending
    }
}

struct SdkHostApprovalController {
    invoker: Box<dyn HostServiceInvoker>,
    request_timeout_ms: u64,
}

impl ApprovalController for SdkHostApprovalController {
    fn request_approval(
        &mut self,
        checkpoint: &crate::manifest::LoopCheckpoint,
    ) -> crate::harness_runtime::ApprovalDecision {
        match self.invoker.invoke_host_service(
            "approval",
            "controller",
            "request_approval",
            json!({ "checkpoint": checkpoint }),
            self.request_timeout_ms,
        ) {
            Ok(value) => match value
                .get("decision")
                .and_then(Value::as_str)
                .unwrap_or("pending")
            {
                "approve" | "approved" => crate::harness_runtime::ApprovalDecision::Approve,
                "deny" | "denied" => crate::harness_runtime::ApprovalDecision::Deny,
                "pending" => crate::harness_runtime::ApprovalDecision::Pending,
                other => crate::harness_runtime::ApprovalDecision::Failure(format!(
                    "unsupported approval decision `{other}`"
                )),
            },
            Err(err) => crate::harness_runtime::ApprovalDecision::Failure(err.to_string()),
        }
    }
}

fn load_plan_loop(plan: &ResolvedHarnessPlan) -> Result<crate::manifest::LoopManifest> {
    let Some(loop_package) = &plan.loop_package else {
        bail!("Harness plan does not include a resolved Loop package");
    };
    let manifest_path = loop_package.root.join("agent.json");
    let (value, _) = load_manifest_value(&manifest_path)?;
    parse_loop_manifest(&value)
}

fn parse_scope(raw: &str) -> Result<(String, String)> {
    let Some((key, value)) = raw.split_once('=') else {
        return Err(anyhow!("scope must be KEY=VALUE"));
    };
    if key.is_empty() || value.is_empty() {
        return Err(anyhow!("scope key and value must be non-empty"));
    }
    Ok((key.to_string(), value.to_string()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreflightOutputStream {
    Stdout,
    Stderr,
}

impl PreflightOutputStream {
    fn for_surface(surface: HarnessExecutionSurface) -> Self {
        match surface {
            HarnessExecutionSurface::Headless | HarnessExecutionSurface::Machine => Self::Stderr,
            HarnessExecutionSurface::Tui => Self::Stdout,
        }
    }

    fn line(self, line: impl std::fmt::Display) -> Result<()> {
        use std::io::Write;
        match self {
            Self::Stdout => {
                let mut stdout = std::io::stdout().lock();
                writeln!(stdout, "{line}").context("writing Harness preflight to stdout")
            }
            Self::Stderr => {
                let mut stderr = std::io::stderr().lock();
                writeln!(stderr, "{line}").context("writing Harness preflight to stderr")
            }
        }
    }
}

fn print_harness_preflight(
    plan: &ResolvedHarnessPlan,
    surface: HarnessExecutionSurface,
    stream: PreflightOutputStream,
    verbose: bool,
) -> Result<()> {
    stream.line("AgentPM Harness preflight")?;
    stream.line(format!("Workspace: {}", plan.workspace_root.display()))?;
    stream.line(format!("Lockfile: {}", plan.lock_path.display()))?;
    stream.line(format!("State dir: {}", plan.state_dir.display()))?;
    if let Some(config_path) = &plan.config.config_path {
        stream.line(format!("Config: {}", config_path.display()))?;
    } else {
        stream.line("Config: defaults")?;
    }
    if let Some(agent) = &plan.selected_agent {
        stream.line(format!("Agent: {}@{}", agent.name, agent.version))?;
    }
    if let Some(loop_package) = &plan.loop_package {
        stream.line(format!(
            "Loop: {}@{}",
            loop_package.name, loop_package.version
        ))?;
    }
    stream.line(format!("Resolved packages: {}", plan.package_graph.len()))?;
    stream.line(format!("Runtime scopes: {}", plan.runtime_scopes.len()))?;
    match &plan.consumer_context.file {
        Some(file) => {
            let mut line = format!(
                "Consumer context: {file} ({:?})",
                plan.consumer_context.state
            );
            if let Some(byte_size) = plan.consumer_context.byte_size {
                line.push_str(&format!(", {byte_size} bytes"));
            }
            if let Some(approximate_tokens) = plan.consumer_context.approximate_tokens {
                line.push_str(&format!(", ~{approximate_tokens} tokens"));
            }
            if let Some(sha256) = &plan.consumer_context.sha256 {
                line.push_str(&format!(", {sha256}"));
            }
            stream.line(line)?
        }
        None => stream.line("Consumer context: not configured")?,
    }

    let capability_counts = capability_counts(plan);
    if !capability_counts.is_empty() {
        stream.line("")?;
        stream.line("Static capabilities:")?;
        for (state, count) in capability_counts {
            stream.line(format!("- {state}: {count}"))?;
        }
        if verbose {
            let detail_lines = static_capability_detail_lines(plan);
            if !detail_lines.is_empty() {
                stream.line("Static capability details:")?;
                for line in detail_lines {
                    stream.line(line)?;
                }
            }
        }
    }

    let mcp_export_lines = mcp_export_preflight_lines(plan);
    if !mcp_export_lines.is_empty() {
        stream.line("")?;
        stream.line("MCP exports:")?;
        for line in mcp_export_lines {
            stream.line(line)?;
        }
    }

    let mcp_import_lines = mcp_import_preflight_lines(plan);
    if !mcp_import_lines.is_empty() {
        stream.line("")?;
        stream.line("MCP imports:")?;
        for line in mcp_import_lines {
            stream.line(line)?;
        }
    }

    if !plan.report.diagnostics.is_empty() {
        stream.line("")?;
        stream.line("Diagnostics:")?;
        for diagnostic in &plan.report.diagnostics {
            let severity = match diagnostic.severity {
                PreflightDiagnosticSeverity::Fatal => "fatal",
                PreflightDiagnosticSeverity::Warning => "warning",
                PreflightDiagnosticSeverity::Suppressed => "suppressed",
                PreflightDiagnosticSeverity::Pending => "pending",
                PreflightDiagnosticSeverity::Info => "info",
            };
            if let Some(path) = &diagnostic.path {
                stream.line(format!(
                    "- [{severity}] {} ({path}) — {}",
                    diagnostic.code, diagnostic.message
                ))?;
            } else {
                stream.line(format!(
                    "- [{severity}] {} — {}",
                    diagnostic.code, diagnostic.message
                ))?;
            }
        }
    }

    stream.line("")?;
    stream.line(format!("Status: {:?}", plan.report.status))?;
    match surface {
        HarnessExecutionSurface::Headless => {
            stream.line("Execution: starting one-shot headless run after preflight.")?
        }
        HarnessExecutionSurface::Machine | HarnessExecutionSurface::Tui => {
            stream.line("Execution: preflight only for this surface in the current milestone.")?
        }
    }
    Ok(())
}

fn mcp_export_preflight_lines(plan: &ResolvedHarnessPlan) -> Vec<String> {
    let exports = &plan.report.mcp_exports;
    let mut lines = vec![
        format!("- enabled: {}", exports.enabled),
        format!("- host: {}", exports.host),
        format!(
            "- restart: max_attempts={}, backoff_ms={}",
            exports.restart.max_attempts, exports.restart.backoff_ms
        ),
    ];
    if exports.surfaces.is_empty() {
        lines.push("- surfaces: none".into());
    } else {
        lines.push(format!("- surfaces: {}", exports.surfaces.len()));
        for surface in &exports.surfaces {
            let tools = if surface.tools.is_empty() {
                "none".to_string()
            } else {
                surface.tools.join(", ")
            };
            lines.push(format!("  - `{}`: tools: {}", surface.id, tools));
        }
    }
    lines
}

fn mcp_import_preflight_lines(plan: &ResolvedHarnessPlan) -> Vec<String> {
    let imports = &plan.report.mcp_imports;
    if !imports.enabled {
        return Vec::new();
    }
    let mut lines = vec![format!("- servers: {}", imports.servers.len())];
    for server in &imports.servers {
        let tools = server
            .tools
            .as_ref()
            .map(|tools| {
                if tools.is_empty() {
                    "none".to_string()
                } else {
                    tools.join(", ")
                }
            })
            .unwrap_or_else(|| "all advertised".into());
        let mut detail = format!(
            "  - `{}`: transport: {}, scope: {}, tools: {}",
            server.id, server.transport, server.scope, tools
        );
        if !server.env.is_empty() {
            detail.push_str(&format!(", env: {}", server.env.join(", ")));
        }
        if !server.headers.is_empty() {
            detail.push_str(&format!(", headers: {}", server.headers.join(", ")));
        }
        if let Some(timeout) = server.request_timeout_ms {
            detail.push_str(&format!(", request_timeout_ms: {timeout}"));
        }
        if let Some(timeout) = server.startup_timeout_ms {
            detail.push_str(&format!(", startup_timeout_ms: {timeout}"));
        }
        if let Some(restart) = &server.restart {
            detail.push_str(&format!(
                ", restart: max_attempts={}, backoff_ms={}",
                restart.max_attempts, restart.backoff_ms
            ));
        }
        lines.push(detail);
    }
    lines
}

fn capability_counts(plan: &ResolvedHarnessPlan) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    let mut seen = std::collections::BTreeSet::new();
    for capability in &plan.capabilities {
        let key = capability_state_preflight_label(capability.state);
        if !seen.insert((&capability.kind, &capability.identity, key)) {
            continue;
        }
        *counts.entry(key).or_insert(0) += 1;
    }
    counts
}

type StaticCapabilityDetailKey<'a> = (&'a str, &'a str, &'a str);
type StaticCapabilityDetailValue<'a> = (BTreeSet<&'a str>, BTreeSet<&'a str>);

fn static_capability_detail_lines(plan: &ResolvedHarnessPlan) -> Vec<String> {
    let mut details: BTreeMap<StaticCapabilityDetailKey<'_>, StaticCapabilityDetailValue<'_>> =
        BTreeMap::new();
    for capability in &plan.capabilities {
        let state = capability_state_preflight_label(capability.state);
        let (scopes, sources) = details
            .entry((state, &capability.kind, &capability.identity))
            .or_default();
        scopes.insert(&capability.scope);
        sources.insert(&capability.source);
    }
    details
        .into_iter()
        .map(|((state, kind, identity), (scopes, sources))| {
            format!(
                "  - {state}: {kind} `{identity}` (scopes: {}, sources: {})",
                scopes.into_iter().collect::<Vec<_>>().join(", "),
                sources.into_iter().collect::<Vec<_>>().join(", ")
            )
        })
        .collect()
}

fn capability_state_preflight_label(state: CapabilityState) -> &'static str {
    match state {
        CapabilityState::Available => "available",
        CapabilityState::Pending => "pending runtime activation",
        CapabilityState::Unavailable => "unavailable",
        CapabilityState::Suppressed => "suppressed",
        CapabilityState::NotConfigured => "not_configured",
    }
}

fn human_preflight_verbose_enabled(cli_verbose: bool, plan: &ResolvedHarnessPlan) -> bool {
    cli_verbose || matches!(plan.config.config.trace.level, HarnessTraceLevel::Verbose)
}

#[cfg(test)]
mod tests;
