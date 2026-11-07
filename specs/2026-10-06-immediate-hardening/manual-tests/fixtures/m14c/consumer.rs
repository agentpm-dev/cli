//! Manual fixture only: exercise the real shared consumer with public dev proof.
use agentpm_sdk::{
    AgentPmClient,
    models::install::PackageArtifact,
    provenance::{self, SignedKeySet, VerificationPolicy},
};
use anyhow::{Result, bail, ensure};
use std::{
    env, fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    ensure!(
        args.len() >= 4,
        "usage: m14c-consumer keys BASE CACHE | verify BASE PROOF KEYSET [--require-signature]"
    );
    ensure!(
        args[2] == "http://127.0.0.1:5000",
        "only the exact development origin is allowed"
    );
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
    let roots = provenance::pinned_roots()?;
    match args[1].as_str() {
        "keys" => {
            AgentPmClient::new(&args[2])?
                .verified_registry_keys(&roots, Some(Path::new(&args[3])), now)
                .await?;
            println!(
                "PASS: real client authenticated the dev key endpoint and populated its cache."
            );
        }
        "verify" => {
            ensure!(args.len() >= 5, "supply the saved public key-set file");
            let keyset: SignedKeySet = serde_json::from_slice(&fs::read(&args[4])?)?;
            let keys = keyset.verify(&roots, &args[2], now)?;
            let artifact: PackageArtifact = serde_json::from_slice(&fs::read(&args[3])?)?;
            provenance::verify_artifact(
                &artifact,
                Some(&keys),
                VerificationPolicy {
                    require_signature: args.iter().any(|s| s == "--require-signature"),
                    require_attestation: true,
                },
            )?;
            println!(
                "PASS: actual Rust consumer verified {}@{} with root-trusted attestation.",
                artifact.name, artifact.version
            );
        }
        _ => bail!("unknown action"),
    }
    Ok(())
}
