use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolveRequest {
    pub items: Vec<ResolveReqItem>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ResolveReqItem {
    pub name: String,
    pub range: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolveResponse {
    pub items: Vec<ResolveRespItem>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ResolveRespItem {
    pub name: String,
    pub version: String,
    /// 64-char lowercase hex SHA-256
    pub integrity: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstallInitResponse {
    pub session_id: String,
    /// When the presigned URLs expire (RFC3339 string for simplicity)
    pub expires_at: String,
    pub artifacts: Vec<InstallArtifact>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstallArtifact {
    /// "@owner/name"
    pub name: String,
    /// Concrete resolved version (e.g., "1.3.4")
    pub version: String,
    /// 64-char lowercase hex SHA-256
    pub integrity: String,
    /// Short-lived GET URL for the tarball
    pub presigned_url: String,
    /// Optional: size in bytes for progress bars
    pub size: Option<u64>,
    /// Optional: e.g., "application/gzip"
    pub content_type: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signing: Option<SigningSummary>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<Runtime>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SigningSummary {
    /// "off" | "optional" | "required"
    pub mode: String,
    pub min_author_signatures: u32,
    pub author_signatures_present: u32,
    pub registry_attested: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Runtime {
    pub r#type: String,
    pub version: String,
}
