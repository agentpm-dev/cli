use crate::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenCache {
    pub access_token: String,
    // Later: expiry, refresh_token, scopes, etc.
}

pub fn read_token(cfg: &Config) -> Result<Option<TokenCache>> {
    if !cfg.token_file.exists() {
        return Ok(None);
    }

    let text = fs::read_to_string(&cfg.token_file)
        .with_context(|| format!("reading token file {}", cfg.token_file.display()))?;
    let token: TokenCache = serde_json::from_str(&text).context("parsing token JSON from cache")?;
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

/// Placeholder "login" that just writes a demo token.
/// Replace it with real auth flow (device code / OAuth / username+password)
pub async fn login_stub(cfg: &Config) -> Result<()> {
    let demo = TokenCache {
        access_token: "DEMO_TOKEN_CHANGE_ME".to_string(),
    };
    write_token(cfg, &demo)?;
    Ok(())
}
