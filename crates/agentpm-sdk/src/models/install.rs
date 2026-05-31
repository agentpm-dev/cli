use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PackageKind {
    #[default]
    Tool,
    Agent,
    Template,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageRequirement {
    #[serde(default)]
    pub kind: PackageKind,
    pub name: String,
    pub range: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolvedPackage {
    #[serde(default)]
    pub kind: PackageKind,
    pub name: String,
    pub version: String,
    /// 64-char lowercase hex SHA-256
    pub integrity: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolveRequest {
    pub items: Vec<PackageRequirement>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolveResponse {
    pub items: Vec<ResolvedPackage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InstallInitResponse {
    pub session_id: String,
    /// When the presigned URLs expire (RFC3339 string for simplicity)
    pub expires_at: String,
    pub artifacts: Vec<PackageArtifact>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageArtifact {
    #[serde(default)]
    pub kind: PackageKind,
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

pub type ResolveReqItem = PackageRequirement;
pub type ResolveRespItem = ResolvedPackage;
pub type InstallArtifact = PackageArtifact;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_dto_defaults_missing_kind_to_tool() {
        let parsed: ResolvedPackage = serde_json::from_str(
            r#"{"name":"@zack/summarize","version":"1.2.3","integrity":"abc"}"#,
        )
        .unwrap();

        assert_eq!(parsed.kind, PackageKind::Tool);
        assert_eq!(parsed.name, "@zack/summarize");
    }

    #[test]
    fn install_dto_supports_explicit_agent_kind() {
        let parsed: PackageRequirement =
            serde_json::from_str(r#"{"kind":"agent","name":"@zack/support-agent","range":"^0.1"}"#)
                .unwrap();

        assert_eq!(parsed.kind, PackageKind::Agent);
        assert_eq!(parsed.range, "^0.1");
    }

    #[test]
    fn install_dto_supports_explicit_template_kind() {
        let parsed: ResolvedPackage = serde_json::from_str(
            r#"{"kind":"template","name":"@zack/research-template","version":"0.1.0","integrity":"abc"}"#,
        )
        .unwrap();

        assert_eq!(parsed.kind, PackageKind::Template);
        assert_eq!(parsed.version, "0.1.0");
    }
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
