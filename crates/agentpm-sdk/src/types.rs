use serde::{Deserialize, Serialize};

/// Basic identity returned by `/whoami`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
}

/// A tool registered in AgentPM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    /// Optional JSON schema-ish shapes for inputs/outputs; refine later
    pub inputs: Option<serde_json::Value>,
    pub outputs: Option<serde_json::Value>,
}

/// Status of a tool run
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Canceled,
}

/// A run/execution of a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRun {
    pub id: String,
    pub tool_id: String,
    pub status: RunStatus,
    /// Timestamps as strings for now (avoid extra deps); swap to `time` later
    pub created_at: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub input: Option<serde_json::Value>,
    pub output: Option<serde_json::Value>,
    pub error_message: Option<String>,
}

/// Stream/log event from a run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub ts: String,                  // ISO-8601 string for now
    pub level: String,               // e.g., "info", "error"
    pub message: String,
    pub fields: Option<serde_json::Value>,
}

/// Simple pagination envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_page_token: Option<String>,
    pub total: Option<u64>,
}
