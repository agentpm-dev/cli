use crate::auth;
use crate::auth::TokenCache;
use crate::config::Config;
use crate::prelude::*;
use anyhow::bail;
use is_terminal::IsTerminal;
use std::io::{self, Read, Write};

#[derive(Args, Debug, Default)]
pub struct LoginArgs {
    /// Prompt once to paste a PAT (input hidden). Default if TTY and no --stdin.
    #[arg(long)]
    pub paste: bool,

    /// Read a PAT from stdin (single line, trimmed). Default if not a TTY.
    #[arg(long)]
    pub stdin: bool,

    /// Validate the token but do not write credentials to disk.
    #[arg(long)]
    pub no_write: bool,
}

fn prompt_for_token_hidden_or_fallback() -> Result<String> {
    // First, try hidden input via /dev/tty
    match rpassword::prompt_password("Paste your AgentPM PAT (input hidden): ") {
        Ok(t) => {
            let t = t.trim().to_owned();
            if t.is_empty() {
                bail!("No token entered.");
            }
            Ok(t)
        }
        Err(e) => {
            // No controlling TTY or similar; fall back to visible stdin
            eprintln!("Note: couldn't enable hidden input ({e}). Falling back to visible input.");
            eprint!("Paste your AgentPM PAT (will be visible): ");
            io::stdout().flush().ok();
            let mut line = String::new();
            io::stdin()
                .read_line(&mut line)
                .context("failed to read token from stdin")?;
            let t = line.trim().to_owned();
            if t.is_empty() {
                bail!("No token entered.");
            }
            Ok(t)
        }
    }
}

fn read_token_from_stdin() -> Result<String> {
    let mut buf = String::new();
    io::stdin()
        .read_to_string(&mut buf)
        .context("failed to read stdin")?;
    let t = buf.trim().to_owned();
    if t.is_empty() {
        bail!("No token provided on stdin.");
    }
    Ok(t)
}

fn looks_like_pat(tok: &str) -> bool {
    tok.starts_with("apm_") && tok.len() > 12
}

impl LoginArgs {
    pub async fn run(self, base_url: String) -> Result<()> {
        let cfg = Config::load(base_url)?;

        // Decide how to get the token:
        // - if --stdin OR stdin is not a TTY → read from stdin
        // - otherwise try hidden prompt (with visible fallback)
        let stdin_is_tty = io::stdin().is_terminal();
        let use_stdin = self.stdin || (!stdin_is_tty && !self.paste);

        let token = if use_stdin {
            read_token_from_stdin()?
        } else {
            prompt_for_token_hidden_or_fallback()?
        };

        if !looks_like_pat(&token) {
            eprintln!(
                "Warning: token doesn’t look like an AgentPM PAT (expected to start with `apm_`). Continuing anyway…"
            );
        }

        // Validate via /auth/whoami before writing
        let client = AgentPmClient::new(cfg.base_url.clone())?.with_token(token.clone());
        match client.whoami().await {
            Ok(who) => {
                // Persist unless --no-write
                if self.no_write {
                    println!(
                        "✅ Token valid for {} ({:?}). (--no-write set; nothing saved.)",
                        who.email,
                        who.scopes.unwrap_or_else(Vec::new)
                    );
                } else {
                    auth::write_token(
                        &cfg,
                        &TokenCache {
                            access_token: token.clone(),
                        },
                    )?;
                    println!(
                        "✅  Logged in as {} ({:?})\nSaved credentials to: {}",
                        who.email,
                        who.scopes.unwrap_or_else(Vec::new),
                        cfg.token_file.display()
                    );
                }
            }
            Err(e) => {
                // Try to make 401/403 clear to users
                let msg = format!("{e}");
                if msg.contains("401")
                    || msg.contains("unauthorized")
                    || msg.contains("invalid token")
                {
                    eprintln!(
                        "Token validation failed (401 Unauthorized). Is the PAT correct and unrevoked?"
                    );
                } else if msg.contains("403") {
                    eprintln!("Token validated but lacks required scopes (403 Forbidden).");
                } else {
                    eprintln!("Failed to validate token: {msg}");
                }
            }
        };

        Ok(())
    }
}
