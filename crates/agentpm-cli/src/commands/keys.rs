use crate::auth::{read_key_file, write_key_file_atomic};
use crate::keys::signing::{
    StoredKeyV1, encrypt_private, keystore_dir, prompt_passphrase_with_fallback,
};
use crate::prelude::*;
use anyhow::anyhow;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand_core::OsRng;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Args, Debug)]
pub struct KeysArgs {
    #[command(subcommand)]
    pub command: KeysCmd,
}

#[derive(Subcommand, Debug)]
pub enum KeysCmd {
    /// Create a new ed25519 keypair locally
    Generate {
        #[arg(long)]
        label: String,
    },
    /// List local keys
    List,
    /// Export the public key (base64 raw 32 bytes) for registration
    Export {
        /// Key id (fingerprint prefix) to export
        #[arg(long)]
        key_id: String,
        /// Output file (defaults to stdout if omitted)
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

fn key_path(key_id: &str) -> Result<PathBuf> {
    Ok(keystore_dir()?.join(format!("{key_id}.json")))
}

/// Produce a stable, filename-safe id from the raw 32-byte public key (base64):
/// id = base64url(SHA256(raw_pubkey)) prefix, e.g. first 16 chars
pub fn key_id_from_pub_b64(pub_b64: &str) -> anyhow::Result<String> {
    let raw = base64::engine::general_purpose::STANDARD.decode(pub_b64)?;
    let fp = B64URL.encode(Sha256::digest(&raw));
    // choose length: 12–20 chars; 16 is a nice balance
    Ok(fp[..16].to_string())
}

impl KeysArgs {
    pub async fn run(self) -> Result<()> {
        match self.command {
            KeysCmd::Generate { label } => {
                // prompt for passphrase (twice)
                let pass1 = prompt_passphrase_with_fallback("New key passphrase: ")?;
                let pass2 = prompt_passphrase_with_fallback("Repeat passphrase: ")?;
                if pass1 != pass2 {
                    return Err(anyhow!("Passphrases do not match"));
                }

                // generate ed25519
                let sk = SigningKey::generate(&mut OsRng);
                let vk: VerifyingKey = sk.verifying_key();
                let pub_raw = vk.to_bytes(); // 32 bytes
                let pub_b64 = B64.encode(pub_raw);

                // encrypt private key bytes
                let (salt, nonce_b64, ct_b64) = encrypt_private(&sk.to_bytes(), &pass1)?;

                let created_at = OffsetDateTime::now_utc().format(&Rfc3339).unwrap();
                let stored = StoredKeyV1 {
                    version: 1,
                    label: label.clone(),
                    created_at,
                    public_key_b64: pub_b64.clone(),
                    kdf: "argon2id".to_string(),
                    salt_b64: salt,
                    nonce_b64,
                    ciphertext_b64: ct_b64,
                };
                let id = key_id_from_pub_b64(&pub_b64)?;
                let path = key_path(&id)?;
                write_key_file_atomic(&path, &stored)?;
                println!("✅  Created key \"{label}\"");
                println!("   id: {id}");
                println!("   stored at: {}", path.display());
            }

            KeysCmd::List => {
                let dir = keystore_dir()?;
                let mut rows = vec![];

                if dir.exists() {
                    for ent in fs::read_dir(&dir)? {
                        let ent = ent?;
                        if !ent.file_type()?.is_file() {
                            continue;
                        }
                        if !ent.file_name().to_string_lossy().ends_with(".json") {
                            continue;
                        }

                        let path = ent.path();
                        match read_key_file(&path) {
                            Ok(k) => {
                                let id = key_id_from_pub_b64(&k.public_key_b64)?;
                                rows.push((id, k.label, k.created_at));
                            }
                            Err(_) => {
                                // optionally log a debug line here if you want:
                                // eprintln!("Skipping unreadable key file: {}", path.display());
                                continue;
                            }
                        }
                    }
                }

                if rows.is_empty() {
                    println!("(no keys)");
                } else {
                    // optional: stable ordering by created_at then label
                    rows.sort_by(|a, b| a.2.cmp(&b.2).then(a.1.cmp(&b.1)));
                    for (id, label, ts) in rows {
                        println!("{id}\t{label}\t{ts}");
                    }
                }
            }

            KeysCmd::Export { key_id, out } => {
                let path = key_path(&key_id)?;
                let k =
                    read_key_file(&path).with_context(|| format!("reading {}", path.display()))?;

                // We only export the **public** key in raw base64 (32 bytes)
                if let Some(f) = out {
                    fs::write(&f, format!("{}\n", k.public_key_b64))?;
                    println!("Wrote public key to {}", f.display());
                } else {
                    println!("{}", k.public_key_b64);
                }
            }
        }
        Ok(())
    }
}
