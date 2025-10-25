use crate::keys::signing::StoredKeyV1;
use crate::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenCache {
    pub access_token: String,
    // Later: expiry, refresh_token, scopes, etc.
}

pub fn resolve_token(cfg: &Config, flag_token: Option<String>) -> Result<Option<String>> {
    // 1) Explicit flag takes top priority
    if let Some(t) = flag_token
        && !t.trim().is_empty()
    {
        return Ok(Some(t));
    }
    // 2) Environment variable
    if let Ok(t) = std::env::var("AGENTPM_TOKEN") {
        let t = t.trim().to_owned();
        if !t.is_empty() {
            return Ok(Some(t));
        }
    }
    // 3) Fallback: token file (existing behavior)
    Ok(read_token(cfg)?.map(|t| t.access_token))
}

// (nice-to-have for logs)
pub fn mask_token(t: &str) -> String {
    if t.len() <= 10 {
        return "apm_****".into();
    }
    format!("{}…{}", &t[..8], &t[t.len() - 2..])
}

pub fn read_token(cfg: &Config) -> Result<Option<TokenCache>> {
    if !cfg.token_file.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&cfg.token_file)
        .with_context(|| format!("reading token file {}", cfg.token_file.display()))?;

    // Empty file → no token
    if raw.trim().is_empty() {
        return Ok(None);
    }

    let mut token: TokenCache =
        serde_json::from_str(&raw).context("parsing token JSON from cache")?;

    // Empty/whitespace token → no token
    let acc = token.access_token.trim();
    if acc.is_empty() {
        return Ok(None);
    }

    // Normalize by trimming before returning
    if acc.len() != token.access_token.len() {
        token.access_token = acc.to_owned();
    }

    Ok(Some(token))
}

pub fn write_token(cfg: &Config, token: &TokenCache) -> Result<()> {
    if let Some(parent) = cfg.token_file.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(token)?;
    let mut f = fs::File::create(&cfg.token_file)
        .with_context(|| format!("creating {}", cfg.token_file.display()))?;
    f.write_all(json.as_bytes())?;
    Ok(())
}

pub fn read_key_file(path: &Path) -> Result<StoredKeyV1> {
    let data = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let k = serde_json::from_slice::<StoredKeyV1>(&data)
        .with_context(|| format!("parsing {}", path.display()))?;
    Ok(k)
}

/// Atomic JSON write with strict perms.
pub fn write_key_file_atomic(path: &Path, key: &StoredKeyV1) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("no parent dir"))?;
    fs::create_dir_all(parent)?;

    // temp file in same dir for atomic rename
    let tmp = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name().unwrap().to_string_lossy(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_micros()
    ));

    let json = serde_json::to_vec_pretty(key)?;
    {
        let mut f = fs::File::create(&tmp)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
        }
        f.write_all(&json)?;
        // flush file contents & metadata
        f.sync_all()?;
    }

    // atomic swap into place
    fs::rename(&tmp, path)?;

    // ensure the directory entry is durable (best-effort on non-Unix)
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let dir = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY)
            .open(parent)?;
        dir.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        // on Windows and others, flush the destination file again (best effort)
        if let Ok(mut f) = fs::File::options().read(true).open(path) {
            let _ = f.sync_all();
        }
    }

    Ok(())
}
