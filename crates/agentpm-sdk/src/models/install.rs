use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PackageKind {
    #[default]
    Tool,
    Agent,
    Skill,
    Knowledge,
    Memory,
    Profile,
    Loop,
    Template,
}

impl PackageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tool => "tool",
            Self::Agent => "agent",
            Self::Skill => "skill",
            Self::Knowledge => "knowledge",
            Self::Memory => "memory",
            Self::Profile => "profile",
            Self::Loop => "loop",
            Self::Template => "template",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageRequirement {
    #[serde(default)]
    pub kind: PackageKind,
    pub name: String,
    pub range: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedPackage {
    #[serde(default)]
    pub kind: PackageKind,
    pub name: String,
    pub version: String,
    /// 64-char lowercase hex SHA-256
    pub integrity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub python_resolution: Option<crate::python::PythonResolution>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveRequest {
    pub items: Vec<PackageRequirement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveResponse {
    pub items: Vec<ResolvedPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallInitResponse {
    pub session_id: String,
    /// When the presigned URLs expire (RFC3339 string for simplicity)
    pub expires_at: String,
    pub artifacts: Vec<PackageArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub verification: Option<crate::provenance::Provenance>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<Runtime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub python_resolution: Option<crate::python::PythonResolution>,
    /// Machine-selected payload; integrity above remains the logical release digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_artifact: Option<crate::release::ReleaseArtifact>,
}

impl PackageArtifact {
    pub fn payload_integrity(&self) -> &str {
        self.selected_artifact
            .as_ref()
            .map_or(self.integrity.as_str(), |a| {
                a.digest.strip_prefix("sha256:").unwrap_or("")
            })
    }

    /// Call only after authenticating the complete release through provenance.
    pub fn verify_selection(&self, target: crate::target::Target) -> anyhow::Result<()> {
        use anyhow::ensure;
        let release = self.verification.as_ref().and_then(|p| p.release.as_ref());
        let Some(release) = release else {
            ensure!(
                self.selected_artifact.is_none(),
                "selected payload has no verified release"
            );
            return Ok(());
        };
        let selected = self
            .selected_artifact
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("new-format release has no selected artifact"))?;
        let expected = release
            .artifacts
            .iter()
            .find(|a| a.target == target)
            .or_else(|| {
                release
                    .artifacts
                    .iter()
                    .find(|a| a.target == crate::target::Target::Any)
            })
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "no compatible artifact for {}@{} on {target}",
                    self.name,
                    self.version
                )
            })?;
        ensure!(
            selected == expected,
            "selected artifact differs from verified release or exact-target preference"
        );
        ensure!(
            self.size == Some(selected.size)
                && self.content_type.as_deref() == Some(selected.content_type.as_str()),
            "selected artifact size/content type differs from verified inventory"
        );
        Ok(())
    }
}

pub type ResolveReqItem = PackageRequirement;
pub type ResolveRespItem = ResolvedPackage;
pub type InstallArtifact = PackageArtifact;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_payload_is_exact_first_and_bound_to_complete_inventory() {
        use crate::target::Target;
        let value: serde_json::Value = serde_json::from_str(include_str!(
            "../../contracts/registry-provenance-v1.fixtures.json"
        ))
        .unwrap();
        let mut artifact: PackageArtifact =
            serde_json::from_value(value["vectors"][1]["artifact"].clone()).unwrap();
        let release = artifact
            .verification
            .as_ref()
            .unwrap()
            .release
            .as_ref()
            .unwrap();
        let selected = release
            .artifacts
            .iter()
            .find(|a| a.target == Target::current().unwrap())
            .unwrap()
            .clone();
        artifact.selected_artifact = Some(selected.clone());
        artifact.size = Some(selected.size);
        artifact.content_type = Some(selected.content_type.clone());
        artifact
            .verify_selection(Target::current().unwrap())
            .unwrap();
        assert_ne!(artifact.payload_integrity(), artifact.integrity);
        let mut changed = artifact.clone();
        changed.selected_artifact.as_mut().unwrap().digest = format!("sha256:{}", "f".repeat(64));
        assert!(
            changed
                .verify_selection(Target::current().unwrap())
                .is_err()
        );
        let mut changed = artifact.clone();
        changed.size = Some(selected.size + 1);
        assert!(
            changed
                .verify_selection(Target::current().unwrap())
                .is_err()
        );
        let mut changed = artifact.clone();
        changed.selected_artifact = None;
        assert!(
            changed
                .verify_selection(Target::current().unwrap())
                .is_err()
        );
        let mut portable: PackageArtifact =
            serde_json::from_value(value["vectors"][0]["artifact"].clone()).unwrap();
        let any = portable
            .verification
            .as_ref()
            .unwrap()
            .release
            .as_ref()
            .unwrap()
            .artifacts[0]
            .clone();
        portable.selected_artifact = Some(any.clone());
        portable.size = Some(any.size);
        portable.content_type = Some(any.content_type.clone());
        for target in Target::SUPPORTED {
            portable.verify_selection(*target).unwrap();
        }
        // Even a legitimately listed portable artifact cannot override an exact match.
        artifact
            .verification
            .as_mut()
            .unwrap()
            .release
            .as_mut()
            .unwrap()
            .artifacts
            .push(any.clone());
        artifact.selected_artifact = Some(any);
        assert!(
            artifact
                .verify_selection(Target::current().unwrap())
                .is_err()
        );
    }

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

    #[test]
    fn install_dto_supports_explicit_skill_kind() {
        let parsed: ResolvedPackage = serde_json::from_str(
            r#"{"kind":"skill","name":"@zack/triage-skill","version":"0.1.0","integrity":"abc"}"#,
        )
        .unwrap();

        assert_eq!(parsed.kind, PackageKind::Skill);
        assert_eq!(parsed.name, "@zack/triage-skill");
    }

    #[test]
    fn install_dto_supports_explicit_knowledge_kind() {
        let parsed: ResolvedPackage = serde_json::from_str(
            r#"{"kind":"knowledge","name":"@zack/python-docs","version":"0.1.0","integrity":"abc"}"#,
        )
        .unwrap();

        assert_eq!(parsed.kind, PackageKind::Knowledge);
        assert_eq!(parsed.name, "@zack/python-docs");
    }

    #[test]
    fn install_dto_supports_explicit_profile_kind() {
        let parsed: ResolvedPackage = serde_json::from_str(
            r#"{"kind":"profile","name":"@zack/support-persona","version":"0.1.0","integrity":"abc"}"#,
        )
        .unwrap();

        assert_eq!(parsed.kind, PackageKind::Profile);
        assert_eq!(parsed.name, "@zack/support-persona");
    }

    #[test]
    fn install_dto_supports_explicit_loop_kind() {
        let parsed: ResolvedPackage = serde_json::from_str(
            r#"{"kind":"loop","name":"@zack/incident-response-loop","version":"0.1.0","integrity":"abc"}"#,
        )
        .unwrap();

        assert_eq!(parsed.kind, PackageKind::Loop);
        assert_eq!(parsed.name, "@zack/incident-response-loop");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningSummary {
    /// "off" | "optional" | "required"
    pub mode: String,
    pub min_author_signatures: u32,
    pub author_signatures_present: u32,
    pub registry_attested: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Runtime {
    pub r#type: String,
    pub version: String,
}
