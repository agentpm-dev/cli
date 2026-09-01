use crate::harness_plan::{
    CapabilityState, HarnessBootstrapOptions, HarnessExecutionSurface, PreflightDiagnosticSeverity,
    PreflightStatus, ResolvedHarnessPlan, ResolvedPackageInfo, resolve_harness_plan,
};
use crate::harness_runtime::{
    ActionDispatcher, AgentPmActionDispatcher, ApprovalController, BuiltInModelRuntime,
    CompositeKnowledgeRuntime, ConfiguredApprovalController, ConfiguredHookRuntime,
    ConsumerContextSnapshot, CustomKnowledgeRuntime, HookRuntime, HostServiceInvoker,
    KnowledgeEmbeddingSnapshot, KnowledgeRuntime, KnowledgeRuntimeSnapshot, LocalKnowledgeRuntime,
    ModelCapabilityAdvertisement, ModelProviderSelection, ModelRequest, ModelRuntime,
    ModelRuntimeFailure, ModelTurn, PackageSnapshot, ProcessModelRuntime, RoutingEmbeddingProvider,
    RuntimeCapabilitySnapshot, RuntimeSnapshot, ServiceEmbeddingProvider, ServiceLifecycleEmitter,
    ServiceLifecycleEvents, ServiceReadinessSnapshot, SkillResourceSnapshot, SkillRuntimeSnapshot,
    ToolRuntimeSnapshot,
};
use crate::manifest::{
    load_manifest_value, parse_knowledge_manifest, parse_loop_manifest, parse_skill_manifest,
    parse_tool_manifest,
};
use crate::prelude::*;
use crate::{
    harness_config::HarnessHookId,
    harness_engine::{
        HarnessEngine, HarnessEngineOptions, HarnessRunResult, HarnessRuntimeServices,
        HarnessSession, RuntimeTerminalResult,
    },
    harness_observability::{
        HarnessEventEnvelope, HarnessEventSink, HarnessTerminalStatus, JsonlTraceSink,
        RunOutputPaths, allocate_harness_run_id, apply_content_policy,
        apply_content_policy_to_value,
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
            print_harness_preflight(&plan, surface, stream)?;
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
    let mut engine = HarnessEngine::new(
        loop_manifest,
        HarnessEngineOptions::new(plan.config.config.runtime.limits.clone()),
    );
    let mut services = HarnessRuntimeServices {
        model: model.as_mut(),
        dispatcher: &mut dispatcher,
        knowledge: knowledge.as_mut(),
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

fn register_host_service(
    plan: &ResolvedHarnessPlan,
    bridge: &MachineHostBridgeHandle,
    payload: &Value,
) -> std::result::Result<HostServiceRegistration, String> {
    let role = payload
        .get("role")
        .and_then(Value::as_str)
        .ok_or_else(|| "host service registration requires payload.role".to_string())?
        .to_string();
    let registry_id = payload
        .get("registry_id")
        .or_else(|| payload.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "host service registration requires payload.registry_id or payload.id".to_string()
        })?
        .to_string();
    let service = HostServiceRegistration { role, registry_id };
    let capabilities = payload
        .get("capabilities")
        .cloned()
        .unwrap_or_else(|| json!({}));
    validate_host_service_readiness(&service, payload)?;
    if !configured_host_services(plan).contains(&service) {
        if service.role == "hook" {
            let registrations = sdk_host_hook_registrations(&service.registry_id, payload)?;
            if registrations.is_empty() {
                return Err(
                    "SDK hook registration requires payload.hooks with at least one Hook ID".into(),
                );
            }
            bridge.register_host_service(&service, capabilities);
            bridge.register_sdk_host_hooks(registrations);
            return Ok(service);
        }
        if service.role == "approval" && service.registry_id == "controller" {
            crate::harness_runtime::approval::approval_capabilities_from_initialization(
                &capabilities,
                "controller",
            )
            .map_err(|err| err.to_string())?;
            bridge.register_host_service(&service, capabilities);
            bridge.register_sdk_approval_controller();
            return Ok(service);
        }
        return Err(format!(
            "host service `{}` with registry ID `{}` is not configured",
            service.role, service.registry_id
        ));
    }
    validate_configured_host_service_registration(plan, &service, payload, &capabilities)?;
    bridge.register_host_service(&service, capabilities);
    Ok(service)
}

fn host_service_registration_response(service: &HostServiceRegistration) -> Value {
    let (active, reason) = host_service_activation_status(service);
    json!({
        "registered": true,
        "service": service,
        "active": active,
        "reason": reason,
    })
}

fn host_service_activation_status(
    service: &HostServiceRegistration,
) -> (bool, Option<&'static str>) {
    match service.role.as_str() {
        "embedding" | "knowledge" => (true, None),
        "memory" => (
            false,
            Some("MemoryRuntime host dispatch is reserved until Milestone 14"),
        ),
        _ => (true, None),
    }
}

fn validate_host_service_readiness(
    service: &HostServiceRegistration,
    payload: &Value,
) -> std::result::Result<(), String> {
    if payload
        .get("ready")
        .and_then(Value::as_bool)
        .is_some_and(|ready| !ready)
    {
        return Err(format!(
            "host service `{}` with registry ID `{}` registered but reported not ready",
            service.role, service.registry_id
        ));
    }
    Ok(())
}

fn validate_configured_host_service_registration(
    plan: &ResolvedHarnessPlan,
    service: &HostServiceRegistration,
    payload: &Value,
    capabilities: &Value,
) -> std::result::Result<(), String> {
    match service.role.as_str() {
        "hook" => validate_configured_host_hook_registration(plan, service, payload, capabilities),
        "embedding" => crate::harness_runtime::knowledge::validate_embedding_provider_capabilities(
            capabilities,
            &service.registry_id,
        )
        .map_err(|err| err.to_string()),
        "knowledge" => {
            let routes = custom_knowledge_routes(plan);
            let mapped_packages = knowledge_snapshots_from_plan(plan)
                .into_iter()
                .filter(|package| routes.get(&package.name) == Some(&service.registry_id))
                .collect::<Vec<_>>();
            crate::harness_runtime::knowledge::validate_knowledge_runtime_capabilities(
                capabilities,
                &service.registry_id,
                &mapped_packages,
            )
            .map_err(|err| err.to_string())
        }
        "approval" if service.registry_id == "controller" => {
            crate::harness_runtime::approval::approval_capabilities_from_initialization(
                capabilities,
                "controller",
            )
            .map_err(|err| err.to_string())?;
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_configured_host_hook_registration(
    plan: &ResolvedHarnessPlan,
    service: &HostServiceRegistration,
    payload: &Value,
    capabilities: &Value,
) -> std::result::Result<(), String> {
    let expected_hooks = configured_host_hook_ids(plan, &service.registry_id);
    if expected_hooks.is_empty() {
        return Ok(());
    }
    let Some(advertised_hooks) = payload
        .get("hooks")
        .or_else(|| capabilities.get("hooks"))
        .cloned()
    else {
        return Err(format!(
            "host hook service `{}` must advertise payload.hooks or capabilities.hooks",
            service.registry_id
        ));
    };
    crate::harness_runtime::hook::validate_hook_service_initialization(
        &json!({
            "registry_id": service.registry_id.clone(),
            "hooks": advertised_hooks,
        }),
        &service.registry_id,
        &expected_hooks,
    )
    .map_err(|err| err.to_string())
}

fn configured_host_hook_ids(
    plan: &ResolvedHarnessPlan,
    implementation: &str,
) -> Vec<HarnessHookId> {
    let mut hook_ids = Vec::new();
    for binding in plan
        .config
        .config
        .hooks
        .bindings
        .iter()
        .filter(|binding| binding.implementation == implementation)
    {
        if !hook_ids.contains(&binding.hook) {
            hook_ids.push(binding.hook.clone());
        }
    }
    hook_ids
}

fn sdk_host_hook_registrations(
    registry_id: &str,
    payload: &Value,
) -> std::result::Result<Vec<SdkHostHookRegistration>, String> {
    let hooks = payload
        .get("hooks")
        .and_then(Value::as_array)
        .ok_or_else(|| "SDK hook registration requires payload.hooks".to_string())?;
    let request_timeout_ms = payload
        .get("request_timeout_ms")
        .and_then(Value::as_u64)
        .unwrap_or(SDK_HOST_REQUEST_TIMEOUT_MS);
    let mut registrations = Vec::new();
    for hook in hooks {
        let Some(hook) = hook.as_str() else {
            return Err("SDK hook registration payload.hooks entries must be strings".into());
        };
        let hook: HarnessHookId =
            serde_json::from_value(Value::String(hook.to_string())).map_err(|err| {
                format!("SDK hook registration contains unsupported Hook ID `{hook}`: {err}")
            })?;
        registrations.push(SdkHostHookRegistration {
            registry_id: registry_id.to_string(),
            hook,
            request_timeout_ms,
        });
    }
    Ok(registrations)
}

fn missing_required_host_services(
    plan: &ResolvedHarnessPlan,
    bridge: &MachineHostBridgeHandle,
) -> Vec<HostServiceRegistration> {
    required_host_services(plan)
        .into_iter()
        .filter(|service| !bridge.has_host_service(service))
        .collect()
}

fn required_host_services(plan: &ResolvedHarnessPlan) -> Vec<HostServiceRegistration> {
    let mut services = Vec::new();
    if let Some(model) = &plan.config.config.model
        && matches!(
            plan.config
                .config
                .providers
                .models
                .get(&model.provider)
                .map(|entry| &entry.implementation),
            Some(crate::harness_config::HarnessImplementation::Host { .. })
        )
    {
        services.push(HostServiceRegistration {
            role: "model".into(),
            registry_id: model.provider.clone(),
        });
    }
    services.extend(host_hook_services(plan));
    if matches!(
        plan.config
            .config
            .approvals
            .controller
            .as_ref()
            .map(|controller| &controller.implementation),
        Some(crate::harness_config::HarnessImplementation::Host { .. })
    ) {
        services.push(HostServiceRegistration {
            role: "approval".into(),
            registry_id: "controller".into(),
        });
    }
    services.extend(required_host_embedding_services(plan));
    services.extend(required_host_knowledge_services(plan));
    dedupe_host_services(services)
}

fn required_host_embedding_services(plan: &ResolvedHarnessPlan) -> Vec<HostServiceRegistration> {
    plan.config
        .config
        .knowledge
        .embedding_matches
        .iter()
        .filter_map(|item| {
            let entry = plan
                .config
                .config
                .providers
                .embeddings
                .get(&item.embedding_provider)?;
            matches!(
                entry.implementation,
                crate::harness_config::HarnessImplementation::Host { .. }
            )
            .then(|| HostServiceRegistration {
                role: "embedding".into(),
                registry_id: item.embedding_provider.clone(),
            })
        })
        .collect()
}

fn required_host_knowledge_services(plan: &ResolvedHarnessPlan) -> Vec<HostServiceRegistration> {
    plan.config
        .config
        .knowledge
        .packages
        .values()
        .filter_map(|mapping| {
            let entry = plan
                .config
                .config
                .knowledge
                .runtimes
                .get(&mapping.runtime)?;
            matches!(
                entry.implementation,
                crate::harness_config::HarnessImplementation::Host { .. }
            )
            .then(|| HostServiceRegistration {
                role: "knowledge".into(),
                registry_id: mapping.runtime.clone(),
            })
        })
        .collect()
}

fn configured_host_services(plan: &ResolvedHarnessPlan) -> Vec<HostServiceRegistration> {
    let mut services = Vec::new();
    services.extend(
        plan.config
            .config
            .providers
            .models
            .iter()
            .filter(|(_, entry)| {
                matches!(
                    entry.implementation,
                    crate::harness_config::HarnessImplementation::Host { .. }
                )
            })
            .map(|(id, _)| HostServiceRegistration {
                role: "model".into(),
                registry_id: id.clone(),
            }),
    );
    services.extend(host_hook_services(plan));
    services.extend(
        plan.config
            .config
            .providers
            .embeddings
            .iter()
            .filter(|(_, entry)| {
                matches!(
                    entry.implementation,
                    crate::harness_config::HarnessImplementation::Host { .. }
                )
            })
            .map(|(id, _)| HostServiceRegistration {
                role: "embedding".into(),
                registry_id: id.clone(),
            }),
    );
    services.extend(
        plan.config
            .config
            .knowledge
            .runtimes
            .iter()
            .filter(|(_, entry)| {
                matches!(
                    entry.implementation,
                    crate::harness_config::HarnessImplementation::Host { .. }
                )
            })
            .map(|(id, _)| HostServiceRegistration {
                role: "knowledge".into(),
                registry_id: id.clone(),
            }),
    );
    services.extend(
        plan.config
            .config
            .memory
            .runtimes
            .iter()
            .filter(|(_, entry)| {
                matches!(
                    entry.implementation,
                    crate::harness_config::HarnessImplementation::Host { .. }
                )
            })
            .map(|(id, _)| HostServiceRegistration {
                role: "memory".into(),
                registry_id: id.clone(),
            }),
    );
    if matches!(
        plan.config
            .config
            .approvals
            .controller
            .as_ref()
            .map(|controller| &controller.implementation),
        Some(crate::harness_config::HarnessImplementation::Host { .. })
    ) {
        services.push(HostServiceRegistration {
            role: "approval".into(),
            registry_id: "controller".into(),
        });
    }
    dedupe_host_services(services)
}

fn host_hook_services(plan: &ResolvedHarnessPlan) -> Vec<HostServiceRegistration> {
    plan.config
        .config
        .hooks
        .bindings
        .iter()
        .filter_map(|binding| {
            let entry = plan
                .config
                .config
                .hooks
                .implementations
                .get(&binding.implementation)?;
            matches!(
                entry.implementation,
                crate::harness_config::HarnessImplementation::Host { .. }
            )
            .then(|| HostServiceRegistration {
                role: "hook".into(),
                registry_id: binding.implementation.clone(),
            })
        })
        .collect()
}

fn dedupe_host_services(services: Vec<HostServiceRegistration>) -> Vec<HostServiceRegistration> {
    let mut seen = BTreeSet::new();
    services
        .into_iter()
        .filter(|service| seen.insert((service.role.clone(), service.registry_id.clone())))
        .collect()
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
    let mut knowledge = {
        let service_events_ref = service_events.as_deref();
        knowledge_runtime_for_headless_plan(
            plan,
            &runtime,
            custom_knowledge.runtime,
            service_events_ref,
        )
    };
    let mut services = HarnessRuntimeServices {
        model,
        dispatcher,
        knowledge: knowledge.as_mut(),
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
    let mut engine = HarnessEngine::new(
        loop_manifest,
        HarnessEngineOptions::new(plan.config.config.runtime.limits.clone()),
    );
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
    for item in &plan.config.config.knowledge.embedding_matches {
        let Some(entry) = plan
            .config
            .config
            .providers
            .embeddings
            .get(&item.embedding_provider)
        else {
            continue;
        };
        routes.insert(
            format!(
                "{}\n{}\n{}\n{}",
                item.r#match.provider,
                item.r#match.model,
                item.r#match.dimensions,
                item.r#match.normalized
            ),
            item.embedding_provider.clone(),
        );
        if providers.contains_key(&item.embedding_provider) {
            continue;
        }
        let provider: Result<Box<dyn crate::harness_runtime::EmbeddingProvider>> =
            match &entry.implementation {
                crate::harness_config::HarnessImplementation::Process { .. } => {
                    ServiceEmbeddingProvider::process(
                        &plan.workspace_root,
                        &item.embedding_provider,
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
                        &item.embedding_provider,
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
            providers.insert(item.embedding_provider.clone(), provider);
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

fn runtime_snapshot_from_plan(plan: &ResolvedHarnessPlan) -> RuntimeSnapshot {
    RuntimeSnapshot {
        session_id: String::new(),
        workspace_root: plan.workspace_root.clone(),
        state_dir: plan.state_dir.clone(),
        agent: plan.selected_agent.as_ref().map(|agent| PackageSnapshot {
            kind: "agent".into(),
            name: agent.name.clone(),
            version: agent.version.clone(),
            root: agent.manifest_path.parent().map(PathBuf::from),
        }),
        loop_package: plan.loop_package.as_ref().map(package_snapshot),
        package_graph: plan.package_graph.values().map(package_snapshot).collect(),
        runtime_config_sources: BTreeMap::from([(
            "state_dir".into(),
            format!("{:?}", plan.config.state_dir_source.kind),
        )]),
        runtime_scopes: plan.runtime_scopes.clone(),
        consumer_context: Some(ConsumerContextSnapshot {
            state: format!("{:?}", plan.consumer_context.state),
            file: plan.consumer_context.file.clone(),
            path: plan.consumer_context.path.clone(),
            content: None,
            byte_size: plan.consumer_context.byte_size,
            approximate_tokens: plan.consumer_context.approximate_tokens,
            sha256: plan.consumer_context.sha256.clone(),
        }),
        services: plan
            .capabilities
            .iter()
            .filter(|capability| capability.kind == "model_provider")
            .map(|capability| ServiceReadinessSnapshot {
                kind: capability.kind.clone(),
                identity: capability.identity.clone(),
                state: format!("{:?}", capability.state),
            })
            .collect(),
        hook_registrations: plan
            .config
            .config
            .hooks
            .bindings
            .iter()
            .map(|binding| format!("{:?}:{}", binding.hook, binding.implementation))
            .collect(),
        profiles: plan.profiles.values().cloned().collect(),
        profile_bindings: plan.profile_bindings.clone(),
        tools: tool_snapshots_from_plan(plan),
        skills: skill_snapshots_from_plan(plan),
        knowledge: knowledge_snapshots_from_plan(plan),
        capability_candidates: plan
            .capabilities
            .iter()
            .map(|capability| RuntimeCapabilitySnapshot {
                kind: capability.kind.clone(),
                identity: capability.identity.clone(),
                scope: capability.scope.clone(),
                source: capability.source.clone(),
                state: capability_state_label(capability.state).into(),
            })
            .collect(),
        model: plan
            .config
            .config
            .model
            .as_ref()
            .map(|model| ModelProviderSelection {
                provider: model.provider.clone(),
                model: model.model.clone(),
                options: model.options.clone(),
            }),
    }
}

fn tool_snapshots_from_plan(plan: &ResolvedHarnessPlan) -> Vec<ToolRuntimeSnapshot> {
    let bound_tools = plan
        .capabilities
        .iter()
        .filter(|capability| capability.kind == "tool")
        .map(|capability| {
            (
                capability.identity.clone(),
                (
                    capability_state_label(capability.state).to_string(),
                    capability.source.clone(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    plan.package_graph
        .values()
        .filter(|package| package.kind == PackageKind::Tool)
        .filter_map(|package| {
            let (state, source) = bound_tools.get(&package.name)?;
            let path = package.root.join("agent.json");
            let manifest = load_manifest_value(&path)
                .and_then(|(value, _)| parse_tool_manifest(&value))
                .ok()?;
            Some(ToolRuntimeSnapshot {
                name: package.name.clone(),
                version: package.version.clone(),
                description: manifest
                    .description
                    .unwrap_or_else(|| "AgentPM Tool capability.".into()),
                root: Some(package.root.clone()),
                input_schema: manifest.inputs,
                state: state.clone(),
                source: source.clone(),
            })
        })
        .collect()
}

fn skill_snapshots_from_plan(plan: &ResolvedHarnessPlan) -> Vec<SkillRuntimeSnapshot> {
    let bound_skills = plan
        .capabilities
        .iter()
        .filter(|capability| capability.kind == "skill")
        .map(|capability| {
            (
                capability.identity.clone(),
                (
                    capability_state_label(capability.state).to_string(),
                    capability.source.clone(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    plan.package_graph
        .values()
        .filter(|package| package.kind == PackageKind::Skill)
        .filter_map(|package| {
            let (state, source) = bound_skills.get(&package.name)?;
            let path = package.root.join("agent.json");
            let manifest = load_manifest_value(&path)
                .and_then(|(value, _)| parse_skill_manifest(&value))
                .ok()?;
            let mut resources = vec![SkillResourceSnapshot {
                id: "entrypoint".into(),
                path: manifest.skill.entrypoint.clone(),
                kind: "entrypoint".into(),
            }];
            resources.extend(manifest.skill.references.iter().map(|reference| {
                SkillResourceSnapshot {
                    id: reference.clone(),
                    path: reference.clone(),
                    kind: "reference".into(),
                }
            }));
            Some(SkillRuntimeSnapshot {
                name: package.name.clone(),
                version: package.version.clone(),
                description: manifest
                    .description
                    .unwrap_or_else(|| "AgentPM Skill resource.".into()),
                root: Some(package.root.clone()),
                resources,
                state: state.clone(),
                source: source.clone(),
            })
        })
        .collect()
}

fn knowledge_snapshots_from_plan(plan: &ResolvedHarnessPlan) -> Vec<KnowledgeRuntimeSnapshot> {
    let bound_knowledge = plan
        .capabilities
        .iter()
        .filter(|capability| capability.kind == "knowledge")
        .map(|capability| {
            (
                capability.identity.clone(),
                (
                    capability_state_label(capability.state).to_string(),
                    capability.source.clone(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    plan.package_graph
        .values()
        .filter(|package| package.kind == PackageKind::Knowledge)
        .filter_map(|package| {
            let (candidate_state, source) = bound_knowledge.get(&package.name)?;
            let path = package.root.join("agent.json");
            let manifest = load_manifest_value(&path)
                .and_then(|(value, _)| parse_knowledge_manifest(&value))
                .ok()?;
            let (runtime, state, readiness_reason) =
                knowledge_runtime_readiness(plan, package, &manifest, candidate_state);
            Some(
                crate::harness_runtime::knowledge::knowledge_snapshot_from_manifest(
                    &package.root,
                    &manifest,
                    source.clone(),
                    runtime,
                    state,
                    readiness_reason,
                ),
            )
        })
        .collect()
}

fn knowledge_runtime_readiness(
    plan: &ResolvedHarnessPlan,
    package: &ResolvedPackageInfo,
    manifest: &crate::manifest::KnowledgeManifest,
    candidate_state: &str,
) -> (String, String, Option<String>) {
    if candidate_state != "available" {
        return (
            "none".into(),
            candidate_state.to_string(),
            Some(format!(
                "Knowledge binding readiness state is {candidate_state}"
            )),
        );
    }
    if let Some(mapping) = plan.config.config.knowledge.packages.get(&package.name) {
        return configured_knowledge_runtime_readiness(plan, &mapping.runtime);
    }
    match manifest.knowledge.mode.as_str() {
        "context" => {
            match crate::commands::knowledge::build_context_mode(&package.root, manifest) {
                Ok(result) => {
                    let mut mismatches = Vec::new();
                    match manifest.knowledge.context.as_ref() {
                        Some(context) => {
                            if context.document_count != Some(result.document_count) {
                                mismatches.push("knowledge.context.document_count");
                            }
                            if context.total_bytes != Some(result.total_bytes) {
                                mismatches.push("knowledge.context.total_bytes");
                            }
                            if context.content_hash.as_deref() != Some(result.content_hash.as_str())
                            {
                                mismatches.push("knowledge.context.content_hash");
                            }
                        }
                        None => mismatches.push("knowledge.context"),
                    }
                    if mismatches.is_empty() {
                        ("local".into(), "available".into(), None)
                    } else {
                        (
                            "local".into(),
                            "unavailable".into(),
                            Some(format!(
                                "context Knowledge metadata is stale or malformed: {}",
                                mismatches.join(", ")
                            )),
                        )
                    }
                }
                Err(err) => ("local".into(), "unavailable".into(), Some(err.to_string())),
            }
        }
        "vector" => match crate::commands::knowledge::local_vector_readiness(&package.root) {
            Ok(readiness) => {
                let space = KnowledgeEmbeddingSnapshot {
                    id: readiness.embedding_id,
                    provider: readiness.provider,
                    model: readiness.model,
                    dimensions: readiness.dimensions,
                    metric: readiness.metric,
                    normalized: readiness.normalized,
                };
                match compatible_embedding_provider_id(plan, &space) {
                    Some(provider_id)
                        if configured_embedding_provider_is_realizable(plan, &provider_id) =>
                    {
                        ("local".into(), "available".into(), None)
                    }
                    Some(provider_id) => (
                        "local".into(),
                        "unavailable".into(),
                        Some(format!(
                            "installed vector artifacts are coherent but EmbeddingProvider `{provider_id}` is unavailable for the current execution surface"
                        )),
                    ),
                    None => (
                        "local".into(),
                        "unavailable".into(),
                        Some(format!(
                            "installed vector artifacts are coherent but no compatible EmbeddingProvider is configured for {}/{}/dimensions={}/normalized={}",
                            space.provider, space.model, space.dimensions, space.normalized
                        )),
                    ),
                }
            }
            Err(err) => (
                "local".into(),
                "unavailable".into(),
                Some(format!("installed vector artifact integrity failed: {err}")),
            ),
        },
        other => (
            "none".into(),
            "unavailable".into(),
            Some(format!("unsupported Knowledge mode `{other}`")),
        ),
    }
}

fn configured_knowledge_runtime_readiness(
    plan: &ResolvedHarnessPlan,
    runtime_id: &str,
) -> (String, String, Option<String>) {
    let Some(_entry) = plan.config.config.knowledge.runtimes.get(runtime_id) else {
        return (
            runtime_id.to_string(),
            "unavailable".into(),
            Some(format!(
                "knowledge.packages references undefined KnowledgeRuntime `{runtime_id}`"
            )),
        );
    };
    match configured_runtime_candidate_state(plan, "knowledge_runtime", runtime_id) {
        Some(
            CapabilityState::Unavailable
            | CapabilityState::Suppressed
            | CapabilityState::NotConfigured,
        ) => (
            runtime_id.to_string(),
            "unavailable".into(),
            Some(format!(
                "configured KnowledgeRuntime `{runtime_id}` is unavailable for the current execution surface"
            )),
        ),
        Some(CapabilityState::Available | CapabilityState::Pending) | None => {
            (runtime_id.to_string(), "available".into(), None)
        }
    }
}

fn configured_embedding_provider_is_realizable(
    plan: &ResolvedHarnessPlan,
    provider_id: &str,
) -> bool {
    matches!(
        configured_runtime_candidate_state(plan, "embedding_provider", provider_id),
        Some(CapabilityState::Available | CapabilityState::Pending) | None
    )
}

fn configured_runtime_candidate_state(
    plan: &ResolvedHarnessPlan,
    kind: &str,
    identity: &str,
) -> Option<CapabilityState> {
    plan.capabilities
        .iter()
        .find(|capability| capability.kind == kind && capability.identity == identity)
        .map(|capability| capability.state)
}

fn compatible_embedding_provider_id(
    plan: &ResolvedHarnessPlan,
    space: &KnowledgeEmbeddingSnapshot,
) -> Option<String> {
    plan.config
        .config
        .knowledge
        .embedding_matches
        .iter()
        .find(|item| crate::harness_runtime::knowledge::embedding_key_matches(space, &item.r#match))
        .map(|item| item.embedding_provider.clone())
}

fn capability_state_label(state: CapabilityState) -> &'static str {
    match state {
        CapabilityState::Available => "available",
        CapabilityState::Pending => "pending",
        CapabilityState::Unavailable => "unavailable",
        CapabilityState::Suppressed => "suppressed",
        CapabilityState::NotConfigured => "not_configured",
    }
}

fn package_snapshot(package: &ResolvedPackageInfo) -> PackageSnapshot {
    PackageSnapshot {
        kind: format!("{:?}", package.kind),
        name: package.name.clone(),
        version: package.version.clone(),
        root: Some(package.root.clone()),
    }
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
        let key = match capability.state {
            CapabilityState::Available => "available",
            CapabilityState::Pending => "pending",
            CapabilityState::Unavailable => "unavailable",
            CapabilityState::Suppressed => "suppressed",
            CapabilityState::NotConfigured => "not_configured",
        };
        if !seen.insert((&capability.kind, &capability.identity, key)) {
            continue;
        }
        *counts.entry(key).or_insert(0) += 1;
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness_config::{
        HarnessApprovalController, HarnessConfig, HarnessConfigSource, HarnessHookBinding,
        HarnessHookFailurePolicy, HarnessHookId, HarnessImplementation, HarnessImplementationEntry,
        HarnessRuntimeMapping, HarnessTraceConfig, HarnessTraceContent, HarnessTraceLevel,
        ResolvedHarnessConfig,
    };
    use crate::harness_observability::{
        HarnessTerminalStatus, ReportPackageIdentity, RunReport, RunUsage,
    };
    use crate::harness_runtime::SemanticAction;
    use crate::harness_runtime::action::ScriptedActionDispatcher;
    use crate::harness_runtime::action::SemanticActionProposal;
    use crate::harness_runtime::model::{
        ModelCapabilityAdvertisement, ModelRuntimeFailure, ModelTurn, ScriptedModelRuntime,
    };
    use crate::semver::types::PackageKind;
    use serde_json::json;
    use std::fs;
    use std::path::Path;

    #[test]
    fn parse_scope_requires_key_value_pair() {
        assert_eq!(
            parse_scope("user=user-1").unwrap(),
            ("user".to_string(), "user-1".to_string())
        );
        assert!(parse_scope("user").is_err());
        assert!(parse_scope("=user-1").is_err());
        assert!(parse_scope("user=").is_err());
    }

    #[test]
    fn default_surface_is_tui_with_explicit_headless_and_machine_modes() {
        let default_args = HarnessArgs {
            agent: None,
            config: None,
            state_dir: None,
            scopes: Vec::new(),
            machine: false,
            headless: false,
            json: false,
            input: None,
            input_file: None,
            report: None,
        };
        assert_eq!(default_args.surface(), HarnessExecutionSurface::Tui);

        let headless_args = HarnessArgs {
            headless: true,
            ..default_args.clone()
        };
        assert_eq!(headless_args.surface(), HarnessExecutionSurface::Headless);

        let machine_args = HarnessArgs {
            machine: true,
            ..default_args
        };
        assert_eq!(machine_args.surface(), HarnessExecutionSurface::Machine);
    }

    #[test]
    fn headless_and_machine_preflight_avoid_stdout_reserved_for_payloads() {
        assert_eq!(
            PreflightOutputStream::for_surface(HarnessExecutionSurface::Headless),
            PreflightOutputStream::Stderr
        );
        assert_eq!(
            PreflightOutputStream::for_surface(HarnessExecutionSurface::Tui),
            PreflightOutputStream::Stdout
        );
        assert_eq!(
            PreflightOutputStream::for_surface(HarnessExecutionSurface::Machine),
            PreflightOutputStream::Stderr
        );
        let args = HarnessArgs {
            agent: None,
            config: None,
            state_dir: None,
            scopes: Vec::new(),
            machine: false,
            headless: true,
            json: true,
            input: None,
            input_file: None,
            report: None,
        };
        let err = validate_surface_flags(HarnessExecutionSurface::Headless, &args).unwrap_err();
        assert!(err.to_string().contains("--json cannot be combined"));
    }

    #[test]
    fn machine_protocol_rejects_wrong_version_and_non_request_input() {
        let mut request = MachineEnvelope {
            protocol: AGENTPM_HARNESS_MACHINE_PROTOCOL.into(),
            version: AGENTPM_HARNESS_MACHINE_VERSION,
            kind: MachineFrameKind::Request,
            id: Some("req-1".into()),
            method: Some("initialize".into()),
            payload: json!({}),
            error: None,
        };
        assert!(validate_machine_request(&request).is_ok());
        request.version = 2;
        assert!(
            validate_machine_request(&request)
                .unwrap_err()
                .contains("unsupported protocol version")
        );
        request.version = AGENTPM_HARNESS_MACHINE_VERSION;
        request.kind = MachineFrameKind::Event;
        assert!(
            validate_machine_request(&request)
                .unwrap_err()
                .contains("kind `request`")
        );
    }

    #[test]
    fn machine_json_flag_is_rejected_because_stdout_is_protocol_only() {
        let args = HarnessArgs {
            agent: None,
            config: None,
            state_dir: None,
            scopes: Vec::new(),
            machine: true,
            headless: false,
            json: true,
            input: None,
            input_file: None,
            report: None,
        };
        let err = validate_surface_flags(HarnessExecutionSurface::Machine, &args).unwrap_err();
        assert!(err.to_string().contains("protocol frames"));
    }

    #[test]
    fn host_service_requirements_include_selected_model_hooks_and_approval() {
        let root = temp_dir("host-service-requirements");
        let mut plan = minimal_plan(&root);
        plan.config.config.model = Some(crate::harness_config::HarnessModelConfig {
            provider: "host-model".into(),
            model: "model-1".into(),
            options: json!({}),
        });
        plan.config.config.providers.models.insert(
            "host-model".into(),
            HarnessImplementationEntry {
                implementation: HarnessImplementation::Host {
                    request_timeout_ms: 1_000,
                },
            },
        );
        plan.config.config.hooks.implementations.insert(
            "host-hooks".into(),
            HarnessImplementationEntry {
                implementation: HarnessImplementation::Host {
                    request_timeout_ms: 1_000,
                },
            },
        );
        plan.config.config.hooks.bindings.push(HarnessHookBinding {
            hook: HarnessHookId::BeforeToolCall,
            implementation: "host-hooks".into(),
            failure_policy: HarnessHookFailurePolicy::Closed,
        });
        plan.config.config.approvals.controller = Some(HarnessApprovalController {
            implementation: HarnessImplementation::Host {
                request_timeout_ms: 1_000,
            },
        });

        let required = required_host_services(&plan);
        assert!(required.contains(&host_service("model", "host-model")));
        assert!(required.contains(&host_service("hook", "host-hooks")));
        assert!(required.contains(&host_service("approval", "controller")));
    }

    #[test]
    fn mapped_knowledge_runtime_readiness_requires_realizable_runtime() {
        let root = temp_dir("mapped-knowledge-runtime-readiness");
        let knowledge_root = root.join(".agentpm/knowledge/@zack/guide/0.1.0");
        write_json(
            &knowledge_root.join("agent.json"),
            json!({
                "kind": "knowledge",
                "name": "@zack/guide",
                "version": "0.1.0",
                "description": "Guide.",
                "knowledge": {
                    "mode": "context",
                    "content_type": "text/markdown",
                    "documents": [
                        { "path": "knowledge/docs/guide.md", "content_type": "text/markdown" }
                    ]
                }
            }),
        );
        let mut plan = minimal_plan(&root);
        plan.package_graph.insert(
            "knowledge:@zack/guide@0.1.0".into(),
            ResolvedPackageInfo {
                key: "knowledge:@zack/guide@0.1.0".into(),
                kind: PackageKind::Knowledge,
                name: "@zack/guide".into(),
                version: "0.1.0".into(),
                root: knowledge_root,
            },
        );
        plan.capabilities
            .push(crate::harness_plan::StaticCapabilityCandidate {
                kind: "knowledge".into(),
                identity: "@zack/guide".into(),
                scope: "global".into(),
                source: "agent_binding".into(),
                state: CapabilityState::Available,
            });
        plan.capabilities
            .push(crate::harness_plan::StaticCapabilityCandidate {
                kind: "knowledge_runtime".into(),
                identity: "remote-knowledge".into(),
                scope: "session".into(),
                source: "harness_config".into(),
                state: CapabilityState::Unavailable,
            });
        plan.config.config.knowledge.runtimes.insert(
            "remote-knowledge".into(),
            HarnessImplementationEntry {
                implementation: HarnessImplementation::Host {
                    request_timeout_ms: 1_000,
                },
            },
        );
        plan.config.config.knowledge.packages.insert(
            "@zack/guide".into(),
            HarnessRuntimeMapping {
                runtime: "remote-knowledge".into(),
            },
        );

        let runtime = runtime_snapshot_from_plan(&plan);
        assert_eq!(runtime.knowledge.len(), 1);
        assert_eq!(runtime.knowledge[0].runtime, "remote-knowledge");
        assert_eq!(runtime.knowledge[0].state, "unavailable");
        assert!(
            runtime.knowledge[0]
                .readiness_reason
                .as_deref()
                .unwrap_or_default()
                .contains("configured KnowledgeRuntime `remote-knowledge` is unavailable")
        );
    }

    #[test]
    fn custom_knowledge_activation_failure_suppresses_mapped_package() {
        let root = temp_dir("custom-knowledge-activation-failure");
        let knowledge_root = root.join(".agentpm/knowledge/@zack/guide/0.1.0");
        write_json(
            &knowledge_root.join("agent.json"),
            json!({
                "kind": "knowledge",
                "name": "@zack/guide",
                "version": "0.1.0",
                "description": "Guide.",
                "knowledge": {
                    "mode": "context",
                    "content_type": "text/markdown",
                    "documents": [
                        { "path": "knowledge/docs/guide.md", "content_type": "text/markdown" }
                    ]
                }
            }),
        );
        let mut plan = minimal_plan(&root);
        plan.package_graph.insert(
            "knowledge:@zack/guide@0.1.0".into(),
            ResolvedPackageInfo {
                key: "knowledge:@zack/guide@0.1.0".into(),
                kind: PackageKind::Knowledge,
                name: "@zack/guide".into(),
                version: "0.1.0".into(),
                root: knowledge_root,
            },
        );
        plan.capabilities
            .push(crate::harness_plan::StaticCapabilityCandidate {
                kind: "knowledge".into(),
                identity: "@zack/guide".into(),
                scope: "global".into(),
                source: "agent_binding".into(),
                state: CapabilityState::Available,
            });
        plan.capabilities
            .push(crate::harness_plan::StaticCapabilityCandidate {
                kind: "knowledge_runtime".into(),
                identity: "remote-knowledge".into(),
                scope: "session".into(),
                source: "harness_config".into(),
                state: CapabilityState::Available,
            });
        plan.config.config.knowledge.runtimes.insert(
            "remote-knowledge".into(),
            HarnessImplementationEntry {
                implementation: HarnessImplementation::Process {
                    command: "__agentpm_missing_knowledge_runtime__".into(),
                    args: Vec::new(),
                    cwd: None,
                    env: Vec::new(),
                    startup_timeout_ms: 100,
                    request_timeout_ms: 100,
                    restart: Default::default(),
                },
            },
        );
        plan.config.config.knowledge.packages.insert(
            "@zack/guide".into(),
            HarnessRuntimeMapping {
                runtime: "remote-knowledge".into(),
            },
        );

        let mut runtime = runtime_snapshot_from_plan(&plan);
        assert_eq!(runtime.knowledge.len(), 1);
        assert_eq!(runtime.knowledge[0].state, "available");

        let activation = activate_custom_knowledge_runtime_for_plan(&plan, &runtime, None, None);
        assert!(activation.runtime.is_none());
        apply_custom_knowledge_activation_to_runtime(&mut runtime, &activation);

        assert_eq!(runtime.knowledge[0].runtime, "remote-knowledge");
        assert_eq!(runtime.knowledge[0].state, "unavailable");
        assert!(
            runtime.knowledge[0]
                .readiness_reason
                .as_deref()
                .unwrap_or_default()
                .contains("configured KnowledgeRuntime `remote-knowledge` could not start")
        );
    }

    #[test]
    fn custom_knowledge_activation_isolates_unhealthy_runtime() {
        let root = temp_dir("custom-knowledge-activation-isolates-runtime");
        let healthy_root = root.join(".agentpm/knowledge/@zack/healthy/0.1.0");
        let unhealthy_root = root.join(".agentpm/knowledge/@zack/unhealthy/0.1.0");
        for (package_root, name, description) in [
            (&healthy_root, "@zack/healthy", "Healthy guide."),
            (&unhealthy_root, "@zack/unhealthy", "Unhealthy guide."),
        ] {
            write_json(
                &package_root.join("agent.json"),
                json!({
                    "kind": "knowledge",
                    "name": name,
                    "version": "0.1.0",
                    "description": description,
                    "knowledge": {
                        "mode": "context",
                        "content_type": "text/markdown",
                        "documents": [
                            { "path": "knowledge/docs/guide.md", "content_type": "text/markdown" }
                        ]
                    }
                }),
            );
        }
        let mut plan = minimal_plan(&root);
        for (name, package_root) in [
            ("@zack/healthy", healthy_root),
            ("@zack/unhealthy", unhealthy_root),
        ] {
            plan.package_graph.insert(
                format!("knowledge:{name}@0.1.0"),
                ResolvedPackageInfo {
                    key: format!("knowledge:{name}@0.1.0"),
                    kind: PackageKind::Knowledge,
                    name: name.into(),
                    version: "0.1.0".into(),
                    root: package_root,
                },
            );
            plan.capabilities
                .push(crate::harness_plan::StaticCapabilityCandidate {
                    kind: "knowledge".into(),
                    identity: name.into(),
                    scope: "global".into(),
                    source: "agent_binding".into(),
                    state: CapabilityState::Available,
                });
        }
        for runtime_id in ["healthy-knowledge", "unhealthy-knowledge"] {
            plan.capabilities
                .push(crate::harness_plan::StaticCapabilityCandidate {
                    kind: "knowledge_runtime".into(),
                    identity: runtime_id.into(),
                    scope: "session".into(),
                    source: "harness_config".into(),
                    state: CapabilityState::Available,
                });
        }
        plan.config.config.knowledge.runtimes.insert(
            "healthy-knowledge".into(),
            HarnessImplementationEntry {
                implementation: HarnessImplementation::Host {
                    request_timeout_ms: 1_000,
                },
            },
        );
        plan.config.config.knowledge.runtimes.insert(
            "unhealthy-knowledge".into(),
            HarnessImplementationEntry {
                implementation: HarnessImplementation::Process {
                    command: "__agentpm_missing_knowledge_runtime__".into(),
                    args: Vec::new(),
                    cwd: None,
                    env: Vec::new(),
                    startup_timeout_ms: 100,
                    request_timeout_ms: 100,
                    restart: Default::default(),
                },
            },
        );
        plan.config.config.knowledge.packages.insert(
            "@zack/healthy".into(),
            HarnessRuntimeMapping {
                runtime: "healthy-knowledge".into(),
            },
        );
        plan.config.config.knowledge.packages.insert(
            "@zack/unhealthy".into(),
            HarnessRuntimeMapping {
                runtime: "unhealthy-knowledge".into(),
            },
        );
        let (bridge, _, _) = buffered_machine_bridge();
        bridge.register_host_service(
            &host_service("knowledge", "healthy-knowledge"),
            json!({
                "ready": true,
                "registry_id": "healthy-knowledge",
                "modes": ["context_document"],
                "features": [],
                "packages": [
                    {
                        "package": "@zack/healthy",
                        "version": "0.1.0",
                        "ready": true
                    }
                ]
            }),
        );

        let mut runtime = runtime_snapshot_from_plan(&plan);
        let activation =
            activate_custom_knowledge_runtime_for_plan(&plan, &runtime, Some(bridge), None);
        assert!(activation.runtime.is_some());
        apply_custom_knowledge_activation_to_runtime(&mut runtime, &activation);

        let healthy = runtime
            .knowledge
            .iter()
            .find(|package| package.name == "@zack/healthy")
            .unwrap();
        assert_eq!(healthy.runtime, "healthy-knowledge");
        assert_eq!(healthy.state, "available");
        assert!(healthy.readiness_reason.is_none());

        let unhealthy = runtime
            .knowledge
            .iter()
            .find(|package| package.name == "@zack/unhealthy")
            .unwrap();
        assert_eq!(unhealthy.runtime, "unhealthy-knowledge");
        assert_eq!(unhealthy.state, "unavailable");
        assert!(
            unhealthy
                .readiness_reason
                .as_deref()
                .unwrap_or_default()
                .contains("configured KnowledgeRuntime `unhealthy-knowledge` could not start")
        );
    }

    #[test]
    fn machine_registration_accepts_unconfigured_sdk_hooks_and_approval() {
        let root = temp_dir("sdk-host-service-registration");
        let plan = minimal_plan(&root);
        let (bridge, _, _) = buffered_machine_bridge();

        let hook_service = register_host_service(
            &plan,
            &bridge,
            &json!({
                "role": "hook",
                "registry_id": "sdk-hooks",
                "hooks": ["before_tool_call", "before_model_request"]
            }),
        )
        .unwrap();
        let approval_service = register_host_service(
            &plan,
            &bridge,
            &json!({
                "role": "approval",
                "registry_id": "controller"
            }),
        )
        .unwrap();

        assert_eq!(hook_service, host_service("hook", "sdk-hooks"));
        assert_eq!(approval_service, host_service("approval", "controller"));
        assert!(bridge.has_host_service(&hook_service));
        assert!(bridge.has_host_service(&approval_service));
        assert!(bridge.has_sdk_approval_controller());
        let hooks = bridge.sdk_host_hooks();
        assert_eq!(hooks.len(), 2);
        assert!(
            hooks.iter().any(|hook| hook.registry_id == "sdk-hooks"
                && hook.hook == HarnessHookId::BeforeToolCall)
        );
        assert!(hooks.iter().any(|hook| hook.registry_id == "sdk-hooks"
            && hook.hook == HarnessHookId::BeforeModelRequest));
    }

    #[test]
    fn machine_registration_rejects_unconfigured_host_provider() {
        let root = temp_dir("unconfigured-host-provider-registration");
        let plan = minimal_plan(&root);
        let (bridge, _, _) = buffered_machine_bridge();

        let err = register_host_service(
            &plan,
            &bridge,
            &json!({
                "role": "model",
                "registry_id": "sdk-model"
            }),
        )
        .unwrap_err();

        assert!(err.contains("is not configured"));
    }

    #[test]
    fn host_registration_response_marks_milestone_twelve_roles_active() {
        let embedding = host_service_registration_response(&host_service("embedding", "embedder"));
        assert_eq!(embedding["registered"], json!(true));
        assert_eq!(embedding["active"], json!(true));
        assert!(embedding["reason"].is_null());

        let knowledge = host_service_registration_response(&host_service("knowledge", "kb"));
        assert_eq!(knowledge["active"], json!(true));
        assert!(knowledge["reason"].is_null());

        let memory = host_service_registration_response(&host_service("memory", "store"));
        assert_eq!(memory["active"], json!(false));
        assert!(memory["reason"].as_str().unwrap().contains("Milestone 14"));

        let model = host_service_registration_response(&host_service("model", "host-model"));
        assert_eq!(model["active"], json!(true));
        assert!(model["reason"].is_null());
    }

    #[test]
    fn host_model_runtime_uses_machine_host_service_contract() {
        let selection = ModelProviderSelection {
            provider: "host-model".into(),
            model: "model-1".into(),
            options: json!({}),
        };
        let expected_turn = ModelTurn {
            assistant_content: Some("from host".into()),
            actions: Vec::new(),
            usage: RunUsage::default(),
            finish_reason: Some("stop".into()),
            provider_metadata: BTreeMap::new(),
        };
        let mut runtime = HostModelRuntime {
            selection: selection.clone(),
            invoker: Box::new(FakeHostInvoker {
                response: serde_json::to_value(&expected_turn).unwrap(),
                capabilities: None,
            }),
            capabilities: host_model_capabilities_from_registration(
                &host_model_capabilities(),
                "host-model",
                "model-1",
            )
            .unwrap(),
            request_timeout_ms: 1_000,
        };

        let turn = runtime.generate(empty_model_request(selection)).unwrap();
        assert_eq!(turn, expected_turn);
    }

    #[test]
    fn host_model_runtime_uses_registered_capability_advertisement() {
        let root = temp_dir("host-model-capability-advertisement");
        let mut plan = minimal_plan(&root);
        plan.config.config.model = Some(crate::harness_config::HarnessModelConfig {
            provider: "host-model".into(),
            model: "model-1".into(),
            options: json!({}),
        });
        plan.config.config.providers.models.insert(
            "host-model".into(),
            HarnessImplementationEntry {
                implementation: HarnessImplementation::Host {
                    request_timeout_ms: 1_000,
                },
            },
        );
        let (bridge, _, _) = buffered_machine_bridge();
        register_host_service(
            &plan,
            &bridge,
            &json!({
                "role": "model",
                "registry_id": "host-model",
                "capabilities": {
                    "provider": "host-model",
                    "model": "model-1",
                    "semantic_actions": false,
                    "structured_output": true,
                    "multimodal_input": false,
                    "usage_reporting": true
                }
            }),
        )
        .unwrap();

        let runtime = model_runtime_from_plan(
            &plan,
            ModelProviderSelection {
                provider: "host-model".into(),
                model: "model-1".into(),
                options: json!({}),
            },
            Some(Box::new(bridge)),
            None,
        )
        .unwrap();
        let err = validate_model_capabilities(runtime.as_ref()).unwrap_err();
        assert!(err.to_string().contains("semantic action support"));
    }

    #[test]
    fn host_model_capabilities_reject_mismatched_model_identity() {
        let err = host_model_capabilities_from_registration(
            &json!({
                "provider": "host-model",
                "model": "other-model",
                "semantic_actions": true,
                "structured_output": true,
                "multimodal_input": false,
                "usage_reporting": true
            }),
            "host-model",
            "model-1",
        )
        .unwrap_err();
        assert!(err.to_string().contains("expected `model-1`"));
    }

    #[test]
    fn host_service_registration_rejects_not_ready() {
        let root = temp_dir("host-service-not-ready");
        let plan = minimal_plan(&root);
        let (bridge, _sender, _output) = buffered_machine_bridge();

        let err = register_host_service(
            &plan,
            &bridge,
            &json!({
                "role": "approval",
                "registry_id": "controller",
                "ready": false
            }),
        )
        .unwrap_err();

        assert!(err.contains("reported not ready"));
    }

    #[test]
    fn configured_host_hook_registration_validates_advertised_hooks() {
        let root = temp_dir("configured-host-hook-registration");
        let mut plan = minimal_plan(&root);
        plan.config.config.hooks.implementations.insert(
            "host-hooks".into(),
            HarnessImplementationEntry {
                implementation: HarnessImplementation::Host {
                    request_timeout_ms: 1_000,
                },
            },
        );
        plan.config.config.hooks.bindings.push(HarnessHookBinding {
            hook: HarnessHookId::BeforeToolCall,
            implementation: "host-hooks".into(),
            failure_policy: HarnessHookFailurePolicy::Closed,
        });
        let (bridge, _sender, _output) = buffered_machine_bridge();

        let err = register_host_service(
            &plan,
            &bridge,
            &json!({
                "role": "hook",
                "registry_id": "host-hooks",
                "hooks": ["before_model_request"]
            }),
        )
        .unwrap_err();
        assert!(err.contains("does not advertise configured hook `before_tool_call`"));

        register_host_service(
            &plan,
            &bridge,
            &json!({
                "role": "hook",
                "registry_id": "host-hooks",
                "capabilities": {
                    "hooks": ["before_tool_call"]
                }
            }),
        )
        .unwrap();
    }

    #[test]
    fn configured_host_approval_rejects_missing_request_capability() {
        let controller = HarnessApprovalController {
            implementation: HarnessImplementation::Host {
                request_timeout_ms: 1_000,
            },
        };

        let err = match ConfiguredApprovalController::host(
            &controller,
            None,
            Box::new(FakeHostInvoker {
                response: json!({ "decision": "approve" }),
                capabilities: Some(json!({
                    "approval": false
                })),
            }),
        ) {
            Ok(_) => panic!("host approval controller should reject missing request capability"),
            Err(err) => err,
        };

        assert!(
            err.to_string()
                .contains("does not advertise request_approval support")
        );
    }

    #[test]
    fn host_service_request_frames_bypass_trace_content_redaction() {
        let writer = MachineProtocolWriter::stdout(HarnessTraceContent::Redacted);
        let host_request = MachineEnvelope {
            protocol: AGENTPM_HARNESS_MACHINE_PROTOCOL.into(),
            version: AGENTPM_HARNESS_MACHINE_VERSION,
            kind: MachineFrameKind::Request,
            id: Some("host-hook-1".into()),
            method: Some("host_service".into()),
            payload: json!({
                "role": "hook",
                "registry_id": "host-hooks",
                "method": "before_tool_call",
                "payload": {
                    "hook": "before_tool_call",
                    "input": {
                        "phase_id": "classify",
                        "tool": "@zack/search",
                        "arguments": {
                            "query": "visible to host implementation"
                        }
                    }
                }
            }),
            error: None,
        };

        let redacted = writer.frame_value(host_request.clone(), true).unwrap();
        assert_eq!(redacted["payload"]["payload"]["input"], json!("[redacted]"));

        let unredacted = writer.frame_value(host_request, false).unwrap();
        assert_eq!(
            unredacted["payload"]["payload"]["input"]["arguments"]["query"],
            json!("visible to host implementation")
        );
    }

    #[test]
    fn machine_bridge_rejects_start_run_while_active_without_blocking_host_service_response() {
        let (bridge, sender, output) = buffered_machine_bridge();
        bridge.register_host_service(
            &host_service("model", "host-model"),
            host_model_capabilities(),
        );
        bridge.set_active_run(true);
        let mut bridge_for_thread = bridge.clone();
        let waiter = std::thread::spawn(move || {
            bridge_for_thread.invoke_host_service(
                "model",
                "host-model",
                "generate",
                json!({ "input": "visible" }),
                1_000,
            )
        });

        sender
            .send(Ok(machine_request(
                "start-while-active",
                "start_run",
                json!({ "input": "second run" }),
            )))
            .unwrap();
        sender
            .send(Ok(machine_response(
                "host-model-host-model-1",
                json!({ "ok": true }),
            )))
            .unwrap();

        assert_eq!(waiter.join().unwrap().unwrap(), json!({ "ok": true }));
        let frames = machine_frames_from_buffer(&output);
        assert!(frames.iter().any(|frame| {
            frame["id"] == "start-while-active"
                && frame["kind"] == "error"
                && frame["error"]["code"] == "session_busy"
        }));
    }

    #[test]
    fn machine_bridge_cancel_run_interrupts_active_host_service_wait() {
        let (bridge, sender, output) = buffered_machine_bridge();
        bridge.register_host_service(
            &host_service("model", "host-model"),
            host_model_capabilities(),
        );
        bridge.set_active_run(true);
        let mut bridge_for_thread = bridge.clone();
        let waiter = std::thread::spawn(move || {
            bridge_for_thread.invoke_host_service(
                "model",
                "host-model",
                "generate",
                json!({ "input": "visible" }),
                1_000,
            )
        });

        sender
            .send(Ok(machine_request("cancel-1", "cancel_run", json!({}))))
            .unwrap();

        let err = waiter.join().unwrap().unwrap_err();
        assert!(err.to_string().contains("run cancellation requested"));
        assert!(bridge.cancellation_token().load(Ordering::SeqCst));
        let frames = machine_frames_from_buffer(&output);
        assert!(frames.iter().any(|frame| {
            frame["id"] == "cancel-1"
                && frame["kind"] == "response"
                && frame["payload"]["accepted"] == true
        }));
    }

    #[test]
    fn machine_bridge_emits_host_service_failure_events() {
        let (bridge, sender, _output) = buffered_machine_bridge();
        bridge.register_host_service(
            &host_service("model", "host-model"),
            host_model_capabilities(),
        );
        let mut service_events = ServiceLifecycleEvents::new();
        bridge.set_host_service_lifecycle_emitter(service_events.emitter());
        let mut bridge_for_thread = bridge.clone();
        let waiter = std::thread::spawn(move || {
            bridge_for_thread.invoke_host_service(
                "model",
                "host-model",
                "generate",
                json!({ "input": "visible" }),
                1_000,
            )
        });

        sender
            .send(Ok(machine_error(
                "host-model-host-model-1",
                "host_failure",
                "host model failed",
            )))
            .unwrap();

        let err = waiter.join().unwrap().unwrap_err();
        assert!(err.to_string().contains("host model failed"));
        let events = service_events.drain();
        assert!(events.iter().any(|event| {
            event.event_type == crate::harness_observability::HarnessEventType::ServiceUnhealthy
                && event.service == "model"
                && event.registry_id == "host-model"
        }));
        assert!(events.iter().any(|event| {
            event.event_type == crate::harness_observability::HarnessEventType::ServiceFailed
                && event.service == "model"
                && event.registry_id == "host-model"
        }));
    }

    #[test]
    fn machine_bridge_accepts_shutdown_control_request() {
        let (bridge, sender, output) = buffered_machine_bridge();
        sender
            .send(Ok(machine_request("shutdown-1", "shutdown", json!({}))))
            .unwrap();

        let request = bridge.recv_control_request().unwrap().unwrap();
        assert_eq!(request.method.as_deref(), Some("shutdown"));
        bridge
            .write_response(request.id.as_deref(), json!({ "shutdown": true }))
            .unwrap();

        let frames = machine_frames_from_buffer(&output);
        assert!(frames.iter().any(|frame| {
            frame["id"] == "shutdown-1"
                && frame["kind"] == "response"
                && frame["payload"]["shutdown"] == true
        }));
    }

    #[test]
    fn host_approval_controller_decodes_machine_host_decision() {
        let controller = HarnessApprovalController {
            implementation: HarnessImplementation::Host {
                request_timeout_ms: 1_000,
            },
        };
        let mut runtime = ConfiguredApprovalController::host(
            &controller,
            None,
            Box::new(FakeHostInvoker {
                response: json!({ "decision": "deny" }),
                capabilities: None,
            }),
        )
        .unwrap()
        .unwrap();
        let decision = runtime.request_approval(&crate::manifest::LoopCheckpoint {
            id: "approve-review".into(),
            r#type: "approval".into(),
            before_phase: "review".into(),
            on_reject: "$handoff".into(),
        });
        assert_eq!(decision, crate::harness_runtime::ApprovalDecision::Deny);
    }

    #[test]
    fn model_selection_requires_configured_model_for_headless_execution() {
        let root = temp_dir("missing-model-selection");
        let plan = minimal_plan(&root);
        let err = model_selection(&plan).unwrap_err();
        assert!(err.to_string().contains("requires model.provider"));
    }

    #[test]
    fn failed_headless_terminal_status_includes_terminal_error_detail() {
        let terminal = RuntimeTerminalResult {
            status: HarnessTerminalStatus::Failed,
            output: Some(json!({ "error": "OPENAI_API_KEY is required for provider `openai`" })),
            report: minimal_run_report("run-1"),
        };
        let message =
            terminal_status_error_message(&terminal, HarnessTerminalStatus::Failed).unwrap();
        assert!(message.contains("terminal status Failed"));
        assert!(message.contains("OPENAI_API_KEY"));
    }

    #[test]
    fn model_capability_validation_rejects_missing_semantic_or_structured_support() {
        let runtime = UnsupportedModelRuntime {
            semantic_actions: false,
            structured_output: true,
        };
        let err = validate_model_capabilities(&runtime).unwrap_err();
        assert!(err.to_string().contains("semantic action support"));

        let runtime = UnsupportedModelRuntime {
            semantic_actions: true,
            structured_output: false,
        };
        let err = validate_model_capabilities(&runtime).unwrap_err();
        assert!(err.to_string().contains("structured output support"));
    }

    #[tokio::test]
    async fn headless_worker_constructs_blocking_provider_outside_tokio_runtime() {
        run_headless_worker(|| {
            let _runtime = BuiltInModelRuntime::from_selection(ModelProviderSelection {
                provider: "openai".into(),
                model: "gpt-4o-mini".into(),
                options: json!({}),
            })
            .map_err(|err| anyhow!(err.message))?;
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn headless_execution_runs_one_engine_run_and_writes_report() {
        let root = temp_dir("headless-exec");
        let loop_root = root.join(".agentpm/loops/zack/review-loop/0.1.0");
        write_json(
            &loop_root.join("agent.json"),
            json!({
                "kind": "loop",
                "name": "@zack/review-loop",
                "version": "0.1.0",
                "loop": {
                    "entry_phase": "respond",
                    "phases": [
                        { "id": "respond", "objective": "Respond to the request." }
                    ],
                    "transitions": [
                        { "from": "respond", "on": "complete", "to": "$end" }
                    ]
                }
            }),
        );
        let mut plan = minimal_plan(&root);
        plan.config.config.model = Some(crate::harness_config::HarnessModelConfig {
            provider: "ollama".into(),
            model: "test-model".into(),
            options: json!({}),
        });
        plan.config.config.trace = HarnessTraceConfig {
            enabled: true,
            level: HarnessTraceLevel::Verbose,
            content: HarnessTraceContent::Full,
        };
        plan.loop_package = Some(ResolvedPackageInfo {
            key: "loop:@zack/review-loop@0.1.0".into(),
            kind: PackageKind::Loop,
            name: "@zack/review-loop".into(),
            version: "0.1.0".into(),
            root: loop_root,
        });
        let report_path = root.join("custom-report.json");
        let mut model = ScriptedModelRuntime::new(vec![ModelTurn {
            assistant_content: Some("final response".into()),
            actions: Vec::new(),
            usage: RunUsage::default(),
            finish_reason: Some("stop".into()),
            provider_metadata: BTreeMap::new(),
        }]);
        let mut dispatcher = ScriptedActionDispatcher::default();
        let result = execute_headless_plan(
            &plan,
            "write a response".into(),
            Some(&report_path),
            &mut model,
            &mut dispatcher,
        )
        .unwrap();
        assert_eq!(result.status, HarnessTerminalStatus::Ended);
        assert_eq!(result.output, Some(json!("final response")));
        assert!(report_path.exists());
        let events_path = plan
            .state_dir
            .join("runs")
            .join(&result.report.run_id)
            .join("events.jsonl");
        assert_eq!(
            result.report.trace_path.as_deref(),
            Some(events_path.to_string_lossy().as_ref())
        );
        let report_json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&report_path).unwrap()).unwrap();
        assert_eq!(
            report_json["trace_path"],
            events_path.to_string_lossy().as_ref()
        );
        let events = fs::read_to_string(&events_path).unwrap();
        assert!(events.contains("\"event_type\":\"run_started\""));
        assert!(events.contains("\"event_type\":\"run_completed\""));
        let parsed_events = events
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect::<Vec<serde_json::Value>>();
        let prompt_event = parsed_events
            .iter()
            .find(|event: &&serde_json::Value| event["event_type"] == "prompt_prepared")
            .unwrap();
        let prompt = prompt_event["payload"]["fields"]["prompt"]
            .as_str()
            .unwrap();
        assert!(prompt.contains("Harness authority"));
        assert!(prompt.contains("write a response"));
        let model_completed = parsed_events
            .iter()
            .find(|event: &&serde_json::Value| event["event_type"] == "model_request_completed")
            .unwrap();
        assert_eq!(
            model_completed["payload"]["fields"]["assistant_content"],
            "final response"
        );
        assert_eq!(
            model_completed["payload"]["fields"]["finish_reason"],
            "stop"
        );
        let phase_result = parsed_events
            .iter()
            .find(|event: &&serde_json::Value| event["event_type"] == "phase_result_ready")
            .unwrap();
        assert_eq!(phase_result["payload"]["output"], "final response");
        assert_eq!(model.requests.len(), 1);
        assert_eq!(model.requests[0].prompt.sections.len(), 6);
    }

    #[test]
    fn headless_execution_runs_three_phase_loop_and_writes_report() {
        let root = temp_dir("headless-three-phase");
        let loop_root = root.join(".agentpm/loops/zack/review-loop/0.1.0");
        write_json(
            &loop_root.join("agent.json"),
            json!({
                "kind": "loop",
                "name": "@zack/review-loop",
                "version": "0.1.0",
                "loop": {
                    "entry_phase": "assess",
                    "phases": [
                        {
                            "id": "assess",
                            "objective": "Assess the request.",
                            "outcomes": [
                                { "id": "draft", "description": "Draft a response." }
                            ]
                        },
                        {
                            "id": "draft",
                            "objective": "Draft the response.",
                            "outcomes": [
                                { "id": "review", "description": "Review the response." }
                            ]
                        },
                        { "id": "review", "objective": "Review the response." }
                    ],
                    "transitions": [
                        { "from": "assess", "on": "draft", "to": "draft" },
                        { "from": "draft", "on": "review", "to": "review" },
                        { "from": "review", "on": "complete", "to": "$end" }
                    ]
                }
            }),
        );
        let mut plan = minimal_plan(&root);
        plan.config.config.model = Some(crate::harness_config::HarnessModelConfig {
            provider: "ollama".into(),
            model: "test-model".into(),
            options: json!({}),
        });
        plan.loop_package = Some(ResolvedPackageInfo {
            key: "loop:@zack/review-loop@0.1.0".into(),
            kind: PackageKind::Loop,
            name: "@zack/review-loop".into(),
            version: "0.1.0".into(),
            root: loop_root,
        });
        let mut model = ScriptedModelRuntime::new(vec![
            phase_completion_turn(Some("draft"), Some(json!({ "assessment": "ok" }))),
            phase_completion_turn(Some("review"), Some(json!({ "draft": "ready" }))),
            phase_completion_turn(None, Some(json!({ "final": "approved" }))),
        ]);
        let mut dispatcher = ScriptedActionDispatcher::default();
        let result = execute_headless_plan(
            &plan,
            "prepare a response".into(),
            None,
            &mut model,
            &mut dispatcher,
        )
        .unwrap();

        assert_eq!(result.status, HarnessTerminalStatus::Ended);
        assert_eq!(result.output, Some(json!({ "final": "approved" })));
        assert_eq!(result.report.phase_summaries.len(), 3);
        assert_eq!(model.requests.len(), 3);
    }

    #[test]
    fn headless_execution_reports_approval_required_terminal_status() {
        let root = temp_dir("headless-approval-required");
        let loop_root = root.join(".agentpm/loops/zack/review-loop/0.1.0");
        write_json(
            &loop_root.join("agent.json"),
            json!({
                "kind": "loop",
                "name": "@zack/review-loop",
                "version": "0.1.0",
                "loop": {
                    "entry_phase": "assess",
                    "checkpoints": [
                        {
                            "id": "approve-review",
                            "type": "approval",
                            "before_phase": "review",
                            "on_reject": "$handoff"
                        }
                    ],
                    "phases": [
                        {
                            "id": "assess",
                            "objective": "Assess the request.",
                            "outcomes": [
                                { "id": "review", "description": "Review the response." }
                            ]
                        },
                        { "id": "review", "objective": "Review the response." }
                    ],
                    "transitions": [
                        { "from": "assess", "on": "review", "to": "review" },
                        { "from": "review", "on": "complete", "to": "$end" }
                    ]
                }
            }),
        );
        let mut plan = minimal_plan(&root);
        plan.config.config.model = Some(crate::harness_config::HarnessModelConfig {
            provider: "ollama".into(),
            model: "test-model".into(),
            options: json!({}),
        });
        plan.loop_package = Some(ResolvedPackageInfo {
            key: "loop:@zack/review-loop@0.1.0".into(),
            kind: PackageKind::Loop,
            name: "@zack/review-loop".into(),
            version: "0.1.0".into(),
            root: loop_root,
        });
        let report_path = root.join("approval-report.json");
        let mut model = ScriptedModelRuntime::new(vec![phase_completion_turn(
            Some("review"),
            Some(json!({ "assessment": "needs review" })),
        )]);
        let mut dispatcher = ScriptedActionDispatcher::default();
        let result = execute_headless_plan(
            &plan,
            "prepare a response".into(),
            Some(&report_path),
            &mut model,
            &mut dispatcher,
        )
        .unwrap();

        assert_eq!(result.status, HarnessTerminalStatus::ApprovalRequired);
        assert!(report_path.exists());
        assert_eq!(model.requests.len(), 1);
    }

    fn phase_completion_turn(
        outcome: Option<&str>,
        output: Option<serde_json::Value>,
    ) -> ModelTurn {
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "complete",
                SemanticAction::PhaseCompletion {
                    outcome: outcome.map(str::to_string),
                    output,
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("stop".into()),
            provider_metadata: BTreeMap::new(),
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "agentpm-harness-command-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn minimal_run_report(run_id: &str) -> RunReport {
        RunReport {
            report_version: crate::harness_observability::HARNESS_REPORT_SCHEMA_VERSION,
            session_id: "session-1".into(),
            run_id: run_id.into(),
            agent: ReportPackageIdentity {
                name: "@zack/test-agent".into(),
                version: "0.1.0".into(),
            },
            loop_package: ReportPackageIdentity {
                name: "@zack/review-loop".into(),
                version: "0.1.0".into(),
            },
            started_at: chrono::Utc::now(),
            ended_at: None,
            duration_ms: None,
            terminal_status: HarnessTerminalStatus::Failed,
            terminal_output: None,
            preflight_status: PreflightStatus::Ready,
            diagnostics: Vec::new(),
            runtime: Default::default(),
            runtime_sources: BTreeMap::new(),
            consumer_context: None,
            scope_summaries: Vec::new(),
            phase_summaries: Vec::new(),
            checkpoint_summaries: Vec::new(),
            action_summaries: Vec::new(),
            tool_summaries: Vec::new(),
            mcp_summaries: Vec::new(),
            knowledge_summaries: Vec::new(),
            memory_summaries: Vec::new(),
            usage: RunUsage::default(),
            retry_count: 0,
            repair_count: 0,
            error_count: 0,
            approval_summary: BTreeMap::new(),
            cancellation_summary: BTreeMap::new(),
            trace_path: None,
        }
    }

    fn minimal_plan(root: &Path) -> ResolvedHarnessPlan {
        let config = HarnessConfig {
            version: 1,
            ..HarnessConfig::default()
        };
        ResolvedHarnessPlan {
            workspace_root: root.to_path_buf(),
            lock_path: root.join("agent.lock"),
            state_dir: root.join(".agentpm-state"),
            config: ResolvedHarnessConfig {
                workspace_root: root.to_path_buf(),
                config_path: None,
                config,
                state_dir: root.join(".agentpm-state"),
                state_dir_source: HarnessConfigSource::cli_override(),
            },
            selected_agent: Some(crate::harness_plan::ResolvedAgentRoot {
                root_key: "local:agent:agent.json".into(),
                name: "@zack/test-agent".into(),
                version: "0.1.0".into(),
                manifest_path: root.join("agent.json"),
                package_key: None,
                tools: Vec::new(),
                skills: Vec::new(),
                knowledge: Vec::new(),
                memory: Vec::new(),
                profiles: Vec::new(),
                loop_key: "loop:@zack/review-loop@0.1.0".into(),
            }),
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
            profile_bindings: crate::harness_runtime::model::ProfileBindingSnapshot::default(),
            profiles: BTreeMap::new(),
            capabilities: Vec::new(),
            report: crate::harness_plan::PreflightReport {
                status: PreflightStatus::Ready,
                diagnostics: Vec::new(),
            },
        }
    }

    fn write_json(path: &Path, value: serde_json::Value) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    }

    fn host_service(role: &str, registry_id: &str) -> HostServiceRegistration {
        HostServiceRegistration {
            role: role.into(),
            registry_id: registry_id.into(),
        }
    }

    fn host_model_capabilities() -> Value {
        json!({
            "provider": "host-model",
            "model": "model-1",
            "semantic_actions": true,
            "structured_output": true,
            "multimodal_input": false,
            "usage_reporting": true
        })
    }

    type MachineBridgeFixture = (
        MachineHostBridgeHandle,
        mpsc::Sender<std::result::Result<MachineEnvelope, String>>,
        Arc<Mutex<Vec<u8>>>,
    );

    fn buffered_machine_bridge() -> MachineBridgeFixture {
        let (writer, output) = MachineProtocolWriter::buffer(HarnessTraceContent::Full);
        let (sender, receiver) = mpsc::channel();
        let cancellation_requested = Arc::new(AtomicBool::new(false));
        let active_run = Arc::new(AtomicBool::new(false));
        (
            MachineHostBridgeHandle::new(writer, receiver, cancellation_requested, active_run),
            sender,
            output,
        )
    }

    fn machine_request(id: &str, method: &str, payload: Value) -> MachineEnvelope {
        MachineEnvelope {
            protocol: AGENTPM_HARNESS_MACHINE_PROTOCOL.into(),
            version: AGENTPM_HARNESS_MACHINE_VERSION,
            kind: MachineFrameKind::Request,
            id: Some(id.into()),
            method: Some(method.into()),
            payload,
            error: None,
        }
    }

    fn machine_response(id: &str, payload: Value) -> MachineEnvelope {
        MachineEnvelope {
            protocol: AGENTPM_HARNESS_MACHINE_PROTOCOL.into(),
            version: AGENTPM_HARNESS_MACHINE_VERSION,
            kind: MachineFrameKind::Response,
            id: Some(id.into()),
            method: None,
            payload,
            error: None,
        }
    }

    fn machine_error(id: &str, code: &str, message: &str) -> MachineEnvelope {
        MachineEnvelope {
            protocol: AGENTPM_HARNESS_MACHINE_PROTOCOL.into(),
            version: AGENTPM_HARNESS_MACHINE_VERSION,
            kind: MachineFrameKind::Error,
            id: Some(id.into()),
            method: None,
            payload: Value::Null,
            error: Some(MachineError {
                code: code.into(),
                message: message.into(),
            }),
        }
    }

    fn machine_frames_from_buffer(output: &Arc<Mutex<Vec<u8>>>) -> Vec<Value> {
        let output = output.lock().unwrap();
        String::from_utf8_lossy(&output)
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn empty_model_request(selection: ModelProviderSelection) -> ModelRequest {
        ModelRequest {
            runtime: RuntimeSnapshot::empty("session-1".into()),
            model: Some(selection),
            prompt: crate::harness_runtime::model::LogicalPrompt {
                sections: Vec::new(),
                action_aliases: Vec::new(),
                completion: crate::harness_runtime::model::CompletionContract {
                    phase_id: "respond".into(),
                    explicit_outcomes: Vec::new(),
                    implicit_complete: true,
                },
                diagnostics: Vec::new(),
            },
            run_id: "run-1".into(),
            phase_execution_id: "phase-exec-1".into(),
            phase_id: "respond".into(),
            phase_objective: "Respond.".into(),
            run_input: "input".into(),
            prior_phase_results: Vec::new(),
            transcript: Vec::new(),
            effective_phase: crate::harness_engine::EffectivePhase {
                phase_id: "respond".into(),
                tools_allowed: Some(false),
                knowledge_allowed: None,
                memory_read_allowed: None,
                memory_write_allowed: None,
                authored_profile_candidates: Vec::new(),
                active_profiles: Vec::new(),
                active_tools: Vec::new(),
                active_skills: Vec::new(),
                active_knowledge: Vec::new(),
                capability_catalog: Vec::new(),
                suppressed_capabilities: Vec::new(),
            },
            repair_feedback: None,
        }
    }

    struct FakeHostInvoker {
        response: Value,
        capabilities: Option<Value>,
    }

    impl HostServiceInvoker for FakeHostInvoker {
        fn invoke_host_service(
            &mut self,
            _role: &str,
            _registry_id: &str,
            _method: &str,
            _payload: Value,
            _timeout_ms: u64,
        ) -> Result<Value> {
            Ok(self.response.clone())
        }

        fn host_service_capabilities(&self, _role: &str, _registry_id: &str) -> Option<Value> {
            self.capabilities.clone()
        }
    }

    struct UnsupportedModelRuntime {
        semantic_actions: bool,
        structured_output: bool,
    }

    impl ModelRuntime for UnsupportedModelRuntime {
        fn capabilities(&self) -> ModelCapabilityAdvertisement {
            ModelCapabilityAdvertisement {
                semantic_actions: self.semantic_actions,
                structured_output: self.structured_output,
                multimodal_input: false,
                context_window_tokens: None,
                usage_reporting: false,
            }
        }

        fn generate(
            &mut self,
            _request: crate::harness_runtime::ModelRequest,
        ) -> std::result::Result<ModelTurn, ModelRuntimeFailure> {
            unreachable!("capability validation should fail before generation")
        }
    }
}
