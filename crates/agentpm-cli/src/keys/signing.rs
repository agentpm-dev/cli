use anyhow::{Context, anyhow};
use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use chacha20poly1305::{
    XChaCha20Poly1305, // 24-byte nonce variant
    XNonce,            // 24-byte nonce type
    aead::{Aead, KeyInit},
};
use directories::ProjectDirs;
use rand_core::{OsRng, RngCore};
use rpassword::prompt_password;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::{fs, io};

#[derive(Serialize, Deserialize)]
pub struct StoredKeyV1 {
    pub version: u8, // 1
    pub label: String,
    pub created_at: String,     // RFC3339
    pub public_key_b64: String, // 32-byte raw key, base64
    // encryption envelope:
    pub kdf: String, // "argon2id"
    pub salt_b64: String,
    pub nonce_b64: String,
    pub ciphertext_b64: String, // sealed private key bytes
}

/// Where we store key files
/// macOS:  ~/Library/Application Support/com.agentpm/AgentPM/keys
/// Linux:  ~/.config/agentpm/AgentPM/keys
/// Windows: C:\Users\<user>\AppData\Roaming\agentpm\AgentPM\keys
pub fn keystore_dir() -> anyhow::Result<PathBuf> {
    let dir = project_dirs()?.data_dir().join("keys");
    fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
}

fn project_dirs() -> anyhow::Result<ProjectDirs> {
    ProjectDirs::from("com", "agentpm", "AgentPM")
        .ok_or_else(|| anyhow!("cannot resolve OS project directories"))
}

pub fn encrypt_private(sk_bytes: &[u8], pass: &str) -> anyhow::Result<(String, String, String)> {
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

pub fn decrypt_private(stored: &StoredKeyV1, pass: &str) -> anyhow::Result<Vec<u8>> {
    // Re-derive key with the SAME salt and Argon2 params as encryption
    // (we used Argon2::default() on encrypt; must match here)
    let salt = SaltString::from_b64(&stored.salt_b64).map_err(|e| anyhow!("bad salt: {e}"))?;
    let phc = Argon2::default()
        .hash_password(pass.as_bytes(), &salt)
        .map_err(|e| anyhow!("argon2 error: {e}"))?;

    let key = phc.hash.ok_or_else(|| anyhow!("argon2 produced no hash"))?;
    let aead =
        XChaCha20Poly1305::new_from_slice(key.as_bytes()).map_err(|_| anyhow!("bad AEAD key"))?;

    // Decode the stored nonce and ciphertext (DO NOT generate a new nonce)
    let nonce_bytes = B64
        .decode(&stored.nonce_b64)
        .map_err(|e| anyhow!("bad nonce base64: {e}"))?;
    if nonce_bytes.len() != 24 {
        return Err(anyhow!("invalid nonce length (expected 24)"));
    }
    let nonce = XNonce::from_slice(&nonce_bytes);

    let ciphertext = B64
        .decode(&stored.ciphertext_b64)
        .map_err(|e| anyhow!("bad ciphertext base64: {e}"))?;

    // Decrypt
    let pt = aead
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| anyhow!("decrypt failed (wrong passphrase or corrupted file)"))?;

    Ok(pt)
}

/// Hidden input if possible; otherwise visible stdin with a warning.
/// Trims trailing newline/whitespace.
pub fn prompt_passphrase_with_fallback(prompt: &str) -> anyhow::Result<String> {
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
