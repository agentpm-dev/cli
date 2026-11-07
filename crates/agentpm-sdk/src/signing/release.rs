//! New release statements use JCS. Legacy author v1 keeps its original serializer.
use crate::{canonical, models::install::PackageKind, python::is_sha256, release::ReleaseManifest};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

pub const AUTHOR_SIGNATURE_V2: &str = "agentpm.package.signature.v2";
pub const REGISTRY_ATTESTATION_V3: &str = "agentpm.registry.attestation.v3";

fn validate_binding(name: &str, version: &str, digest: &str) -> Result<()> {
    let parts: Vec<_> = name.strip_prefix('@').unwrap_or("").split('/').collect();
    ensure!(
        parts.len() == 2 && parts.iter().all(|p| crate::release::valid_component(p)),
        "invalid qualified statement name"
    );
    semver::Version::parse(version)?;
    ensure!(is_sha256(digest), "invalid release digest");
    Ok(())
}

fn validate_timestamp(value: &str) -> Result<()> {
    ensure!(
        canonical::normalize_timestamp(value)? == value,
        "new statement timestamps must use normalized UTC RFC3339 Z form"
    );
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AuthorSignatureV2 {
    #[serde(rename = "type")]
    pub statement_type: String,
    pub kind: PackageKind,
    pub name: String,
    pub version: String,
    pub release_digest: String,
    pub created_at: String,
}

impl AuthorSignatureV2 {
    pub fn new(release: &ReleaseManifest, created_at: &str) -> Result<Self> {
        Ok(Self {
            statement_type: AUTHOR_SIGNATURE_V2.into(),
            kind: release.kind,
            name: release.name.clone(),
            version: release.version.clone(),
            release_digest: release.digest()?,
            created_at: canonical::normalize_timestamp(created_at)?,
        })
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        ensure!(
            self.statement_type == AUTHOR_SIGNATURE_V2,
            "unsupported author signature type"
        );
        validate_binding(&self.name, &self.version, &self.release_digest)?;
        validate_timestamp(&self.created_at)?;
        canonical::contract_bytes(&serde_json::to_value(self)?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RegistryAttestationV3 {
    #[serde(rename = "type")]
    pub statement_type: String,
    pub kind: PackageKind,
    pub name: String,
    pub version: String,
    pub release_digest: String,
    pub stored_at: String,
    pub published_by_account_id: String,
    pub registry_key_id: String,
}

impl RegistryAttestationV3 {
    pub fn new(
        release: &ReleaseManifest,
        stored_at: &str,
        publisher: &str,
        key_id: &str,
    ) -> Result<Self> {
        let value = Self {
            statement_type: REGISTRY_ATTESTATION_V3.into(),
            kind: release.kind,
            name: release.name.clone(),
            version: release.version.clone(),
            release_digest: release.digest()?,
            stored_at: canonical::normalize_timestamp(stored_at)?,
            published_by_account_id: publisher.into(),
            registry_key_id: key_id.into(),
        };
        value.canonical_bytes()?;
        Ok(value)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        ensure!(
            self.statement_type == REGISTRY_ATTESTATION_V3,
            "unsupported registry attestation type"
        );
        validate_binding(&self.name, &self.version, &self.release_digest)?;
        validate_timestamp(&self.stored_at)?;
        ensure!(
            !self.published_by_account_id.is_empty() && !self.registry_key_id.is_empty(),
            "missing registry provenance identity"
        );
        canonical::contract_bytes(&serde_json::to_value(self)?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseAuthorSignature {
    pub algo: String,
    pub public_key_b64: String,
    pub signature_b64: String,
    pub statement_json: AuthorSignatureV2,
}
