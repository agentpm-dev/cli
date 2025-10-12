use crate::auth;
use crate::auth::TokenCache;
use crate::config::Config;
use crate::prelude::*;
use agentpm_sdk::{DevicePollRes, DeviceStartRes};
use anyhow::bail;
use is_terminal::IsTerminal;
use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

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

    /// Force browser-assisted device flow
    #[arg(long)]
    pub device: bool,

    /// Do not auto-open the browser in device flow
    #[arg(long)]
    pub no_open: bool,

    /// Override device-flow timeout (seconds)
    #[arg(long, value_name = "SECS", default_value_t = 600)]
    pub timeout: u64,

    /// Optional starting poll interval (seconds). If 0, use server-provided.
    #[arg(long, value_name = "SECS", default_value_t = 0)]
    pub interval: u64,

    /// Optional scopes to request (repeat flag)
    #[arg(long = "scope", value_name = "SCOPE")]
    pub scopes: Vec<String>,

    /// Optional positional token; "-" means stdin
    pub token_opt: Option<String>,
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

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn try_open(url: &str) {
    if let Err(e) = open::that(url) {
        eprintln!("(Tip) Couldn't open browser automatically: {e}");
    }
}

// Validates PAT via whoami, prints user-friendly messages, and optionally persists.
// Returns:
//   Ok(true)  -> token valid (and written unless no_write)
//   Ok(false) -> token invalid/forbidden/other API error (already printed; no write)
//   Err(..)   -> unexpected local error (e.g., writing file failed)
pub async fn try_whoami_and_maybe_persist(
    cfg: &Config,
    base_url: &str,
    pat: &str,
    no_write: bool,
) -> Result<bool> {
    let client = AgentPmClient::new(base_url.to_string())?.with_token(pat.to_owned());

    match client.whoami().await {
        Ok(who) => {
            if no_write {
                println!(
                    "✅  Token valid for {} ({:?}). (--no-write set; nothing saved.)",
                    who.email,
                    who.scopes.clone().unwrap_or_else(Vec::new)
                );
            } else {
                auth::write_token(
                    cfg,
                    &TokenCache {
                        access_token: pat.to_owned(),
                    },
                )?;
                println!(
                    "✅  Logged in as {} ({:?})\nSaved credentials to: {}",
                    who.email,
                    who.scopes.clone().unwrap_or_else(Vec::new),
                    cfg.token_file.display()
                );
            }
            Ok(true)
        }
        Err(e) => {
            // Friendly one-liners; do NOT return Err to avoid a backtrace
            let msg = e.to_string();
            let lmsg = msg.to_ascii_lowercase();
            if msg.contains("401")
                || lmsg.contains("unauthorized")
                || lmsg.contains("invalid token")
            {
                eprintln!(
                    "Token validation failed (401 Unauthorized). Is the PAT correct and unrevoked?"
                );
            } else if msg.contains("403") || lmsg.contains("forbidden") {
                eprintln!("Token validated but lacks required scopes (403 Forbidden).");
            } else {
                eprintln!("Failed to validate token: {msg}");
            }
            Ok(false)
        }
    }
}

impl LoginArgs {
    pub async fn run(self, base_url: String) -> Result<()> {
        let cfg = Config::load(base_url)?;
        let stdin_is_tty = io::stdin().is_terminal();
        let stdout_is_tty = io::stdout().is_terminal();

        // Decide how to get the token:
        let token = if let Some(opt) = &self.token_opt {
            if opt == "-" {
                Some(read_token_from_stdin()?)
            } else {
                Some(opt.trim().to_owned())
            }
        } else if self.stdin || (!stdin_is_tty && !self.paste && !self.device) {
            Some(read_token_from_stdin()?)
        } else if self.paste {
            Some(prompt_for_token_hidden_or_fallback()?)
        } else {
            // Default (TTY): device flow
            None
        };

        if let Some(pat) = token {
            // Mode 1/2 behavior: validate and maybe write
            if !looks_like_pat(&pat) {
                eprintln!(
                    "Warning: token doesn’t look like an AgentPM PAT (expected to start with `apm_`). Continuing anyway…"
                );
            }

            try_whoami_and_maybe_persist(&cfg, &cfg.base_url.clone(), &pat, self.no_write).await?;
            return Ok(());
        }

        // -------- Device flow --------
        let client = AgentPmClient::new(cfg.base_url.clone())?;
        let device_meta = serde_json::json!({
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "cli_version": env!("CARGO_PKG_VERSION"),
        });

        // Start
        let start = match client
            .cli_device_start(&self.scopes, "agentpm-cli", device_meta)
            .await
        {
            Ok(res) => res,
            Err(e) => {
                // If server doesn’t have device flow yet, auto-fallback to paste UX
                let msg = format!("{e:#}");
                if msg.contains("404")
                    || msg
                        .to_ascii_lowercase()
                        .contains(&"Not Found".to_ascii_lowercase())
                    || msg.contains("501")
                {
                    eprintln!(
                        "Device login not available on this server; falling back to paste mode."
                    );
                    let pat = prompt_for_token_hidden_or_fallback()?;
                    try_whoami_and_maybe_persist(&cfg, &cfg.base_url.clone(), &pat, self.no_write)
                        .await?;
                    return Ok(());
                }
                return Err(e).context("Failed to start device login");
            }
        };

        print_device_instructions(&start, stdout_is_tty);
        if !self.no_open {
            try_open(&start.verification_uri);
        }

        // Poll
        let deadline = Instant::now() + Duration::from_secs(self.timeout);
        let mut interval = if self.interval > 0 {
            self.interval
        } else {
            start.interval.max(1)
        };
        let mut last_info = Instant::now();

        loop {
            if Instant::now() >= deadline {
                eprintln!("Login timed out after {} seconds.", self.timeout);
                break;
            }

            match client.cli_device_poll(&start.device_code).await {
                Ok(DevicePollRes::AuthorizationPending {
                    interval: server_interval,
                }) => {
                    // If server suggests a new interval, adopt it (within sane bounds)
                    if let Some(si) = server_interval {
                        // clamp 1..=30 to avoid silly values
                        let si = si.clamp(1, 30);
                        if si != interval {
                            interval = si;
                        }
                    }
                    // periodic heartbeat line
                    if last_info.elapsed() >= Duration::from_secs(15) {
                        eprintln!("Still waiting for approval… (code: {})", start.user_code);
                        last_info = Instant::now();
                    }
                }

                Ok(DevicePollRes::Denied) => {
                    eprintln!("Access denied in browser. Aborting.");
                    break;
                }

                Ok(DevicePollRes::Expired) => {
                    eprintln!("Code expired. Re-run `agentpm login`.");
                    break;
                }

                Ok(DevicePollRes::ServerError) => {
                    // transient hiccup; back off a bit
                    interval = (interval + 2).min(15);
                    eprintln!("Server error during polling; backing off to {}s…", interval);
                }

                Ok(DevicePollRes::Success { pat, .. }) => {
                    // Validate + persist using your helper
                    let _ = try_whoami_and_maybe_persist(&cfg, &cfg.base_url, &pat, self.no_write)
                        .await?;
                    break;
                }

                Err(e) => {
                    // Network hiccup—don’t crash immediately
                    eprintln!("Polling error: {e}. Retrying…");
                }
            }

            tokio::time::sleep(Duration::from_secs(interval)).await;
        }

        Ok(())
    }
}

fn print_device_instructions(start: &DeviceStartRes, stdout_is_tty: bool) {
    println!("To continue, approve this device in your browser:");
    println!("  URL : {}", start.verification_uri);
    println!("  CODE: {}", start.user_code);
    if stdout_is_tty {
        println!(
            "\n(We’ll keep polling for up to {} minutes. Press Ctrl+C to cancel.)",
            (start.expires_in as f64 / 60.0).ceil() as u64
        );
    }
}
