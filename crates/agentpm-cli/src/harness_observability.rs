#![allow(dead_code)]

use crate::harness_config::{HarnessTraceConfig, HarnessTraceContent, HarnessTraceLevel};
use crate::harness_plan::{PreflightDiagnostic, PreflightStatus};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub const HARNESS_EVENT_SCHEMA_VERSION: u8 = 1;
pub const HARNESS_REPORT_SCHEMA_VERSION: u8 = 1;

static HARNESS_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn allocate_harness_session_id() -> String {
    allocate_harness_id("sess")
}

pub fn allocate_harness_run_id() -> String {
    allocate_harness_id("run")
}

fn allocate_harness_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = HARNESS_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{nanos:x}-{counter:x}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessTerminalStatus {
    Ended,
    HandedOff,
    Aborted,
    Failed,
    Cancelled,
    LimitReached,
    ApprovalRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessEventType {
    SessionStarting,
    ServiceStarting,
    ServiceHandshaking,
    ServiceReady,
    ServiceUnhealthy,
    ServiceRestarting,
    ServiceFailed,
    ServiceStopped,
    PreflightCompleted,
    SessionStarted,
    SessionUsageUpdated,
    SessionStopping,
    SessionStopped,
    RunStarted,
    ConsumerContextLoaded,
    ConsumerContextUnavailable,
    PhaseEnterRequested,
    EffectivePhaseComputed,
    PhaseStarted,
    PhaseResultReady,
    PhaseFailed,
    RunCompleted,
    RunFailed,
    RunCancelled,
    RunLimitReached,
    RunApprovalRequired,
    PromptPrepared,
    ModelRequestStarted,
    ModelRequestCompleted,
    ModelRequestFailed,
    SemanticActionProposed,
    SemanticActionRejected,
    SemanticActionCompleted,
    ModelRepairRequested,
    OutcomeProposed,
    OutcomeSelected,
    OutcomeInvalid,
    TransitionSelected,
    LoopLimitReached,
    ToolCandidatesComputed,
    ToolInvoked,
    ToolRetrying,
    ToolCompleted,
    ToolFailed,
    SkillActivated,
    SkillResourceRequested,
    SkillResourceLoaded,
    SkillResourceFailed,
    KnowledgeSurfaceReady,
    KnowledgeSurfaceUnavailable,
    KnowledgeRequestStarted,
    KnowledgeRetrieved,
    KnowledgeFailed,
    EmbeddingRequestStarted,
    EmbeddingRequestCompleted,
    EmbeddingRequestFailed,
    MemorySurfaceReady,
    MemorySurfaceUnavailable,
    MemoryReadStarted,
    MemoryReadCompleted,
    MemoryReadFailed,
    MemoryWriteStarted,
    MemoryWriteCompleted,
    MemoryWriteFailed,
    MemoryTriggerEvaluated,
    MemoryOperationEligible,
    MemoryOperationStarted,
    MemoryOperationCompleted,
    MemoryOperationFailed,
    HookStarted,
    HookCompleted,
    HookRejected,
    HookFailed,
    ApprovalRequested,
    ApprovalApproved,
    ApprovalDenied,
    ApprovalFailed,
    McpSurfaceStarting,
    McpSurfaceReady,
    McpSurfaceFailed,
    McpSurfaceStopped,
    McpImportConnected,
    McpImportFailed,
    McpToolInvoked,
    McpToolCompleted,
    McpToolFailed,
    CancellationRequested,
    CancellationCompleted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarnessEventEnvelope {
    pub schema_version: u8,
    pub event_id: String,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub session_sequence: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_sequence: Option<u64>,
    pub timestamp: DateTime<Utc>,
    pub event_type: HarnessEventType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase_execution_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_event_id: Option<String>,
    pub payload: HarnessEventPayload,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "payload_type", rename_all = "snake_case")]
pub enum HarnessEventPayload {
    Empty,
    Lifecycle {
        message: String,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        fields: BTreeMap<String, Value>,
    },
    Service {
        service: String,
        status: String,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        fields: BTreeMap<String, Value>,
    },
    Preflight {
        status: PreflightStatus,
        fatal_count: usize,
        warning_count: usize,
        suppressed_count: usize,
        pending_count: usize,
    },
    Phase {
        phase_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        outcome: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        transition_to: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<Value>,
    },
    Action {
        action_kind: String,
        identity: String,
        status: String,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        fields: BTreeMap<String, Value>,
    },
    Content {
        label: String,
        content: Value,
    },
    Usage {
        run_usage: Box<RunUsage>,
        session_usage: Box<SessionUsage>,
    },
    Terminal {
        status: HarnessTerminalStatus,
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<Value>,
    },
}

#[derive(Debug, Clone, Default)]
pub struct HarnessEventBuilder {
    pub run_id: Option<String>,
    pub phase_execution_id: Option<String>,
    pub correlation_id: Option<String>,
    pub parent_event_id: Option<String>,
}

pub trait HarnessEventSink {
    fn record(&mut self, event: &HarnessEventEnvelope) -> Result<()>;
    fn flush(&mut self) -> Result<()>;
}

pub struct HarnessEventEmitter {
    session_id: String,
    session_sequence: u64,
    run_sequences: BTreeMap<String, u64>,
    sinks: Vec<Box<dyn HarnessEventSink>>,
}

impl HarnessEventEmitter {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            session_sequence: 0,
            run_sequences: BTreeMap::new(),
            sinks: Vec::new(),
        }
    }

    pub fn add_sink(&mut self, sink: Box<dyn HarnessEventSink>) {
        self.sinks.push(sink);
    }

    pub fn emit(
        &mut self,
        event_type: HarnessEventType,
        payload: HarnessEventPayload,
        builder: HarnessEventBuilder,
    ) -> Result<HarnessEventEnvelope> {
        self.session_sequence += 1;
        let run_sequence = builder.run_id.as_ref().map(|run_id| {
            let next = self.run_sequences.entry(run_id.clone()).or_insert(0);
            *next += 1;
            *next
        });
        let event = HarnessEventEnvelope {
            schema_version: HARNESS_EVENT_SCHEMA_VERSION,
            event_id: format!("evt-{}-{}", self.session_id, self.session_sequence),
            session_id: self.session_id.clone(),
            run_id: builder.run_id,
            session_sequence: self.session_sequence,
            run_sequence,
            timestamp: Utc::now(),
            event_type,
            phase_execution_id: builder.phase_execution_id,
            correlation_id: builder.correlation_id,
            parent_event_id: builder.parent_event_id,
            payload,
        };
        for sink in &mut self.sinks {
            sink.record(&event)?;
        }
        Ok(event)
    }

    pub fn flush(&mut self) -> Result<()> {
        for sink in &mut self.sinks {
            sink.flush()?;
        }
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct InMemoryEventSink {
    events: Arc<Mutex<Vec<HarnessEventEnvelope>>>,
}

impl InMemoryEventSink {
    pub fn events(&self) -> Vec<HarnessEventEnvelope> {
        self.events.lock().expect("event sink poisoned").clone()
    }
}

impl HarnessEventSink for InMemoryEventSink {
    fn record(&mut self, event: &HarnessEventEnvelope) -> Result<()> {
        self.events
            .lock()
            .expect("event sink poisoned")
            .push(event.clone());
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

pub struct JsonlTraceSink {
    writer: BufWriter<File>,
    config: HarnessTraceConfig,
}

impl JsonlTraceSink {
    pub fn create(path: &Path, config: HarnessTraceConfig) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating trace directory {}", parent.display()))?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)
            .with_context(|| format!("opening trace file {}", path.display()))?;
        Ok(Self {
            writer: BufWriter::new(file),
            config,
        })
    }
}

impl HarnessEventSink for JsonlTraceSink {
    fn record(&mut self, event: &HarnessEventEnvelope) -> Result<()> {
        if !self.config.enabled || !trace_level_includes(&self.config.level, event.event_type) {
            return Ok(());
        }
        let event = apply_content_policy(event, &self.config.content)?;
        serde_json::to_writer(&mut self.writer, &event).context("writing trace event JSON")?;
        self.writer
            .write_all(b"\n")
            .context("writing trace event newline")?;
        self.writer.flush().context("flushing trace event")?;
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        self.writer.flush().context("flushing trace sink")
    }
}

fn trace_level_includes(level: &HarnessTraceLevel, event_type: HarnessEventType) -> bool {
    match level {
        HarnessTraceLevel::Verbose => true,
        HarnessTraceLevel::Normal => !matches!(
            event_type,
            HarnessEventType::PromptPrepared
                | HarnessEventType::SemanticActionProposed
                | HarnessEventType::ModelRepairRequested
        ),
        HarnessTraceLevel::Minimal => matches!(
            event_type,
            HarnessEventType::SessionStarting
                | HarnessEventType::PreflightCompleted
                | HarnessEventType::SessionStarted
                | HarnessEventType::SessionUsageUpdated
                | HarnessEventType::SessionStopping
                | HarnessEventType::SessionStopped
                | HarnessEventType::RunStarted
                | HarnessEventType::PhaseStarted
                | HarnessEventType::PhaseResultReady
                | HarnessEventType::TransitionSelected
                | HarnessEventType::RunCompleted
                | HarnessEventType::RunFailed
                | HarnessEventType::RunCancelled
                | HarnessEventType::RunLimitReached
                | HarnessEventType::RunApprovalRequired
                | HarnessEventType::CancellationRequested
                | HarnessEventType::CancellationCompleted
        ),
    }
}

pub fn apply_content_policy(
    event: &HarnessEventEnvelope,
    policy: &HarnessTraceContent,
) -> Result<HarnessEventEnvelope> {
    let mut value = serde_json::to_value(event).context("serializing event for redaction")?;
    apply_content_policy_to_value(&mut value, policy);
    serde_json::from_value(value).context("deserializing redacted event")
}

pub fn apply_content_policy_to_value(value: &mut Value, policy: &HarnessTraceContent) {
    sanitize_value(value, policy, None);
}

fn sanitize_value(value: &mut Value, policy: &HarnessTraceContent, key: Option<&str>) {
    if let Some(key) = key
        && is_secret_key(key)
    {
        *value = Value::String("[secret redacted]".into());
        return;
    }
    if let Some(key) = key
        && is_content_key(key)
    {
        match policy {
            HarnessTraceContent::None => *value = Value::Null,
            HarnessTraceContent::Redacted => *value = Value::String("[redacted]".into()),
            HarnessTraceContent::Full => {}
        }
        if *policy != HarnessTraceContent::Full {
            return;
        }
    }

    match value {
        Value::Object(map) => sanitize_object(map, policy),
        Value::Array(items) => {
            for item in items {
                sanitize_value(item, policy, None);
            }
        }
        _ => {}
    }
}

fn sanitize_object(map: &mut Map<String, Value>, policy: &HarnessTraceContent) {
    for (key, value) in map.iter_mut() {
        sanitize_value(value, policy, Some(key));
    }
    if *policy == HarnessTraceContent::None {
        map.retain(|key, _| !is_content_key(key));
    }
}

fn is_content_key(key: &str) -> bool {
    matches!(
        key,
        "content"
            | "prompt"
            | "active_profiles"
            | "assistant_content"
            | "arguments"
            | "argument"
            | "query"
            | "result"
            | "vector"
            | "vectors"
            | "embedding_vector"
            | "embedding_values"
            | "input"
            | "output"
            | "text"
            | "value"
    )
}

fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("secret")
        || key == "token"
        || key.ends_with("_token")
        || key.ends_with("-token")
        || key.contains("password")
        || key.contains("api_key")
        || key == "key"
        || key == "authorization"
        || key == "cookie"
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TokenUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

impl Default for TokenUsage {
    fn default() -> Self {
        Self::unknown()
    }
}

impl TokenUsage {
    pub fn unknown() -> Self {
        Self {
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CostUsage {
    pub amount: Option<f64>,
    pub currency: Option<String>,
}

impl Default for CostUsage {
    fn default() -> Self {
        Self::unknown()
    }
}

impl CostUsage {
    pub fn unknown() -> Self {
        Self {
            amount: None,
            currency: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RunUsage {
    pub model_calls: u64,
    pub tokens: TokenUsage,
    pub accepted_semantic_actions: u64,
    pub tool_calls: u64,
    pub tool_retries: u64,
    pub knowledge_requests: u64,
    pub memory_requests: u64,
    pub embedding_requests: u64,
    pub duration_ms: Option<u64>,
    pub cost: CostUsage,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionUsage {
    pub started_runs: u64,
    pub completed_runs: u64,
    pub model_calls: u64,
    pub tokens: TokenUsage,
    pub accepted_semantic_actions: u64,
    pub tool_calls: u64,
    pub tool_retries: u64,
    pub knowledge_requests: u64,
    pub memory_requests: u64,
    pub embedding_requests: u64,
    pub duration_ms: Option<u64>,
    pub cost: CostUsage,
}

impl SessionUsage {
    pub fn record_run_started(&mut self) {
        self.started_runs += 1;
    }

    pub fn record_run_completed(&mut self, run_usage: &RunUsage) {
        self.completed_runs += 1;
        self.model_calls += run_usage.model_calls;
        self.accepted_semantic_actions += run_usage.accepted_semantic_actions;
        self.tool_calls += run_usage.tool_calls;
        self.tool_retries += run_usage.tool_retries;
        self.knowledge_requests += run_usage.knowledge_requests;
        self.memory_requests += run_usage.memory_requests;
        self.embedding_requests += run_usage.embedding_requests;
        self.duration_ms = add_optional(self.duration_ms, run_usage.duration_ms);
        self.tokens.input_tokens =
            add_optional(self.tokens.input_tokens, run_usage.tokens.input_tokens);
        self.tokens.output_tokens =
            add_optional(self.tokens.output_tokens, run_usage.tokens.output_tokens);
        self.tokens.total_tokens =
            add_optional(self.tokens.total_tokens, run_usage.tokens.total_tokens);
        if self.cost.currency.is_none() {
            self.cost.currency = run_usage.cost.currency.clone();
        }
        if self.cost.currency == run_usage.cost.currency {
            self.cost.amount = add_optional(self.cost.amount, run_usage.cost.amount);
        }
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportPackageIdentity {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhaseReportSummary {
    pub phase_execution_id: String,
    pub phase_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition_to: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionReportSummary {
    pub action_kind: String,
    pub identity: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RuntimeReportSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_dir_source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsumerContextReportSummary {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approximate_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    pub content_included: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScopeReportSummary {
    pub name: String,
    pub value_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointReportSummary {
    pub checkpoint_id: String,
    pub before_phase: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_reject: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationReportSummary {
    pub operation_kind: String,
    pub identity: String,
    pub status: String,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunReport {
    pub report_version: u8,
    pub session_id: String,
    pub run_id: String,
    pub agent: ReportPackageIdentity,
    pub loop_package: ReportPackageIdentity,
    pub started_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    pub terminal_status: HarnessTerminalStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_output: Option<Value>,
    pub preflight_status: PreflightStatus,
    pub diagnostics: Vec<PreflightDiagnostic>,
    pub runtime: RuntimeReportSummary,
    pub runtime_sources: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumer_context: Option<ConsumerContextReportSummary>,
    pub scope_summaries: Vec<ScopeReportSummary>,
    pub phase_summaries: Vec<PhaseReportSummary>,
    pub checkpoint_summaries: Vec<CheckpointReportSummary>,
    pub action_summaries: Vec<ActionReportSummary>,
    pub tool_summaries: Vec<OperationReportSummary>,
    pub mcp_summaries: Vec<OperationReportSummary>,
    pub knowledge_summaries: Vec<OperationReportSummary>,
    pub memory_summaries: Vec<OperationReportSummary>,
    pub usage: RunUsage,
    pub retry_count: u64,
    pub repair_count: u64,
    pub error_count: u64,
    pub approval_summary: BTreeMap<String, u64>,
    pub cancellation_summary: BTreeMap<String, u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_path: Option<String>,
}

impl RunReport {
    pub fn write_pretty(&self, path: &Path, policy: &HarnessTraceContent) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating report directory {}", parent.display()))?;
        }
        let mut value = serde_json::to_value(self).context("serializing run report")?;
        sanitize_value(&mut value, policy, None);
        let bytes = serde_json::to_vec_pretty(&value).context("rendering run report")?;
        fs::write(path, bytes).with_context(|| format!("writing run report {}", path.display()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutputPaths {
    pub run_dir: PathBuf,
    pub events_path: PathBuf,
    pub report_path: PathBuf,
}

impl RunOutputPaths {
    pub fn resolve(
        state_dir: &Path,
        run_id: &str,
        report_path_override: Option<&Path>,
    ) -> Result<Self> {
        let run_dir = state_dir.join("runs").join(run_id);
        let events_path = run_dir.join("events.jsonl");
        let report_path = match report_path_override {
            Some(path) => path.to_path_buf(),
            None => run_dir.join("report.json"),
        };
        if let Some(parent) = events_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating run state directory {}", parent.display()))?;
        }
        if let Some(parent) = report_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating report directory {}", parent.display()))?;
        }
        Ok(Self {
            run_dir,
            events_path,
            report_path,
        })
    }
}

pub struct SyntheticHarnessSession {
    pub session_id: String,
    pub emitter: HarnessEventEmitter,
    pub usage: SessionUsage,
    started_at: Instant,
}

impl SyntheticHarnessSession {
    pub fn new_allocated() -> Self {
        Self::new(allocate_harness_session_id())
    }

    pub fn new(session_id: impl Into<String>) -> Self {
        let session_id = session_id.into();
        Self {
            emitter: HarnessEventEmitter::new(session_id.clone()),
            session_id,
            usage: SessionUsage::default(),
            started_at: Instant::now(),
        }
    }

    pub fn emit_session_starting(&mut self) -> Result<HarnessEventEnvelope> {
        self.emitter.emit(
            HarnessEventType::SessionStarting,
            HarnessEventPayload::Lifecycle {
                message: "Harness session is starting.".into(),
                fields: BTreeMap::new(),
            },
            HarnessEventBuilder::default(),
        )
    }

    pub fn start_allocated_run(&mut self) -> Result<SyntheticHarnessRun> {
        self.start_run(allocate_harness_run_id())
    }

    pub fn start_run(&mut self, run_id: impl Into<String>) -> Result<SyntheticHarnessRun> {
        let run_id = run_id.into();
        self.usage.record_run_started();
        self.emitter.emit(
            HarnessEventType::RunStarted,
            HarnessEventPayload::Lifecycle {
                message: "Harness run started.".into(),
                fields: BTreeMap::new(),
            },
            HarnessEventBuilder {
                run_id: Some(run_id.clone()),
                ..HarnessEventBuilder::default()
            },
        )?;
        Ok(SyntheticHarnessRun {
            run_id,
            started_at: Utc::now(),
            usage: RunUsage::default(),
            phase_summaries: Vec::new(),
            action_summaries: Vec::new(),
        })
    }

    pub fn complete_run(
        &mut self,
        run: SyntheticHarnessRun,
        terminal_status: HarnessTerminalStatus,
    ) -> Result<RunUsage> {
        let mut usage = run.usage;
        usage.duration_ms.get_or_insert(0);
        self.usage.record_run_completed(&usage);
        self.usage.duration_ms = Some(self.started_at.elapsed().as_millis() as u64);
        self.emitter.emit(
            event_type_for_terminal_status(terminal_status),
            HarnessEventPayload::Terminal {
                status: terminal_status,
                output: None,
            },
            HarnessEventBuilder {
                run_id: Some(run.run_id),
                ..HarnessEventBuilder::default()
            },
        )?;
        self.emitter.emit(
            HarnessEventType::SessionUsageUpdated,
            HarnessEventPayload::Usage {
                run_usage: Box::new(usage.clone()),
                session_usage: Box::new(self.usage.clone()),
            },
            HarnessEventBuilder::default(),
        )?;
        Ok(usage)
    }

    pub fn stop(&mut self) -> Result<HarnessEventEnvelope> {
        self.usage.duration_ms = Some(self.started_at.elapsed().as_millis() as u64);
        self.emitter.emit(
            HarnessEventType::SessionStopped,
            HarnessEventPayload::Usage {
                run_usage: Box::new(RunUsage::default()),
                session_usage: Box::new(self.usage.clone()),
            },
            HarnessEventBuilder::default(),
        )
    }
}

#[derive(Debug)]
pub struct SyntheticHarnessRun {
    pub run_id: String,
    pub started_at: DateTime<Utc>,
    pub usage: RunUsage,
    pub phase_summaries: Vec<PhaseReportSummary>,
    pub action_summaries: Vec<ActionReportSummary>,
}

impl SyntheticHarnessRun {
    pub fn report(self, input: SyntheticRunReportInput) -> RunReport {
        RunReport {
            report_version: HARNESS_REPORT_SCHEMA_VERSION,
            session_id: input.session_id,
            run_id: self.run_id,
            agent: input.agent,
            loop_package: input.loop_package,
            started_at: self.started_at,
            ended_at: Some(Utc::now()),
            duration_ms: self.usage.duration_ms,
            terminal_status: input.terminal_status,
            terminal_output: None,
            preflight_status: input.preflight_status,
            diagnostics: input.diagnostics,
            runtime: RuntimeReportSummary::default(),
            runtime_sources: BTreeMap::new(),
            consumer_context: None,
            scope_summaries: Vec::new(),
            phase_summaries: self.phase_summaries,
            checkpoint_summaries: Vec::new(),
            action_summaries: self.action_summaries,
            tool_summaries: Vec::new(),
            mcp_summaries: Vec::new(),
            knowledge_summaries: Vec::new(),
            memory_summaries: Vec::new(),
            usage: self.usage,
            retry_count: 0,
            repair_count: 0,
            error_count: 0,
            approval_summary: BTreeMap::new(),
            cancellation_summary: BTreeMap::new(),
            trace_path: input.trace_path,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SyntheticRunReportInput {
    pub session_id: String,
    pub agent: ReportPackageIdentity,
    pub loop_package: ReportPackageIdentity,
    pub terminal_status: HarnessTerminalStatus,
    pub preflight_status: PreflightStatus,
    pub diagnostics: Vec<PreflightDiagnostic>,
    pub trace_path: Option<String>,
}

fn event_type_for_terminal_status(status: HarnessTerminalStatus) -> HarnessEventType {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("agentpm-harness-observability-{label}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn trace_config(level: HarnessTraceLevel, content: HarnessTraceContent) -> HarnessTraceConfig {
        HarnessTraceConfig {
            enabled: true,
            level,
            content,
        }
    }

    #[test]
    fn allocated_session_and_run_ids_are_unique_and_prefixed() {
        let session_id = allocate_harness_session_id();
        let other_session_id = allocate_harness_session_id();
        let run_id = allocate_harness_run_id();
        let other_run_id = allocate_harness_run_id();

        assert!(session_id.starts_with("sess-"));
        assert!(run_id.starts_with("run-"));
        assert_ne!(session_id, other_session_id);
        assert_ne!(run_id, other_run_id);

        let mut session = SyntheticHarnessSession::new_allocated();
        let run = session.start_allocated_run().unwrap();
        assert!(session.session_id.starts_with("sess-"));
        assert!(run.run_id.starts_with("run-"));
    }

    #[test]
    fn session_and_run_sequences_are_monotonic_and_run_local_sequences_reset() {
        let memory = InMemoryEventSink::default();
        let handle = memory.clone();
        let mut emitter = HarnessEventEmitter::new("session-1");
        emitter.add_sink(Box::new(memory));

        emitter
            .emit(
                HarnessEventType::SessionStarting,
                HarnessEventPayload::Empty,
                HarnessEventBuilder::default(),
            )
            .unwrap();
        emitter
            .emit(
                HarnessEventType::RunStarted,
                HarnessEventPayload::Empty,
                HarnessEventBuilder {
                    run_id: Some("run-1".into()),
                    ..HarnessEventBuilder::default()
                },
            )
            .unwrap();
        emitter
            .emit(
                HarnessEventType::PhaseStarted,
                HarnessEventPayload::Empty,
                HarnessEventBuilder {
                    run_id: Some("run-1".into()),
                    ..HarnessEventBuilder::default()
                },
            )
            .unwrap();
        emitter
            .emit(
                HarnessEventType::RunStarted,
                HarnessEventPayload::Empty,
                HarnessEventBuilder {
                    run_id: Some("run-2".into()),
                    ..HarnessEventBuilder::default()
                },
            )
            .unwrap();

        let events = handle.events();
        assert_eq!(
            events
                .iter()
                .map(|event| event.session_sequence)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        assert_eq!(events[0].run_sequence, None);
        assert_eq!(events[1].run_sequence, Some(1));
        assert_eq!(events[2].run_sequence, Some(2));
        assert_eq!(events[3].run_sequence, Some(1));
    }

    #[test]
    fn event_ids_correlation_and_parent_relationships_are_serialized() {
        let mut emitter = HarnessEventEmitter::new("session-1");
        let parent = emitter
            .emit(
                HarnessEventType::ModelRequestStarted,
                HarnessEventPayload::Lifecycle {
                    message: "starting".into(),
                    fields: BTreeMap::new(),
                },
                HarnessEventBuilder {
                    run_id: Some("run-1".into()),
                    correlation_id: Some("corr-1".into()),
                    ..HarnessEventBuilder::default()
                },
            )
            .unwrap();
        let child = emitter
            .emit(
                HarnessEventType::ModelRequestCompleted,
                HarnessEventPayload::Lifecycle {
                    message: "done".into(),
                    fields: BTreeMap::new(),
                },
                HarnessEventBuilder {
                    run_id: Some("run-1".into()),
                    correlation_id: Some("corr-1".into()),
                    parent_event_id: Some(parent.event_id.clone()),
                    ..HarnessEventBuilder::default()
                },
            )
            .unwrap();

        let value = serde_json::to_value(&child).unwrap();
        assert_eq!(value["schema_version"], HARNESS_EVENT_SCHEMA_VERSION);
        assert_eq!(value["event_type"], "model_request_completed");
        assert_eq!(value["correlation_id"], "corr-1");
        assert_eq!(value["parent_event_id"], parent.event_id);
    }

    #[test]
    fn trace_level_filters_events_without_changing_canonical_emission() {
        assert!(trace_level_includes(
            &HarnessTraceLevel::Minimal,
            HarnessEventType::RunStarted
        ));
        assert!(!trace_level_includes(
            &HarnessTraceLevel::Minimal,
            HarnessEventType::ToolInvoked
        ));
        assert!(trace_level_includes(
            &HarnessTraceLevel::Normal,
            HarnessEventType::ToolInvoked
        ));
        assert!(!trace_level_includes(
            &HarnessTraceLevel::Normal,
            HarnessEventType::PromptPrepared
        ));
        assert!(trace_level_includes(
            &HarnessTraceLevel::Verbose,
            HarnessEventType::PromptPrepared
        ));
    }

    #[test]
    fn content_policy_redacts_content_but_always_removes_secrets() {
        let mut fields = BTreeMap::new();
        fields.insert("prompt".into(), json!("visible prompt"));
        fields.insert("api_token".into(), json!("secret-token"));
        let event = HarnessEventEnvelope {
            schema_version: HARNESS_EVENT_SCHEMA_VERSION,
            event_id: "evt-1".into(),
            session_id: "session-1".into(),
            run_id: Some("run-1".into()),
            session_sequence: 1,
            run_sequence: Some(1),
            timestamp: Utc::now(),
            event_type: HarnessEventType::PromptPrepared,
            phase_execution_id: None,
            correlation_id: None,
            parent_event_id: None,
            payload: HarnessEventPayload::Lifecycle {
                message: "prepared".into(),
                fields,
            },
        };

        let redacted = serde_json::to_value(
            apply_content_policy(&event, &HarnessTraceContent::Redacted).unwrap(),
        )
        .unwrap();
        assert_eq!(redacted["payload"]["fields"]["prompt"], "[redacted]");
        assert_eq!(
            redacted["payload"]["fields"]["api_token"],
            "[secret redacted]"
        );

        let full =
            serde_json::to_value(apply_content_policy(&event, &HarnessTraceContent::Full).unwrap())
                .unwrap();
        assert_eq!(full["payload"]["fields"]["prompt"], "visible prompt");
        assert_eq!(full["payload"]["fields"]["api_token"], "[secret redacted]");

        let none =
            serde_json::to_value(apply_content_policy(&event, &HarnessTraceContent::None).unwrap())
                .unwrap();
        assert!(none["payload"]["fields"].get("prompt").is_none());
        assert_eq!(none["payload"]["fields"]["api_token"], "[secret redacted]");
    }

    #[test]
    fn jsonl_trace_is_incremental_ordered_and_independently_parseable() {
        let dir = temp_dir("jsonl");
        let path = dir.join("events.jsonl");
        let sink = JsonlTraceSink::create(
            &path,
            trace_config(HarnessTraceLevel::Verbose, HarnessTraceContent::Redacted),
        )
        .unwrap();
        let mut emitter = HarnessEventEmitter::new("session-1");
        emitter.add_sink(Box::new(sink));
        emitter
            .emit(
                HarnessEventType::RunStarted,
                HarnessEventPayload::Empty,
                HarnessEventBuilder {
                    run_id: Some("run-1".into()),
                    ..HarnessEventBuilder::default()
                },
            )
            .unwrap();

        let first = fs::read_to_string(&path).unwrap();
        assert_eq!(first.lines().count(), 1);
        let first_event: HarnessEventEnvelope =
            serde_json::from_str(first.lines().next().unwrap()).unwrap();
        assert_eq!(first_event.session_sequence, 1);

        emitter
            .emit(
                HarnessEventType::ToolInvoked,
                HarnessEventPayload::Action {
                    action_kind: "tool".into(),
                    identity: "@zack/search".into(),
                    status: "started".into(),
                    fields: BTreeMap::new(),
                },
                HarnessEventBuilder {
                    run_id: Some("run-1".into()),
                    ..HarnessEventBuilder::default()
                },
            )
            .unwrap();
        let lines = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<HarnessEventEnvelope>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].session_sequence, 2);
    }

    #[test]
    fn run_output_paths_create_state_dir_and_support_report_override() {
        let dir = temp_dir("paths");
        let override_path = dir.join("custom").join("report.json");
        let paths =
            RunOutputPaths::resolve(&dir.join(".agentpm-state"), "run-1", Some(&override_path))
                .unwrap();
        assert!(paths.run_dir.ends_with(".agentpm-state/runs/run-1"));
        assert!(
            paths
                .events_path
                .ends_with(".agentpm-state/runs/run-1/events.jsonl")
        );
        assert_eq!(paths.report_path, override_path);
        assert!(paths.run_dir.exists());
        assert!(paths.report_path.parent().unwrap().exists());
    }

    #[test]
    fn redacted_content_policy_redacts_object_valued_output_fields() {
        let event = HarnessEventEnvelope {
            schema_version: HARNESS_EVENT_SCHEMA_VERSION,
            event_id: "evt-1".into(),
            session_id: "session-1".into(),
            run_id: Some("run-1".into()),
            session_sequence: 1,
            run_sequence: Some(1),
            timestamp: Utc::now(),
            event_type: HarnessEventType::ModelRequestCompleted,
            phase_execution_id: Some("phase-exec-1".into()),
            correlation_id: None,
            parent_event_id: None,
            payload: HarnessEventPayload::Lifecycle {
                message: "Model request completed.".into(),
                fields: BTreeMap::from([(
                    "proposed_actions".into(),
                    json!([
                        {
                            "action_kind": "phase_completion",
                            "fields": {
                                "outcome": "complete",
                                "output": { "message": "final answer" }
                            }
                        }
                    ]),
                )]),
            },
        };

        let redacted = apply_content_policy(&event, &HarnessTraceContent::Redacted).unwrap();
        let value = serde_json::to_value(redacted).unwrap();
        assert_eq!(
            value["payload"]["fields"]["proposed_actions"][0]["fields"]["output"],
            "[redacted]"
        );
        assert!(
            !serde_json::to_string(&value)
                .unwrap()
                .contains("final answer")
        );
    }

    #[test]
    fn content_policy_redacts_embedding_event_payload_content_fields() {
        let event = HarnessEventEnvelope {
            schema_version: HARNESS_EVENT_SCHEMA_VERSION,
            event_id: "evt-1".into(),
            session_id: "session-1".into(),
            run_id: Some("run-1".into()),
            session_sequence: 1,
            run_sequence: Some(1),
            timestamp: Utc::now(),
            event_type: HarnessEventType::EmbeddingRequestCompleted,
            phase_execution_id: Some("phase-exec-1".into()),
            correlation_id: None,
            parent_event_id: None,
            payload: HarnessEventPayload::Action {
                action_kind: "embedding_request".into(),
                identity: "@zack/guide".into(),
                status: "completed".into(),
                fields: BTreeMap::from([
                    ("package".into(), json!("@zack/guide")),
                    ("provider".into(), json!("manual")),
                    ("model".into(), json!("toy-3d")),
                    ("dimensions".into(), json!(3)),
                    ("query".into(), json!("alpha launch checklist")),
                    ("text".into(), json!("alpha launch checklist")),
                    ("vector".into(), json!([1.0, 0.0, 0.0])),
                    ("embedding_vector".into(), json!([1.0, 0.0, 0.0])),
                ]),
            },
        };

        let redacted = serde_json::to_value(
            apply_content_policy(&event, &HarnessTraceContent::Redacted).unwrap(),
        )
        .unwrap();
        assert_eq!(redacted["payload"]["fields"]["package"], "@zack/guide");
        assert_eq!(redacted["payload"]["fields"]["provider"], "manual");
        assert_eq!(redacted["payload"]["fields"]["model"], "toy-3d");
        assert_eq!(redacted["payload"]["fields"]["dimensions"], 3);
        assert_eq!(redacted["payload"]["fields"]["query"], "[redacted]");
        assert_eq!(redacted["payload"]["fields"]["text"], "[redacted]");
        assert_eq!(redacted["payload"]["fields"]["vector"], "[redacted]");
        assert_eq!(
            redacted["payload"]["fields"]["embedding_vector"],
            "[redacted]"
        );

        let none =
            serde_json::to_value(apply_content_policy(&event, &HarnessTraceContent::None).unwrap())
                .unwrap();
        assert!(none["payload"]["fields"].get("query").is_none());
        assert!(none["payload"]["fields"].get("text").is_none());
        assert!(none["payload"]["fields"].get("vector").is_none());
        assert!(none["payload"]["fields"].get("embedding_vector").is_none());
        assert_eq!(none["payload"]["fields"]["provider"], "manual");
    }

    #[test]
    fn content_policy_redacts_active_profile_event_payloads() {
        let event = HarnessEventEnvelope {
            schema_version: HARNESS_EVENT_SCHEMA_VERSION,
            event_id: "evt-1".into(),
            session_id: "session-1".into(),
            run_id: Some("run-1".into()),
            session_sequence: 1,
            run_sequence: Some(1),
            timestamp: Utc::now(),
            event_type: HarnessEventType::EffectivePhaseComputed,
            phase_execution_id: Some("phase-exec-1".into()),
            correlation_id: None,
            parent_event_id: None,
            payload: HarnessEventPayload::Lifecycle {
                message: "Effective phase computed.".into(),
                fields: BTreeMap::from([
                    ("profile_candidates".into(), json!(["@zack/support-style"])),
                    (
                        "active_profiles".into(),
                        json!([
                            {
                                "name": "@zack/support-style",
                                "profile": {
                                    "objectives": ["Do not leak authored profile objectives."],
                                    "constraints": [
                                        {
                                            "id": "private-guidance",
                                            "instruction": "Never expose this instruction text."
                                        }
                                    ]
                                }
                            }
                        ]),
                    ),
                ]),
            },
        };

        let redacted = serde_json::to_value(
            apply_content_policy(&event, &HarnessTraceContent::Redacted).unwrap(),
        )
        .unwrap();
        assert_eq!(
            redacted["payload"]["fields"]["active_profiles"],
            "[redacted]"
        );
        assert!(
            !serde_json::to_string(&redacted)
                .unwrap()
                .contains("Never expose this instruction text")
        );

        let none =
            serde_json::to_value(apply_content_policy(&event, &HarnessTraceContent::None).unwrap())
                .unwrap();
        assert!(none["payload"]["fields"].get("active_profiles").is_none());
        assert_eq!(
            none["payload"]["fields"]["profile_candidates"][0],
            "@zack/support-style"
        );
    }

    #[test]
    fn run_report_serializes_and_redacts_sensitive_output() {
        let dir = temp_dir("report");
        let path = dir.join("report.json");
        let mut run = SyntheticHarnessRun {
            run_id: "run-1".into(),
            started_at: Utc::now(),
            usage: RunUsage::default(),
            phase_summaries: vec![PhaseReportSummary {
                phase_execution_id: "phase-exec-1".into(),
                phase_id: "draft".into(),
                outcome: Some("complete".into()),
                transition_to: Some("$end".into()),
                status: "completed".into(),
            }],
            action_summaries: vec![ActionReportSummary {
                action_kind: "tool".into(),
                identity: "@zack/search".into(),
                status: "completed".into(),
                error: None,
            }],
        };
        run.usage.model_calls = 1;
        let mut report = run.report(SyntheticRunReportInput {
            session_id: "session-1".into(),
            agent: ReportPackageIdentity {
                name: "@zack/agent".into(),
                version: "0.1.0".into(),
            },
            loop_package: ReportPackageIdentity {
                name: "@zack/loop".into(),
                version: "0.1.0".into(),
            },
            terminal_status: HarnessTerminalStatus::Ended,
            preflight_status: PreflightStatus::Ready,
            diagnostics: Vec::new(),
            trace_path: Some("events.jsonl".into()),
        });
        report.terminal_output = Some(json!({
            "text": "final answer",
            "api_key": "raw-secret"
        }));
        report
            .write_pretty(&path, &HarnessTraceContent::Redacted)
            .unwrap();

        let value: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(value["report_version"], HARNESS_REPORT_SCHEMA_VERSION);
        assert_eq!(value["terminal_output"]["text"], "[redacted]");
        assert_eq!(value["terminal_output"]["api_key"], "[secret redacted]");
        assert_eq!(value["phase_summaries"][0]["phase_id"], "draft");
        assert_eq!(value["usage"]["model_calls"], 1);
    }

    #[test]
    fn run_report_contract_includes_runtime_context_and_operation_summaries() {
        let mut report = SyntheticHarnessRun {
            run_id: "run-1".into(),
            started_at: Utc::now(),
            usage: RunUsage::default(),
            phase_summaries: Vec::new(),
            action_summaries: Vec::new(),
        }
        .report(SyntheticRunReportInput {
            session_id: "session-1".into(),
            agent: ReportPackageIdentity {
                name: "@zack/agent".into(),
                version: "0.1.0".into(),
            },
            loop_package: ReportPackageIdentity {
                name: "@zack/loop".into(),
                version: "0.1.0".into(),
            },
            terminal_status: HarnessTerminalStatus::Ended,
            preflight_status: PreflightStatus::Ready,
            diagnostics: Vec::new(),
            trace_path: Some("events.jsonl".into()),
        });
        report.runtime = RuntimeReportSummary {
            provider_id: Some("openai".into()),
            provider_source: Some("config".into()),
            model_id: Some("gpt-4.1-mini".into()),
            model_source: Some("config".into()),
            state_dir: Some(".agentpm-state".into()),
            state_dir_source: Some("default".into()),
        };
        report.consumer_context = Some(ConsumerContextReportSummary {
            status: "loaded".into(),
            path: Some("context.md".into()),
            byte_size: Some(128),
            approximate_tokens: Some(32),
            sha256: Some("sha256:abc".into()),
            content_included: false,
        });
        report.scope_summaries = vec![ScopeReportSummary {
            name: "user".into(),
            value_available: true,
            value: Some(json!("user-123")),
        }];
        report.checkpoint_summaries = vec![CheckpointReportSummary {
            checkpoint_id: "approve".into(),
            before_phase: "review".into(),
            status: "approved".into(),
            on_reject: Some("$handoff".into()),
        }];
        report.tool_summaries = vec![OperationReportSummary {
            operation_kind: "tool".into(),
            identity: "@zack/search".into(),
            status: "completed".into(),
            count: 1,
        }];
        report.memory_summaries = vec![OperationReportSummary {
            operation_kind: "memory_read".into(),
            identity: "@zack/conversation-continuity".into(),
            status: "completed".into(),
            count: 2,
        }];

        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(value["runtime"]["provider_id"], "openai");
        assert_eq!(value["consumer_context"]["path"], "context.md");
        assert_eq!(value["scope_summaries"][0]["name"], "user");
        assert_eq!(value["scope_summaries"][0]["value"], "user-123");
        assert_eq!(value["checkpoint_summaries"][0]["checkpoint_id"], "approve");
        assert_eq!(value["tool_summaries"][0]["identity"], "@zack/search");
        assert_eq!(
            value["memory_summaries"][0]["identity"],
            "@zack/conversation-continuity"
        );
    }

    #[test]
    fn run_report_scope_values_follow_content_policy() {
        let dir = temp_dir("scope-value-policy");
        let report = RunReport {
            report_version: HARNESS_REPORT_SCHEMA_VERSION,
            session_id: "session-1".into(),
            run_id: "run-1".into(),
            agent: ReportPackageIdentity {
                name: "@zack/agent".into(),
                version: "0.1.0".into(),
            },
            loop_package: ReportPackageIdentity {
                name: "@zack/loop".into(),
                version: "0.1.0".into(),
            },
            started_at: Utc::now(),
            ended_at: Some(Utc::now()),
            duration_ms: Some(1),
            terminal_status: HarnessTerminalStatus::Ended,
            terminal_output: None,
            preflight_status: PreflightStatus::Ready,
            diagnostics: Vec::new(),
            runtime: RuntimeReportSummary::default(),
            runtime_sources: BTreeMap::new(),
            consumer_context: None,
            scope_summaries: vec![ScopeReportSummary {
                name: "user".into(),
                value_available: true,
                value: Some(json!("user-123")),
            }],
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
        };

        let full_path = dir.join("full.json");
        report
            .write_pretty(&full_path, &HarnessTraceContent::Full)
            .unwrap();
        let full: Value = serde_json::from_str(&fs::read_to_string(full_path).unwrap()).unwrap();
        assert_eq!(full["scope_summaries"][0]["value"], "user-123");

        let redacted_path = dir.join("redacted.json");
        report
            .write_pretty(&redacted_path, &HarnessTraceContent::Redacted)
            .unwrap();
        let redacted: Value =
            serde_json::from_str(&fs::read_to_string(redacted_path).unwrap()).unwrap();
        assert_eq!(redacted["scope_summaries"][0]["value"], "[redacted]");

        let none_path = dir.join("none.json");
        report
            .write_pretty(&none_path, &HarnessTraceContent::None)
            .unwrap();
        let none: Value = serde_json::from_str(&fs::read_to_string(none_path).unwrap()).unwrap();
        assert!(none["scope_summaries"][0].get("value").is_none());
    }

    #[test]
    fn terminal_statuses_emit_events_and_serialize_valid_reports() {
        let statuses = [
            (
                HarnessTerminalStatus::Ended,
                HarnessEventType::RunCompleted,
                "ended",
            ),
            (
                HarnessTerminalStatus::HandedOff,
                HarnessEventType::RunCompleted,
                "handed_off",
            ),
            (
                HarnessTerminalStatus::Aborted,
                HarnessEventType::RunFailed,
                "aborted",
            ),
            (
                HarnessTerminalStatus::Failed,
                HarnessEventType::RunFailed,
                "failed",
            ),
            (
                HarnessTerminalStatus::Cancelled,
                HarnessEventType::RunCancelled,
                "cancelled",
            ),
            (
                HarnessTerminalStatus::LimitReached,
                HarnessEventType::RunLimitReached,
                "limit_reached",
            ),
            (
                HarnessTerminalStatus::ApprovalRequired,
                HarnessEventType::RunApprovalRequired,
                "approval_required",
            ),
        ];

        for (index, (status, expected_event_type, expected_status)) in
            statuses.into_iter().enumerate()
        {
            assert_eq!(event_type_for_terminal_status(status), expected_event_type);
            let dir = temp_dir(&format!("terminal-{index}"));
            let report_path = dir.join("report.json");
            let report = SyntheticHarnessRun {
                run_id: format!("run-{index}"),
                started_at: Utc::now(),
                usage: RunUsage::default(),
                phase_summaries: Vec::new(),
                action_summaries: Vec::new(),
            }
            .report(SyntheticRunReportInput {
                session_id: "session-1".into(),
                agent: ReportPackageIdentity {
                    name: "@zack/agent".into(),
                    version: "0.1.0".into(),
                },
                loop_package: ReportPackageIdentity {
                    name: "@zack/loop".into(),
                    version: "0.1.0".into(),
                },
                terminal_status: status,
                preflight_status: PreflightStatus::ReadyWithWarnings,
                diagnostics: Vec::new(),
                trace_path: Some("events.jsonl".into()),
            });
            report
                .write_pretty(&report_path, &HarnessTraceContent::Redacted)
                .unwrap();
            let value: Value =
                serde_json::from_str(&fs::read_to_string(report_path).unwrap()).unwrap();
            assert_eq!(value["terminal_status"], expected_status);
        }
    }

    #[test]
    fn session_usage_aggregates_runs_without_fabricating_unknowns() {
        let mut session_usage = SessionUsage::default();
        session_usage.record_run_started();
        let mut first = RunUsage {
            model_calls: 2,
            accepted_semantic_actions: 3,
            tool_calls: 1,
            tool_retries: 1,
            knowledge_requests: 1,
            memory_requests: 2,
            embedding_requests: 1,
            duration_ms: Some(50),
            ..RunUsage::default()
        };
        first.tokens.input_tokens = Some(10);
        first.tokens.output_tokens = Some(5);
        first.tokens.total_tokens = Some(15);
        session_usage.record_run_completed(&first);

        session_usage.record_run_started();
        let second = RunUsage {
            model_calls: 1,
            duration_ms: Some(25),
            ..RunUsage::default()
        };
        session_usage.record_run_completed(&second);

        assert_eq!(session_usage.started_runs, 2);
        assert_eq!(session_usage.completed_runs, 2);
        assert_eq!(session_usage.model_calls, 3);
        assert_eq!(session_usage.duration_ms, Some(75));
        assert_eq!(session_usage.tokens.input_tokens, Some(10));
        assert_eq!(session_usage.tokens.output_tokens, Some(5));
        assert_eq!(session_usage.tokens.total_tokens, Some(15));
        assert_eq!(session_usage.cost.amount, None);
    }

    #[test]
    fn session_stop_emits_terminal_usage_summary() {
        let memory = InMemoryEventSink::default();
        let handle = memory.clone();
        let mut session = SyntheticHarnessSession::new("session-1");
        session.emitter.add_sink(Box::new(memory));

        let mut run = session.start_run("run-1").unwrap();
        run.usage.model_calls = 1;
        session
            .complete_run(run, HarnessTerminalStatus::Ended)
            .unwrap();
        let stopped = session.stop().unwrap();

        assert_eq!(stopped.event_type, HarnessEventType::SessionStopped);
        assert_eq!(stopped.run_id, None);
        let events = handle.events();
        assert_eq!(
            events.last().map(|event| event.event_type),
            Some(HarnessEventType::SessionStopped)
        );
        match &stopped.payload {
            HarnessEventPayload::Usage { session_usage, .. } => {
                assert_eq!(session_usage.completed_runs, 1);
                assert_eq!(session_usage.model_calls, 1);
            }
            other => panic!("expected usage payload, got {other:?}"),
        }
    }

    #[test]
    fn synthetic_lifecycle_flushes_failure_safe_trace_and_report() {
        let dir = temp_dir("lifecycle");
        let paths = RunOutputPaths::resolve(&dir.join(".agentpm-state"), "run-1", None).unwrap();
        let trace = JsonlTraceSink::create(
            &paths.events_path,
            trace_config(HarnessTraceLevel::Minimal, HarnessTraceContent::Redacted),
        )
        .unwrap();
        let mut session = SyntheticHarnessSession::new("session-1");
        session.emitter.add_sink(Box::new(trace));
        session.emit_session_starting().unwrap();
        let mut run = session.start_run("run-1").unwrap();
        run.usage.model_calls = 1;
        run.usage.tokens.total_tokens = None;
        let report = run.report(SyntheticRunReportInput {
            session_id: session.session_id.clone(),
            agent: ReportPackageIdentity {
                name: "@zack/agent".into(),
                version: "0.1.0".into(),
            },
            loop_package: ReportPackageIdentity {
                name: "@zack/loop".into(),
                version: "0.1.0".into(),
            },
            terminal_status: HarnessTerminalStatus::Failed,
            preflight_status: PreflightStatus::ReadyWithWarnings,
            diagnostics: Vec::new(),
            trace_path: Some("events.jsonl".into()),
        });
        report
            .write_pretty(&paths.report_path, &HarnessTraceContent::Redacted)
            .unwrap();
        session.emitter.flush().unwrap();

        assert!(paths.events_path.exists());
        assert!(paths.report_path.exists());
        let report_value: Value =
            serde_json::from_str(&fs::read_to_string(paths.report_path).unwrap()).unwrap();
        assert_eq!(report_value["terminal_status"], "failed");
        assert_eq!(report_value["usage"]["tokens"]["total_tokens"], Value::Null);
    }
}
