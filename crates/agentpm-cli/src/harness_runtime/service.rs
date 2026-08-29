#![allow(dead_code)]

use crate::harness_config::{HarnessImplementation, HarnessRestartPolicy};
use crate::harness_observability::HarnessEventType;
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

pub const AGENTPM_SERVICE_PROTOCOL: &str = "agentpm-service";
pub const AGENTPM_SERVICE_VERSION: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEnvelope {
    pub protocol: String,
    pub version: u8,
    pub kind: ServiceEnvelopeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub service: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default)]
    pub payload: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ServiceError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceEnvelopeKind {
    Initialize,
    Initialized,
    Request,
    Response,
    Event,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceError {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub retryable: bool,
}

#[derive(Debug, Clone)]
pub struct ProcessServiceConfig {
    pub service: String,
    pub registry_id: String,
    pub initialize_payload: Map<String, Value>,
    pub implementation: HarnessImplementation,
    pub workspace_root: PathBuf,
    pub lifecycle_events: Option<ServiceLifecycleEmitter>,
}

pub struct ProcessServiceClient {
    config: ProcessServiceConfig,
    process: Option<RunningProcess>,
    initialization_result: Value,
    request_counter: u64,
    restart_attempts: u32,
}

pub trait HostServiceInvoker {
    fn invoke_host_service(
        &mut self,
        role: &str,
        registry_id: &str,
        method: &str,
        payload: Value,
        timeout_ms: u64,
    ) -> Result<Value>;
}

struct RunningProcess {
    child: Child,
    stdin: ChildStdin,
    receiver: Receiver<std::result::Result<ServiceEnvelope, String>>,
}

#[derive(Debug, Clone)]
pub struct ServiceLifecycleEvent {
    pub event_type: HarnessEventType,
    pub service: String,
    pub registry_id: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug)]
pub struct ServiceLifecycleEvents {
    sender: Sender<ServiceLifecycleEvent>,
    receiver: Receiver<ServiceLifecycleEvent>,
}

impl ServiceLifecycleEvents {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self { sender, receiver }
    }

    pub fn emitter(&self) -> ServiceLifecycleEmitter {
        ServiceLifecycleEmitter {
            sender: self.sender.clone(),
        }
    }

    pub fn drain(&mut self) -> Vec<ServiceLifecycleEvent> {
        self.receiver.try_iter().collect()
    }
}

impl Default for ServiceLifecycleEvents {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct ServiceLifecycleEmitter {
    sender: Sender<ServiceLifecycleEvent>,
}

impl ServiceLifecycleEmitter {
    fn emit(
        &self,
        event_type: HarnessEventType,
        service: &str,
        registry_id: &str,
        status: &str,
        message: impl Into<String>,
    ) {
        let _ = self.sender.send(ServiceLifecycleEvent {
            event_type,
            service: service.to_string(),
            registry_id: registry_id.to_string(),
            status: status.to_string(),
            message: message.into(),
        });
    }
}

impl ProcessServiceClient {
    pub fn start(config: ProcessServiceConfig) -> Result<Self> {
        let mut client = Self {
            config,
            process: None,
            initialization_result: Value::Null,
            request_counter: 0,
            restart_attempts: 0,
        };
        client.start_process()?;
        Ok(client)
    }

    pub fn initialization_result(&self) -> &Value {
        &self.initialization_result
    }

    pub fn request(&mut self, method: &str, payload: Value) -> Result<Value> {
        let id = self.next_request_id();
        let result = self.request_once(&id, method, payload);
        if result.is_ok() {
            return result;
        }
        self.emit_lifecycle(
            HarnessEventType::ServiceUnhealthy,
            "unhealthy",
            "Service request failed; restart policy will be evaluated.",
        );
        self.restart_after_failed_request()?;
        result
    }

    fn request_once(&mut self, id: &str, method: &str, payload: Value) -> Result<Value> {
        let timeout = self.request_timeout()?;
        let envelope = ServiceEnvelope {
            protocol: AGENTPM_SERVICE_PROTOCOL.into(),
            version: AGENTPM_SERVICE_VERSION,
            kind: ServiceEnvelopeKind::Request,
            id: Some(id.to_string()),
            service: self.config.service.clone(),
            method: Some(method.to_string()),
            payload,
            result: None,
            error: None,
        };
        let process = self
            .process
            .as_mut()
            .ok_or_else(|| anyhow!("service process is not running"))?;
        write_envelope(&mut process.stdin, &envelope)?;
        read_correlated_response(&process.receiver, id, timeout)
    }

    fn start_process(&mut self) -> Result<()> {
        self.stop_process(false);
        let HarnessImplementation::Process {
            command,
            args,
            cwd,
            env,
            ..
        } = &self.config.implementation
        else {
            bail!("process service client requires a process implementation");
        };

        self.emit_lifecycle(
            HarnessEventType::ServiceStarting,
            "starting",
            "Service process starting.",
        );
        let mut command = Command::new(command);
        command.args(args);
        command.current_dir(resolve_process_cwd(
            &self.config.workspace_root,
            cwd.as_deref(),
        ));
        for name in env {
            if let Ok(value) = std::env::var(name) {
                command.env(name, value);
            }
        }
        let mut child = match command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
        {
            Ok(child) => child,
            Err(err) => {
                self.emit_lifecycle(
                    HarnessEventType::ServiceFailed,
                    "failed",
                    format!("Service process failed to start: {err}"),
                );
                return Err(err).with_context(|| {
                    format!(
                        "starting {} process service `{}`",
                        self.config.service, self.config.registry_id
                    )
                });
            }
        };
        let stdin = match child.stdin.take().context("opening service stdin") {
            Ok(stdin) => stdin,
            Err(err) => {
                self.emit_lifecycle(
                    HarnessEventType::ServiceFailed,
                    "failed",
                    format!("Service stdin failed to open: {err}"),
                );
                return Err(err);
            }
        };
        let stdout = match child.stdout.take().context("opening service stdout") {
            Ok(stdout) => stdout,
            Err(err) => {
                self.emit_lifecycle(
                    HarnessEventType::ServiceFailed,
                    "failed",
                    format!("Service stdout failed to open: {err}"),
                );
                return Err(err);
            }
        };
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        let parsed = serde_json::from_str::<ServiceEnvelope>(line.trim_end())
                            .map_err(|err| format!("malformed service frame: {err}"));
                        if sender.send(parsed).is_err() {
                            break;
                        }
                    }
                    Err(err) => {
                        let _ = sender.send(Err(format!("reading service stdout failed: {err}")));
                        break;
                    }
                }
            }
        });
        self.process = Some(RunningProcess {
            child,
            stdin,
            receiver,
        });
        self.emit_lifecycle(
            HarnessEventType::ServiceHandshaking,
            "handshaking",
            "Service process handshake starting.",
        );
        if let Err(err) = self.handshake() {
            self.emit_lifecycle(
                HarnessEventType::ServiceFailed,
                "failed",
                format!(
                    "Service handshake failed for `{}`: {err}",
                    self.config.registry_id
                ),
            );
            return Err(err);
        }
        self.emit_lifecycle(
            HarnessEventType::ServiceReady,
            "ready",
            "Service process is ready.",
        );
        Ok(())
    }

    fn handshake(&mut self) -> Result<()> {
        let id = self.next_request_id();
        let timeout = self.startup_timeout()?;
        let mut payload = Map::from_iter([
            ("role".into(), json!(self.config.service.clone())),
            ("registry_id".into(), json!(self.config.registry_id.clone())),
            ("protocol_version".into(), json!(AGENTPM_SERVICE_VERSION)),
        ]);
        payload.extend(self.config.initialize_payload.clone());
        let envelope = ServiceEnvelope {
            protocol: AGENTPM_SERVICE_PROTOCOL.into(),
            version: AGENTPM_SERVICE_VERSION,
            kind: ServiceEnvelopeKind::Initialize,
            id: Some(id.clone()),
            service: self.config.service.clone(),
            method: Some("initialize".into()),
            payload: Value::Object(payload),
            result: None,
            error: None,
        };
        let process = self
            .process
            .as_mut()
            .ok_or_else(|| anyhow!("service process is not running"))?;
        write_envelope(&mut process.stdin, &envelope)?;
        self.initialization_result = read_correlated_response(&process.receiver, &id, timeout)?;
        Ok(())
    }

    fn restart_after_failed_request(&mut self) -> Result<()> {
        let policy = self.restart_policy()?;
        if self.restart_attempts >= policy.max_attempts {
            self.emit_lifecycle(
                HarnessEventType::ServiceFailed,
                "failed",
                "Service restart attempts exhausted.",
            );
            return Ok(());
        }
        self.restart_attempts += 1;
        self.emit_lifecycle(
            HarnessEventType::ServiceRestarting,
            "restarting",
            "Service process restarting after failed request.",
        );
        if policy.backoff_ms > 0 {
            std::thread::sleep(Duration::from_millis(policy.backoff_ms));
        }
        self.start_process()
    }

    fn next_request_id(&mut self) -> String {
        self.request_counter += 1;
        format!(
            "{}-{}-{}",
            self.config.service, self.config.registry_id, self.request_counter
        )
    }

    fn startup_timeout(&self) -> Result<Duration> {
        match &self.config.implementation {
            HarnessImplementation::Process {
                startup_timeout_ms, ..
            } => Ok(Duration::from_millis(*startup_timeout_ms)),
            HarnessImplementation::Host { .. } => {
                bail!("host implementation has no process timeout")
            }
        }
    }

    fn request_timeout(&self) -> Result<Duration> {
        match &self.config.implementation {
            HarnessImplementation::Process {
                request_timeout_ms, ..
            }
            | HarnessImplementation::Host {
                request_timeout_ms, ..
            } => Ok(Duration::from_millis(*request_timeout_ms)),
        }
    }

    fn restart_policy(&self) -> Result<HarnessRestartPolicy> {
        match &self.config.implementation {
            HarnessImplementation::Process { restart, .. } => Ok(restart.clone()),
            HarnessImplementation::Host { .. } => {
                bail!("host implementation has no process restart policy")
            }
        }
    }

    fn stop_process(&mut self, emit_stopped: bool) {
        if let Some(mut process) = self.process.take() {
            let _ = process.child.kill();
            let _ = process.child.wait();
            if emit_stopped {
                self.emit_lifecycle(
                    HarnessEventType::ServiceStopped,
                    "stopped",
                    "Service process stopped.",
                );
            }
        }
    }

    fn emit_lifecycle(
        &self,
        event_type: HarnessEventType,
        status: &str,
        message: impl Into<String>,
    ) {
        if let Some(events) = &self.config.lifecycle_events {
            events.emit(
                event_type,
                &self.config.service,
                &self.config.registry_id,
                status,
                message,
            );
        }
    }
}

impl Drop for ProcessServiceClient {
    fn drop(&mut self) {
        self.stop_process(true);
    }
}

fn resolve_process_cwd(workspace_root: &Path, cwd: Option<&str>) -> PathBuf {
    cwd.map(|cwd| workspace_root.join(cwd))
        .unwrap_or_else(|| workspace_root.to_path_buf())
}

fn write_envelope(stdin: &mut ChildStdin, envelope: &ServiceEnvelope) -> Result<()> {
    serde_json::to_writer(&mut *stdin, envelope).context("writing service frame")?;
    stdin.write_all(b"\n").context("writing service newline")?;
    stdin.flush().context("flushing service stdin")
}

fn read_correlated_response(
    receiver: &Receiver<std::result::Result<ServiceEnvelope, String>>,
    id: &str,
    timeout: Duration,
) -> Result<Value> {
    let started = Instant::now();
    loop {
        let remaining = timeout.checked_sub(started.elapsed()).ok_or_else(|| {
            anyhow!(
                "service request `{id}` timed out after {} ms",
                timeout.as_millis()
            )
        })?;
        let envelope = receiver
            .recv_timeout(remaining)
            .with_context(|| format!("waiting for service response `{id}`"))?
            .map_err(anyhow::Error::msg)?;
        validate_service_envelope(&envelope)?;
        if envelope.id.as_deref() != Some(id) {
            continue;
        }
        return match envelope.kind {
            ServiceEnvelopeKind::Initialized | ServiceEnvelopeKind::Response => {
                Ok(envelope.result.unwrap_or(envelope.payload))
            }
            ServiceEnvelopeKind::Error => {
                let error = envelope.error.unwrap_or(ServiceError {
                    code: "service_error".into(),
                    message: "service returned an error without an error payload".into(),
                    retryable: false,
                });
                Err(anyhow!("{}: {}", error.code, error.message))
            }
            other => Err(anyhow!(
                "service response `{id}` used invalid frame kind `{other:?}`"
            )),
        };
    }
}

fn validate_service_envelope(envelope: &ServiceEnvelope) -> Result<()> {
    if envelope.protocol != AGENTPM_SERVICE_PROTOCOL {
        bail!(
            "unsupported service protocol `{}`; expected `{AGENTPM_SERVICE_PROTOCOL}`",
            envelope.protocol
        );
    }
    if envelope.version != AGENTPM_SERVICE_VERSION {
        bail!(
            "unsupported service protocol version {}; expected {AGENTPM_SERVICE_VERSION}",
            envelope.version
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn validates_protocol_and_version() {
        let mut envelope = ServiceEnvelope {
            protocol: AGENTPM_SERVICE_PROTOCOL.into(),
            version: AGENTPM_SERVICE_VERSION,
            kind: ServiceEnvelopeKind::Response,
            id: Some("req".into()),
            service: "model".into(),
            method: None,
            payload: Value::Null,
            result: None,
            error: None,
        };
        assert!(validate_service_envelope(&envelope).is_ok());
        envelope.version = 2;
        assert!(validate_service_envelope(&envelope).is_err());
    }

    #[test]
    fn process_model_service_round_trips_correlated_request() {
        let temp =
            std::env::temp_dir().join(format!("agentpm-service-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&temp);
        let script = temp.join("service.py");
        fs::write(
            &script,
            r#"
import json, sys
for line in sys.stdin:
    msg = json.loads(line)
    kind = "initialized" if msg["kind"] == "initialize" else "response"
    result = {"ok": True} if kind == "initialized" else {"echo": msg["payload"]}
    print(json.dumps({
        "protocol": "agentpm-service",
        "version": 1,
        "kind": kind,
        "id": msg.get("id"),
        "service": msg["service"],
        "result": result
    }), flush=True)
"#,
        )
        .unwrap();
        let mut client = ProcessServiceClient::start(ProcessServiceConfig {
            service: "model".into(),
            registry_id: "test".into(),
            initialize_payload: Map::new(),
            implementation: HarnessImplementation::Process {
                command: "python3".into(),
                args: vec![script.display().to_string()],
                cwd: None,
                env: Vec::new(),
                startup_timeout_ms: 1_000,
                request_timeout_ms: 1_000,
                restart: HarnessRestartPolicy::default(),
            },
            workspace_root: temp,
            lifecycle_events: None,
        })
        .unwrap();
        assert_eq!(client.initialization_result()["ok"], json!(true));
        let response = client.request("generate", json!({ "value": 42 })).unwrap();
        assert_eq!(response["echo"]["value"], json!(42));
    }

    #[test]
    fn service_lifecycle_reports_handshake_ready_and_stopped() {
        let temp = std::env::temp_dir().join(format!(
            "agentpm-service-lifecycle-test-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&temp);
        let script = temp.join("service.py");
        fs::write(
            &script,
            r#"
import json, sys
for line in sys.stdin:
    msg = json.loads(line)
    if msg["kind"] == "initialize":
        print(json.dumps({
            "protocol": "agentpm-service",
            "version": 1,
            "kind": "initialized",
            "id": msg.get("id"),
            "service": msg["service"],
            "result": {"ok": True}
        }), flush=True)
"#,
        )
        .unwrap();
        let mut service_events = ServiceLifecycleEvents::new();
        {
            let _client = ProcessServiceClient::start(ProcessServiceConfig {
                service: "model".into(),
                registry_id: "test".into(),
                initialize_payload: Map::new(),
                implementation: HarnessImplementation::Process {
                    command: "python3".into(),
                    args: vec![script.display().to_string()],
                    cwd: None,
                    env: Vec::new(),
                    startup_timeout_ms: 1_000,
                    request_timeout_ms: 1_000,
                    restart: HarnessRestartPolicy {
                        max_attempts: 1,
                        backoff_ms: 0,
                    },
                },
                workspace_root: temp,
                lifecycle_events: Some(service_events.emitter()),
            })
            .unwrap();
        }

        let event_types = service_events
            .drain()
            .into_iter()
            .map(|event| event.event_type)
            .collect::<Vec<_>>();
        assert_eq!(
            event_types,
            vec![
                HarnessEventType::ServiceStarting,
                HarnessEventType::ServiceHandshaking,
                HarnessEventType::ServiceReady,
                HarnessEventType::ServiceStopped,
            ]
        );
    }

    #[test]
    fn service_request_timeout_is_reported_and_restart_is_attempted() {
        let temp = std::env::temp_dir().join(format!(
            "agentpm-service-timeout-test-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&temp);
        let script = temp.join("service.py");
        fs::write(
            &script,
            r#"
import json, sys, time
for line in sys.stdin:
    msg = json.loads(line)
    if msg["kind"] == "initialize":
        print(json.dumps({
            "protocol": "agentpm-service",
            "version": 1,
            "kind": "initialized",
            "id": msg.get("id"),
            "service": msg["service"],
            "result": {"ok": True}
        }), flush=True)
    else:
        time.sleep(1)
"#,
        )
        .unwrap();
        let mut service_events = ServiceLifecycleEvents::new();
        let mut client = ProcessServiceClient::start(ProcessServiceConfig {
            service: "model".into(),
            registry_id: "test".into(),
            initialize_payload: Map::new(),
            implementation: HarnessImplementation::Process {
                command: "python3".into(),
                args: vec![script.display().to_string()],
                cwd: None,
                env: Vec::new(),
                startup_timeout_ms: 1_000,
                request_timeout_ms: 50,
                restart: HarnessRestartPolicy {
                    max_attempts: 1,
                    backoff_ms: 0,
                },
            },
            workspace_root: temp,
            lifecycle_events: Some(service_events.emitter()),
        })
        .unwrap();

        let err = client
            .request("generate", json!({ "value": 1 }))
            .unwrap_err();

        assert!(
            err.chain()
                .any(|cause| cause.to_string().contains("timed out"))
        );
        let event_types = service_events
            .drain()
            .into_iter()
            .map(|event| event.event_type)
            .collect::<Vec<_>>();
        assert!(event_types.contains(&HarnessEventType::ServiceUnhealthy));
        assert!(event_types.contains(&HarnessEventType::ServiceRestarting));
    }

    #[test]
    fn service_restart_does_not_replay_failed_in_flight_request() {
        let temp = std::env::temp_dir().join(format!(
            "agentpm-service-no-replay-test-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&temp);
        let request_log = temp.join("requests.jsonl");
        let script = temp.join("service.py");
        fs::write(
            &script,
            format!(
                r#"
import json, sys
request_log = {request_log:?}
for line in sys.stdin:
    msg = json.loads(line)
    if msg["kind"] == "initialize":
        print(json.dumps({{
            "protocol": "agentpm-service",
            "version": 1,
            "kind": "initialized",
            "id": msg.get("id"),
            "service": msg["service"],
            "result": {{"ok": True}}
        }}), flush=True)
    else:
        with open(request_log, "a", encoding="utf-8") as handle:
            handle.write(json.dumps(msg["payload"]) + "\n")
        print(json.dumps({{
            "protocol": "agentpm-service",
            "version": 1,
            "kind": "error",
            "id": msg.get("id"),
            "service": msg["service"],
            "error": {{
                "code": "request_failed",
                "message": "always fails",
                "retryable": True
            }}
        }}), flush=True)
"#,
                request_log = request_log.display().to_string()
            ),
        )
        .unwrap();
        let mut client = ProcessServiceClient::start(ProcessServiceConfig {
            service: "model".into(),
            registry_id: "test".into(),
            initialize_payload: Map::new(),
            implementation: HarnessImplementation::Process {
                command: "python3".into(),
                args: vec![script.display().to_string()],
                cwd: None,
                env: Vec::new(),
                startup_timeout_ms: 1_000,
                request_timeout_ms: 1_000,
                restart: HarnessRestartPolicy {
                    max_attempts: 1,
                    backoff_ms: 0,
                },
            },
            workspace_root: temp,
            lifecycle_events: None,
        })
        .unwrap();

        assert!(client.request("generate", json!({ "value": 1 })).is_err());

        let requests = fs::read_to_string(request_log).unwrap();
        assert_eq!(requests.lines().count(), 1);
        assert_eq!(
            serde_json::from_str::<Value>(requests.lines().next().unwrap()).unwrap(),
            json!({ "value": 1 })
        );
    }

    #[test]
    fn service_restart_budget_accumulates_across_successful_restarts() {
        let temp = std::env::temp_dir().join(format!(
            "agentpm-service-restart-budget-test-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&temp);
        let script = temp.join("service.py");
        fs::write(
            &script,
            r#"
import json, sys
for line in sys.stdin:
    msg = json.loads(line)
    if msg["kind"] == "initialize":
        print(json.dumps({
            "protocol": "agentpm-service",
            "version": 1,
            "kind": "initialized",
            "id": msg.get("id"),
            "service": msg["service"],
            "result": {"ok": True}
        }), flush=True)
    else:
        print(json.dumps({
            "protocol": "agentpm-service",
            "version": 1,
            "kind": "error",
            "id": msg.get("id"),
            "service": msg["service"],
            "error": {
                "code": "request_failed",
                "message": "always fails",
                "retryable": True
            }
        }), flush=True)
"#,
        )
        .unwrap();
        let mut service_events = ServiceLifecycleEvents::new();
        let mut client = ProcessServiceClient::start(ProcessServiceConfig {
            service: "model".into(),
            registry_id: "test".into(),
            initialize_payload: Map::new(),
            implementation: HarnessImplementation::Process {
                command: "python3".into(),
                args: vec![script.display().to_string()],
                cwd: None,
                env: Vec::new(),
                startup_timeout_ms: 1_000,
                request_timeout_ms: 1_000,
                restart: HarnessRestartPolicy {
                    max_attempts: 1,
                    backoff_ms: 0,
                },
            },
            workspace_root: temp,
            lifecycle_events: Some(service_events.emitter()),
        })
        .unwrap();

        assert!(client.request("generate", json!({ "value": 1 })).is_err());
        assert!(client.request("generate", json!({ "value": 2 })).is_err());

        let restarting_events = service_events
            .drain()
            .into_iter()
            .filter(|event| event.event_type == HarnessEventType::ServiceRestarting)
            .count();
        assert_eq!(restarting_events, 1);
        assert_eq!(client.restart_attempts, 1);
    }
}
