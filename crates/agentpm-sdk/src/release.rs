//! Release data contract only; upload transactions and target selection are separate.
use crate::{
    artifact::ArtifactFormat, models::install::PackageKind, python::is_sha256, target::Target,
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub const RELEASE_TYPE: &str = "agentpm.package.release.v1";
pub const ARTIFACT_METADATA_FILE: &str = "agentpm-artifact.json";
const MAX_JCS_INTEGER: u64 = crate::canonical::MAX_SAFE_INTEGER;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReleaseArtifact {
    pub target: Target,
    pub digest: String,
    pub size: u64,
    pub content_type: String,
    pub format: ArtifactFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    rename_all = "camelCase",
    try_from = "ReleaseWire"
)]
pub struct ReleaseManifest {
    #[serde(rename = "type")]
    pub release_type: String,
    pub kind: PackageKind,
    pub name: String,
    pub version: String,
    pub manifest_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub python_resolution_digest: Option<String>,
    pub artifacts: Vec<ReleaseArtifact>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ReleaseWire {
    #[serde(rename = "type")]
    release_type: String,
    kind: PackageKind,
    name: String,
    version: String,
    manifest_digest: String,
    #[serde(default, deserialize_with = "optional_digest")]
    python_resolution_digest: Option<String>,
    artifacts: Vec<ReleaseArtifact>,
}

fn optional_digest<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}

impl TryFrom<ReleaseWire> for ReleaseManifest {
    type Error = anyhow::Error;

    fn try_from(wire: ReleaseWire) -> Result<Self> {
        let value = Self {
            release_type: wire.release_type,
            kind: wire.kind,
            name: wire.name,
            version: wire.version,
            manifest_digest: wire.manifest_digest,
            python_resolution_digest: wire.python_resolution_digest,
            artifacts: wire.artifacts,
        };
        value.validate()?;
        Ok(value)
    }
}

impl ReleaseManifest {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.release_type == RELEASE_TYPE,
            "unsupported release format"
        );
        let parts: Vec<_> = self
            .name
            .strip_prefix('@')
            .unwrap_or("")
            .split('/')
            .collect();
        ensure!(
            parts.len() == 2 && parts.iter().all(|p| valid_component(p)),
            "invalid qualified release name"
        );
        semver::Version::parse(&self.version)?;
        ensure!(is_sha256(&self.manifest_digest), "invalid manifest digest");
        if let Some(digest) = &self.python_resolution_digest {
            ensure!(is_sha256(digest), "invalid Python resolution digest");
        }
        ensure!(
            !self.artifacts.is_empty(),
            "release artifact inventory is empty"
        );
        let mut seen = HashSet::new();
        for artifact in &self.artifacts {
            ensure!(
                seen.insert(artifact.target.as_str()),
                "duplicate release target"
            );
            ensure!(is_sha256(&artifact.digest), "invalid artifact digest");
            ensure!(
                (1..=MAX_JCS_INTEGER).contains(&artifact.size),
                "invalid artifact size"
            );
            ensure!(
                artifact.content_type == "application/gzip",
                "unsupported artifact content type"
            );
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let mut value = self.clone();
        value.artifacts.sort_by_key(|a| a.target.as_str());
        crate::canonical::contract_bytes(&serde_json::to_value(value)?)
    }

    pub fn digest(&self) -> Result<String> {
        Ok(format!("sha256:{}", self.digest_hex()?))
    }

    /// Portable agent.lock integrity uses bare lowercase hex, never a selected artifact digest.
    pub fn digest_hex(&self) -> Result<String> {
        Ok(format!("{:x}", Sha256::digest(self.canonical_bytes()?)))
    }
}

fn valid_component(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value.as_bytes()[0].is_ascii_lowercase()
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Shared descriptor embedded only in new-format artifacts. No artifact digest:
/// including an archive's own digest inside it would create a self-reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactMetadata {
    pub target: Target,
    pub format: ArtifactFormat,
}

pub fn manifest_digest(manifest: &Value) -> Result<String> {
    ensure!(manifest.is_object(), "manifest must be an object");
    Ok(format!(
        "sha256:{:x}",
        Sha256::digest(crate::canonical::canonical_bytes(manifest)?)
    ))
}

#[cfg(test)]
mod tests;
