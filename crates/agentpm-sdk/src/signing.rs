//! Historical author statement. Its serializer is intentionally NOT JCS.
use serde::{Deserialize, Serialize};

pub const AUTHOR_SIGNATURE_V1: &str = "agentpm.package.signature.v1";

/// Field order is the historical sorted-key JSON order. Do not change it:
/// `serde_json::to_vec` must match the old inline `json!` construction byte-for-byte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorSignatureV1 {
    pub artifact_digest: String,
    pub created_at: String,
    pub kind: String,
    pub name: String,
    #[serde(rename = "type")]
    pub statement_type: String,
    pub version: String,
}

impl AuthorSignatureV1 {
    pub fn new(
        kind: String,
        name: String,
        version: String,
        artifact_digest: String,
        created_at: String,
    ) -> Self {
        Self {
            artifact_digest,
            created_at,
            kind,
            name,
            statement_type: AUTHOR_SIGNATURE_V1.into(),
            version,
        }
    }

    pub fn legacy_bytes(&self) -> serde_json::Result<Vec<u8>> {
        serde_json::to_vec(self)
    }
}

#[cfg(test)]
mod tests;
