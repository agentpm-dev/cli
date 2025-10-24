use crate::auth::{read_key_file, write_key_file_atomic};
use crate::prelude::*;
use anyhow::anyhow;
use argon2::{
    Argon2, PasswordHasher,
    password_hash::{PasswordHash, SaltString},
};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use chacha20poly1305::{
    XChaCha20Poly1305, // 24-byte nonce variant
    XNonce,            // 24-byte nonce type
    aead::{Aead, KeyInit},
};
use directories::ProjectDirs;
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand_core::{OsRng, RngCore};
use rpassword::prompt_password;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, Write};
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

#[derive(Serialize, Deserialize)]
pub struct StoredKeyV1 {
    version: u8, // 1
    label: String,
    created_at: String,     // RFC3339
    public_key_b64: String, // 32-byte raw key, base64
    // encryption envelope:
    kdf: String, // "argon2id"
    salt_b64: String,
    nonce_b64: String,
    ciphertext_b64: String, // sealed private key bytes
}

fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("com", "agentpm", "AgentPM")
        .ok_or_else(|| anyhow!("cannot resolve OS project directories"))
}

/// Where we store key files
/// macOS:  ~/Library/Application Support/com.agentpm/AgentPM/keys
/// Linux:  ~/.config/agentpm/AgentPM/keys
/// Windows: C:\Users\<user>\AppData\Roaming\agentpm\AgentPM\keys
fn keystore_dir() -> Result<PathBuf> {
    let dir = project_dirs()?.data_dir().join("keys");
    fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
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

fn encrypt_private(sk_bytes: &[u8], pass: &str) -> Result<(String, String, String)> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::default();
    let hash = argon
        .hash_password(pass.as_bytes(), &salt)
        .map_err(|e| anyhow!("argon2 error: {e}"))?;
    // derive key bytes from hash.hash (32 bytes)
    let key = hash
        .hash
        .ok_or_else(|| anyhow!("argon2 produced no hash"))?;
    let aead =
        XChaCha20Poly1305::new_from_slice(key.as_bytes()).map_err(|_| anyhow!("bad AEAD key"))?;
    let mut nonce_bytes = [0u8; 24];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from(nonce_bytes);
    let ct = aead
        .encrypt(&nonce, sk_bytes)
        .map_err(|e| anyhow!("encrypt failed: {e}"))?;
    Ok((salt.to_string(), B64.encode(nonce), B64.encode(ct)))
}

#[allow(dead_code)]
fn decrypt_private(stored: &StoredKeyV1, pass: &str) -> Result<Vec<u8>> {
    let _parsed = PasswordHash::new(&format!(
        "$argon2id$v=19${}",
        // StoredKeyV1 kept the full PHC string in kdf? We kept only params via default.
        // For simplicity, re-hash using salt; Argon2::default must match derive used above.
        // We'll verify password to derive key material.
        // NOTE: we can't reconstruct PHC fully; instead: recompute and use its bytes as key.
        // To avoid mismatch, use the same "hash_password" flow again and use .hash bytes.
        // This function ignores parsed and recomputes below.
        ""
    ))
    .ok(); // not used; see note above

    // Re-derive key with same salt
    let salt = SaltString::from_b64(&stored.salt_b64).map_err(|e| anyhow!("bad salt: {e}"))?;
    let argon = Argon2::default();
    let phc = argon
        .hash_password(pass.as_bytes(), &salt)
        .map_err(|e| anyhow!("argon2 error: {e}"))?;
    let key = phc.hash.ok_or_else(|| anyhow!("argon2 produced no hash"))?;
    let aead =
        XChaCha20Poly1305::new_from_slice(key.as_bytes()).map_err(|_| anyhow!("bad AEAD key"))?;
    let mut nonce_bytes = [0u8; 24];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from(nonce_bytes);
    let pt = aead
        .decrypt(&nonce, B64.decode(&stored.ciphertext_b64)?[..].as_ref())
        .map_err(|e| anyhow!("decrypt failed: {e}"))?;
    Ok(pt)
}

/// Hidden input if possible; otherwise visible stdin with a warning.
/// Trims trailing newline/whitespace.
fn prompt_passphrase_with_fallback(prompt: &str) -> Result<String> {
    match prompt_password(prompt) {
        Ok(s) => {
            let s = s.trim().to_owned();
            if s.is_empty() {
                return Err(anyhow!("No passphrase entered."));
            }
            Ok(s)
        }
        Err(e) => {
            eprintln!("Note: couldn't enable hidden input ({e}). Falling back to visible input.");
            eprint!("{prompt}");
            io::stdout().flush().ok();
            let mut line = String::new();
            io::stdin()
                .read_line(&mut line)
                .context("failed to read passphrase")?;
            let s = line.trim().to_owned();
            if s.is_empty() {
                return Err(anyhow!("No passphrase entered."));
            }
            Ok(s)
        }
    }
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
