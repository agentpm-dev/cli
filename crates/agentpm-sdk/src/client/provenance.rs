use super::AgentPmClient;
use crate::models::install::PackageArtifact;
use crate::provenance::{self, SignedKeySet, TrustRoot, VerificationPolicy, VerifiedKeySet};
use anyhow::{Context, Result, ensure};
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_BYTES: usize = 262144;

impl AgentPmClient {
    pub async fn verified_registry_keys(
        &self,
        roots: &[TrustRoot],
        cache: Option<&Path>,
        now: i64,
    ) -> Result<VerifiedKeySet> {
        let base = self.api_base();
        let origin = url::Url::parse(&base)?.origin().ascii_serialization();
        ensure!(
            roots.iter().any(|root| root.registry == origin),
            "This CLI has no audited Registry root for {origin}; complete Registry root bootstrap and install a CLI with its public pin before requiring attestations"
        );
        let path = cache.map(|directory| {
            directory.join(format!("{:x}.json", Sha256::digest(origin.as_bytes())))
        });
        let response = self
            .http
            .get(format!("{base}/v1/registry/signing-keys"))
            .send()
            .await;
        let response = match response {
            Ok(value) if value.status().is_success() => value,
            other => {
                if let Some(path) = path
                    .as_ref()
                    .filter(|p| p.metadata().is_ok_and(|m| m.len() <= MAX_BYTES as u64))
                {
                    let saved: SignedKeySet = serde_json::from_slice(&fs::read(path)?)?;
                    return saved
                        .verify(roots, &origin, now)
                        .context("cached Registry key set is not trusted/valid");
                }
                anyhow::bail!(
                    "Authenticated Registry signing keys unavailable and no verified cache is available: {}",
                    match other {
                        Ok(r) => r.status().to_string(),
                        Err(e) => e.to_string(),
                    }
                );
            }
        };
        ensure!(
            response.content_length().unwrap_or(0) <= MAX_BYTES as u64,
            "Registry key set exceeds size limit"
        );
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            ensure!(
                bytes.len() + chunk.len() <= MAX_BYTES,
                "Registry key set exceeds size limit"
            );
            bytes.extend_from_slice(&chunk);
        }
        // Invalid 200 responses are never hidden by cache fallback or persisted.
        let signed: SignedKeySet = serde_json::from_slice(&bytes)?;
        let verified = signed.verify(roots, &origin, now)?;
        if let Some(path) = path
            && let Some(parent) = path.parent()
            && fs::create_dir_all(parent).is_ok()
        {
            let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
            if fs::write(&temporary, bytes).is_ok() {
                let _ = fs::rename(&temporary, path);
                let _ = fs::remove_file(temporary);
            }
        }
        Ok(verified)
    }

    pub async fn verify_install_provenance(
        &self,
        artifacts: &[PackageArtifact],
        policy: VerificationPolicy,
        cache: Option<&Path>,
    ) -> Result<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        let keys = if artifacts
            .iter()
            .any(|a| provenance::needs_registry_keys(a, policy))
        {
            Some(
                self.verified_registry_keys(&provenance::pinned_roots()?, cache, now)
                    .await?,
            )
        } else {
            None
        };
        for artifact in artifacts {
            provenance::verify_artifact(artifact, keys.as_ref(), policy).with_context(|| {
                format!(
                    "Provenance verification failed for {}@{}",
                    artifact.name, artifact.version
                )
            })?;
        }
        Ok(())
    }
}
