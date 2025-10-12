use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct PublishReceipt {
    pub id: String,
    pub name: String,
    pub version: String,
    pub url: String,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct InitPublish {
    pub upload_id: String,
    pub put_url: String,
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub tmp_key: String,
    pub final_key: String,
    pub expires_in: u32,
    #[serde(default)]
    pub expires_at: String,
    #[serde(default)]
    pub max_bytes: Option<u64>,
}

/// Basic identity returned by `/whoami`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub kind: String,
    pub sub: String,
    pub email: String,
    pub scopes: Option<Vec<String>>,
    pub pat_id: Option<String>,
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
    pub ts: String,    // ISO-8601 string for now
    pub level: String, // e.g., "info", "error"
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

/// POST /cli/device/start
#[derive(serde::Deserialize)]
pub struct DeviceStartRes {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
    pub expires_in: u64,
}

/// POST /cli/device/poll
#[derive(serde::Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DevicePollRes {
    AuthorizationPending,
    SlowDown, // optional; server may send
    Denied,   // map access_denied to this
    Expired,  // map expired_token to this
    Success {
        pat: String,
        token_id: String,
        scopes: Vec<String>,
        created_at: String,
    },
}
