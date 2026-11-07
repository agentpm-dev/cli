//! Consumer verification. Display IDs and API booleans are never verification inputs.
use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use chrono::DateTime;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

use crate::{
    canonical,
    models::install::PackageArtifact,
    release::ReleaseManifest,
    signing::{AuthorSignatureV2, RegistryAttestationV3},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TrustRoot {
    pub registry: String,
    pub root_key_id: String,
    pub public_key_b64: String,
}

pub fn pinned_roots() -> Result<Vec<TrustRoot>> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Roots {
        version: u32,
        roots: Vec<TrustRoot>,
    }
    let value: Roots =
        serde_json::from_str(include_str!("../contracts/registry-trust-roots-v1.json"))?;
    ensure!(
        value.version == 1,
        "unsupported Registry trust-root version"
    );
    Ok(value.roots)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RegistryKey {
    pub key_id: String,
    pub algo: String,
    pub public_key_b64: String,
    pub created_at: String,
    pub activated_at: String,
    #[serde(
        default,
        deserialize_with = "non_null_retirement",
        skip_serializing_if = "Option::is_none"
    )]
    pub retired_at: Option<String>,
}

fn non_null_retirement<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct KeySetStatement {
    #[serde(rename = "type")]
    pub statement_type: String,
    pub registry: String,
    pub issued_at: String,
    pub expires_at: String,
    pub keys: Vec<RegistryKey>,
}

impl KeySetStatement {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        let mut value = self.clone();
        value.keys.sort_by(|a, b| a.key_id.cmp(&b.key_id));
        canonical::contract_bytes(&serde_json::to_value(value)?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SignedKeySet {
    pub statement: KeySetStatement,
    pub root_key_id: String,
    pub algo: String,
    pub signature_b64: String,
}

/// Can only be obtained after root verification. Revalidate serialized caches
/// through SignedKeySet::verify; never deserialize directly into trusted state.
#[derive(Debug, Clone)]
pub struct VerifiedKeySet {
    statement: KeySetStatement,
}

fn timestamp(value: &str) -> Result<i64> {
    ensure!(
        canonical::normalize_timestamp(value)? == value,
        "noncanonical signing timestamp"
    );
    Ok(DateTime::parse_from_rfc3339(value)?.timestamp())
}

fn decode<const N: usize>(value: &str) -> Result<[u8; N]> {
    let bytes = B64.decode(value)?;
    ensure!(
        B64.encode(&bytes) == value,
        "noncanonical signature/key encoding"
    );
    bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid Ed25519 key/signature length"))
}

fn verify(public: &str, signature: &str, message: &[u8]) -> Result<()> {
    VerifyingKey::from_bytes(&decode::<32>(public)?)?
        .verify_strict(message, &Signature::from_bytes(&decode::<64>(signature)?))?;
    Ok(())
}

impl SignedKeySet {
    pub fn verify(&self, roots: &[TrustRoot], origin: &str, now: i64) -> Result<VerifiedKeySet> {
        let value = &self.statement;
        ensure!(
            self.algo == "ed25519" && value.statement_type == "agentpm.registry.keyset.v1",
            "unsupported Registry key-set type/algorithm"
        );
        ensure!(
            value.registry == origin,
            "Registry key set belongs to another origin"
        );
        let root = roots.iter().find(|r| r.registry == origin && r.root_key_id == self.root_key_id)
            .ok_or_else(|| anyhow::anyhow!("untrusted Registry root; complete root bootstrap and install a CLI with the audited public root pin"))?;
        verify(
            &root.public_key_b64,
            &self.signature_b64,
            &value.canonical_bytes()?,
        )?;
        let issued = timestamp(&value.issued_at)?;
        let expires = timestamp(&value.expires_at)?;
        ensure!(
            issued <= now + 300
                && issued < expires
                && expires <= issued + 90 * 86400
                && now < expires,
            "Registry key set expired or has invalid validity dates"
        );
        ensure!(
            !value.keys.is_empty() && value.keys.len() <= 128,
            "invalid Registry key inventory size"
        );
        let mut seen = BTreeSet::new();
        for key in &value.keys {
            ensure!(
                !key.key_id.is_empty() && key.key_id.len() <= 128 && seen.insert(&key.key_id),
                "duplicate/empty Registry signing key ID"
            );
            ensure!(
                key.algo == "ed25519",
                "unsupported Registry signing key algorithm"
            );
            VerifyingKey::from_bytes(&decode::<32>(&key.public_key_b64)?)?;
            timestamp(&key.created_at)?;
            let activated = timestamp(&key.activated_at)?;
            if let Some(retired) = &key.retired_at {
                ensure!(
                    timestamp(retired)? >= activated,
                    "invalid Registry retirement timestamp"
                );
            }
        }
        Ok(VerifiedKeySet {
            statement: value.clone(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorProof {
    pub algo: String,
    pub public_key_b64: String,
    pub signature_b64: String,
    pub statement_json: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statement_bytes_b64: Option<String>,
    pub signer_id: String,
    pub is_active: bool,
    #[serde(default)]
    pub revoked_ts: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryProof {
    pub algo: String,
    pub signature_b64: String,
    pub statement_json: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statement_bytes_b64: Option<String>,
    pub registry_key_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    pub mode: String,
    pub min_author_signatures: u32,
    pub manifest: Value,
    pub authors: Vec<AuthorProof>,
    pub registry_attestation: Option<RegistryProof>,
    pub registry_keyset_path: String,
    #[serde(default)]
    pub release: Option<ReleaseManifest>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct VerificationPolicy {
    pub require_signature: bool,
    pub require_attestation: bool,
}

/// Historical Python json.dumps(sort_keys=True, ensure_ascii=True), including
/// all original statement keys. Never normalize old timestamps or apply JCS.
pub fn legacy_statement_bytes(value: &Value) -> Result<Vec<u8>> {
    let text = serde_json::to_string(value)?;
    let mut result = String::new();
    for ch in text.chars() {
        if ch.is_ascii() {
            result.push(ch);
        } else {
            for unit in ch.encode_utf16(&mut [0; 2]) {
                result.push_str(&format!("\\u{unit:04x}"));
            }
        }
    }
    Ok(result.into_bytes())
}

fn historical_bytes(value: &Value, supplied: Option<&str>) -> Result<Vec<u8>> {
    if let Some(encoded) = supplied {
        let bytes = B64.decode(encoded)?;
        ensure!(
            B64.encode(&bytes) == encoded,
            "noncanonical legacy statement encoding"
        );
        ensure!(
            serde_json::from_slice::<Value>(&bytes)? == *value,
            "legacy signed bytes differ from the bound statement"
        );
        return Ok(bytes);
    }
    legacy_statement_bytes(value)
}

fn legacy_binding(statement: &Value, artifact: &PackageArtifact, registry: bool) -> Result<()> {
    let (namespace, name) = artifact
        .name
        .trim_start_matches('@')
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("invalid qualified package name"))?;
    ensure!(
        statement["name"] == name && statement["version"] == artifact.version,
        "legacy signature package identity mismatch"
    );
    ensure!(
        statement
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("tool")
            == artifact.kind.as_str(),
        "legacy signature package kind mismatch"
    );
    ensure!(
        statement["artifactDigest"] == format!("sha256:{}", artifact.integrity),
        "legacy signature artifact digest mismatch"
    );
    if registry {
        ensure!(
            statement["type"] == "agentpm.registry.attestation.v2"
                && statement["namespace"] == namespace,
            "legacy Registry statement type/namespace mismatch"
        );
    } else {
        ensure!(
            statement["type"] == "agentpm.package.signature.v1",
            "unsupported legacy author statement"
        );
    }
    Ok(())
}

pub fn needs_registry_keys(artifact: &PackageArtifact, policy: VerificationPolicy) -> bool {
    policy.require_attestation
        || artifact
            .verification
            .as_ref()
            .is_some_and(|p| p.release.is_some())
}

pub fn verify_artifact(
    artifact: &PackageArtifact,
    keys: Option<&VerifiedKeySet>,
    policy: VerificationPolicy,
) -> Result<()> {
    let Some(proof) = &artifact.verification else {
        let required = artifact
            .signing
            .as_ref()
            .is_some_and(|s| s.mode == "required");
        ensure!(
            !policy.require_signature && !policy.require_attestation && !required,
            "Registry did not provide cryptographic provenance; upgrade Registry support before requiring signatures/attestations"
        );
        return Ok(()); // Backward-compatible unsigned legacy responses only.
    };
    ensure!(
        ["off", "optional", "required"].contains(&proof.mode.as_str())
            && proof.min_author_signatures == 1,
        "unsupported namespace signing policy"
    );
    if let Some(summary) = &artifact.signing {
        ensure!(
            summary.mode == proof.mode,
            "inconsistent namespace signing policy"
        );
    }
    ensure!(
        proof.registry_keyset_path == "/v1/registry/signing-keys",
        "unexpected Registry key lookup path"
    );
    ensure!(
        proof.manifest["name"] == artifact.name.split('/').next_back().unwrap_or("")
            && proof.manifest["version"] == artifact.version
            && proof
                .manifest
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("tool")
                == artifact.kind.as_str(),
        "install manifest identity mismatch"
    );
    if let Some(release) = &proof.release {
        ensure!(
            release.name == artifact.name
                && release.version == artifact.version
                && release.kind == artifact.kind
                && release.digest_hex()? == artifact.integrity,
            "release identity/digest mismatch"
        );
        ensure!(
            release.manifest_digest == crate::release::manifest_digest(&proof.manifest)?,
            "release manifest digest mismatch"
        );
        let resolution = artifact
            .python_resolution
            .as_ref()
            .map(|r| r.digest().map(|digest| format!("sha256:{digest}")))
            .transpose()?;
        ensure!(
            release.python_resolution_digest == resolution,
            "release Python resolution digest mismatch"
        );
    }
    let mut seen = BTreeSet::new();
    for author in &proof.authors {
        ensure!(
            author.algo == "ed25519" && seen.insert(&author.public_key_b64),
            "unsupported/duplicate author signature"
        );
        let message = if let Some(release) = &proof.release {
            let statement: AuthorSignatureV2 =
                serde_json::from_value(author.statement_json.clone())?;
            ensure!(
                statement.kind == release.kind
                    && statement.name == release.name
                    && statement.version == release.version
                    && statement.release_digest == release.digest()?,
                "author release binding mismatch"
            );
            statement.canonical_bytes()?
        } else {
            legacy_binding(&author.statement_json, artifact, false)?;
            historical_bytes(
                &author.statement_json,
                author.statement_bytes_b64.as_deref(),
            )?
        };
        // Stored rows are signatures accepted at publication. Normal current
        // revocation is metadata, not retroactive compromise invalidation.
        verify(&author.public_key_b64, &author.signature_b64, &message)?;
    }
    ensure!(
        !(policy.require_signature || proof.mode == "required") || !seen.is_empty(),
        "at least one valid trusted author signature is required"
    );
    if needs_registry_keys(artifact, policy) {
        let att = proof
            .registry_attestation
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Registry attestation required but missing"))?;
        let keys = keys
            .ok_or_else(|| anyhow::anyhow!("root-verified Registry signing keys are required"))?;
        let key = keys
            .statement
            .keys
            .iter()
            .find(|k| k.key_id == att.registry_key_id)
            .ok_or_else(|| anyhow::anyhow!("unknown Registry signing key ID"))?;
        ensure!(
            att.algo == "ed25519" && att.statement_json["registryKeyId"] == att.registry_key_id,
            "Registry attestation algorithm/key ID mismatch"
        );
        let message = if let Some(release) = &proof.release {
            let statement: RegistryAttestationV3 =
                serde_json::from_value(att.statement_json.clone())?;
            ensure!(
                statement.kind == release.kind
                    && statement.name == release.name
                    && statement.version == release.version
                    && statement.release_digest == release.digest()?,
                "Registry attestation release binding mismatch"
            );
            statement.canonical_bytes()?
        } else {
            legacy_binding(&att.statement_json, artifact, true)?;
            historical_bytes(&att.statement_json, att.statement_bytes_b64.as_deref())?
        };
        if let Some(retired) = &key.retired_at {
            let stored = att.statement_json["storedAt"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("missing attestation timestamp"))?;
            ensure!(
                DateTime::parse_from_rfc3339(stored)?.timestamp() <= timestamp(retired)?,
                "attestation postdates Registry key retirement"
            );
        }
        verify(&key.public_key_b64, &att.signature_b64, &message)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
