use crate::harness_plan::{
    CapabilityState, HarnessBootstrapOptions, HarnessExecutionSurface, PreflightDiagnosticSeverity,
    PreflightStatus, ResolvedHarnessPlan, ResolvedPackageInfo, resolve_harness_plan,
};
use crate::harness_runtime::{
    ActionDispatcher, AgentPmActionDispatcher, ApprovalController, BuiltInModelRuntime,
    CompositeKnowledgeRuntime, ConfiguredApprovalController, ConfiguredHookRuntime,
    ConsumerContextSnapshot, CustomKnowledgeRuntime, CustomMemoryRuntime, HookRuntime,
    HostServiceInvoker, KnowledgeEmbeddingSnapshot, KnowledgeRuntime, KnowledgeRuntimeSnapshot,
    LocalKnowledgeRuntime, MemoryRecordTypeRuntimeSnapshot, MemorySpaceRuntimeSnapshot,
    ModelCapabilityAdvertisement, ModelProviderSelection, ModelRequest, ModelRuntime,
    ModelRuntimeFailure, ModelRuntimeRequestSnapshot, ModelTurn, PackageSnapshot,
    ProcessModelRuntime, RoutingEmbeddingProvider, RuntimeCapabilitySnapshot, RuntimeSnapshot,
    ServiceEmbeddingProvider, ServiceLifecycleEmitter, ServiceLifecycleEvents,
    ServiceReadinessSnapshot, SkillResourceSnapshot, SkillRuntimeSnapshot, ToolRuntimeSnapshot,
};
use crate::manifest::{
    AgentManifest, AgentMemoryBinding, MemoryManifest, MemoryRetrievalMode, load_manifest_value,
    parse_knowledge_manifest, parse_loop_manifest, parse_memory_manifest, parse_skill_manifest,
    parse_tool_manifest,
};
use crate::prelude::*;
use crate::{
    harness_config::{HarnessHookId, HarnessTraceLevel},
    harness_engine::{
        HarnessEngine, HarnessEngineOptions, HarnessRunResult, HarnessRuntimeServices,
        HarnessSession, RuntimeTerminalResult,
    },
    harness_observability::{
        HarnessEventEnvelope, HarnessEventSink, HarnessTerminalStatus, JsonlTraceSink,
        MemoryWriteReviewReportSummary, RunOutputPaths, RunReport, allocate_harness_run_id,
        apply_content_policy, apply_content_policy_to_value,
    },
    harness_runtime::SdkHostHookRegistration,
};
use anyhow::{anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::io::{BufRead, Read, Write};
use std::path::PathBuf;
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

#[derive(Args, Debug, Clone)]
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
        let plan = resolve_harness_plan(
            &workspace_root,
            &HarnessBootstrapOptions {
                agent_selector: self.agent.clone(),
                config_path: self.config.clone(),
                state_dir_override: self.state_dir.clone(),
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
            HarnessExecutionSurface::Tui => run_tui_surface(plan),
        }
    }
}

fn run_surface(
    surface: HarnessExecutionSurface,
    plan: ResolvedHarnessPlan,
    args: HarnessArgs,
) -> Result<()> {
    if surface == HarnessExecutionSurface::Headless {
        return run_headless_worker(move || surface.run(&plan, &args));
    }
    surface.run(&plan, &args)
}

fn run_headless_worker(run: impl FnOnce() -> Result<()> + Send + 'static) -> Result<()> {
    std::thread::spawn(run)
        .join()
        .map_err(|_| anyhow!("Harness headless worker panicked"))?
}

fn run_tui_surface(_plan: &ResolvedHarnessPlan) -> Result<()> {
    Ok(())
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
                initialized = true;
                bridge.write_response(
                    id.as_deref(),
                    json!({
                        "session": {
                            "protocol": AGENTPM_HARNESS_MACHINE_PROTOCOL,
                            "version": AGENTPM_HARNESS_MACHINE_VERSION,
                        },
                        "preflight": plan.report,
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
                bridge.write_response(id.as_deref(), json!(plan.report))?;
            }
            "start_run" => {
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
                let terminal = match execute_machine_run(plan, input, &bridge) {
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
                    "memory_operation_unavailable",
                    "external Memory-operation control requests are reserved until the Memory runtime milestone",
                )?;
            }
            "shutdown" => {
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
    Ok(())
}

fn execute_machine_run(
    plan: &ResolvedHarnessPlan,
    input: String,
    bridge: &MachineHostBridgeHandle,
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
    let mut dispatcher = AgentPmActionDispatcher::from_runtime(&runtime)?
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
    let loop_manifest = load_plan_loop(plan)?;
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    session
        .emitter
        .add_sink(Box::new(MachineEventSink::new(bridge.writer())));
    let run_id = allocate_harness_run_id();
    let output_paths = RunOutputPaths::resolve(&plan.state_dir, &run_id, None)?;
    if plan.config.config.trace.enabled {
        session.emitter.add_sink(Box::new(JsonlTraceSink::create(
            &output_paths.events_path,
            plan.config.config.trace.clone(),
        )?));
    }
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
        approval_controller_from_plan(plan, Some(Box::new(bridge.clone())), Some(&service_events))?
    };
    let engine_options = harness_engine_options_from_plan(plan);
    let mut engine = HarnessEngine::new(loop_manifest, engine_options);
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
    let result = engine.execute_run_with_id(&mut session, run_id, input, &mut services)?;
    let HarnessRunResult::Terminal(result) = result else {
        bail!(
            "machine surface cannot retain pending approval without an interactive host controller"
        );
    };
    let mut terminal = *result;
    if plan.config.config.trace.enabled {
        terminal.report.trace_path = Some(output_paths.events_path.display().to_string());
    }
    terminal
        .report
        .write_pretty(&output_paths.report_path, &plan.config.config.trace.content)?;
    session.emitter.flush()?;
    Ok(terminal)
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

    fn writer(&self) -> MachineProtocolWriter {
        self.inner
            .lock()
            .expect("machine bridge poisoned")
            .writer
            .clone()
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
    let mut services = HarnessRuntimeServices {
        model,
        dispatcher,
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
    let engine_options = harness_engine_options_from_plan(plan);
    let mut engine = HarnessEngine::new(loop_manifest, engine_options);
    let result = engine.execute_run_with_id(&mut session, run_id, input, services)?;
    let HarnessRunResult::Terminal(result) = result else {
        bail!("Harness --headless cannot wait for interactive approval");
    };
    let mut terminal = *result;
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
