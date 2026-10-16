use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

/// Keep presentation normalization separate from filesystem resolution and JSON output.
pub fn display_path(path: &Path) -> String {
    let normalized: PathBuf = path
        .components()
        .filter(|part| *part != Component::CurDir)
        .collect();
    let cwd = std::env::current_dir().ok();
    let shown = cwd
        .as_ref()
        .and_then(|cwd| normalized.strip_prefix(cwd).ok())
        .unwrap_or(&normalized);
    if shown.as_os_str().is_empty() {
        ".".into()
    } else {
        shown.display().to_string()
    }
}

pub fn inspect_header(
    kind: &str,
    name: &str,
    version: &str,
    target: &str,
    status: &str,
    manifest: &Path,
) -> Vec<String> {
    vec![
        format!("{kind}: {name}@{version}"),
        format!("Target: {}", display_path(Path::new(target))),
        format!("Status: {status}"),
        format!("Manifest: {}", display_path(manifest)),
    ]
}

pub fn registry_error(error: anyhow::Error, action: &str, base_url: &str) -> anyhow::Error {
    if error.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<agentpm_sdk::SdkError>(),
            Some(agentpm_sdk::SdkError::Unauthorized)
        )
    }) {
        return anyhow::anyhow!(
            "Credentials rejected while trying to {action}. Check that your PAT is valid and unrevoked; update AGENTPM_TOKEN, pass --token, or run `agentpm login --paste`."
        );
    }
    let Some(http) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<reqwest::Error>())
    else {
        return error;
    };
    // Debug retains the underlying transport cause, without signed URLs/query strings.
    tracing::debug!(cause = ?std::error::Error::source(http), "Registry transport failure");
    if http.is_status() {
        return error;
    }
    let origin = http
        .url()
        .cloned()
        .or_else(|| reqwest::Url::parse(base_url).ok())
        .map(|url| url.origin().ascii_serialization())
        .unwrap_or_else(|| "the registry".into());
    let invalid_response = http.is_decode() && !http.is_timeout();
    let problem = if http.is_timeout() {
        "Request timed out"
    } else if http.is_connect() {
        "Cannot connect"
    } else if invalid_response {
        "Invalid registry response"
    } else {
        "Request failed"
    };
    let guidance = if invalid_response {
        "Check that the registry supports this CLI version, then retry."
    } else {
        "Check your network, registry address, and server availability, then retry."
    };
    anyhow::anyhow!("{problem} at {origin} while trying to {action}. {guidance}")
}

/// Human-friendly step logger that prints to STDERR.
/// - Auto-suppressed with `quiet` (e.g., --quiet or non-TTY)
/// - Methods borrow `&mut self` so you can call from closures if needed.
pub struct Step {
    label: String,
    started: Instant,
    finished: bool,
    quiet: bool,
}

impl Step {
    pub fn new(label: impl Into<String>, quiet: bool) -> Self {
        let label = label.into();
        if !quiet {
            eprintln!("• {}…", label);
        }
        Self {
            label,
            started: Instant::now(),
            finished: false,
            quiet,
        }
    }

    pub fn ok(&mut self, details: impl Into<String>) {
        if self.finished || self.quiet {
            return;
        }
        let d = details.into();
        let dur = self.started.elapsed();
        if d.is_empty() {
            eprintln!("✓ {} ({})", self.label, fmt_dur(dur));
        } else {
            eprintln!("✓ {} ({}) — {}", self.label, fmt_dur(dur), d);
        }
        self.finished = true;
    }

    pub fn err(&mut self, details: impl Into<String>) {
        if self.finished || self.quiet {
            return;
        }
        let dur = self.started.elapsed();
        eprintln!("✗ {} ({}) — {}", self.label, fmt_dur(dur), details.into());
        self.finished = true;
    }

    pub fn final_msg(msg: impl AsRef<str>, quiet: bool) {
        if quiet {
            return;
        }
        eprintln!("{}", msg.as_ref());
    }
}

fn fmt_dur(d: Duration) -> String {
    if d.as_secs() >= 1 {
        format!("{:.1}s", d.as_secs_f32())
    } else {
        format!("{}ms", d.as_millis())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_paths_remove_dot_components_without_changing_parent_or_symlink_semantics() {
        assert_eq!(
            display_path(Path::new("./some/./file")),
            Path::new("some/file").display().to_string()
        );
        assert_eq!(display_path(Path::new(".")), ".");
        assert_eq!(
            display_path(&std::env::current_dir().unwrap().join("./agent.json")),
            "agent.json"
        );
        assert_eq!(
            display_path(Path::new("symlink/../file")),
            Path::new("symlink/../file").display().to_string()
        );
    }

    #[tokio::test]
    async fn shared_timeout_message_retains_action_without_internal_endpoint_or_query() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let _connection = listener.accept().await.unwrap();
            std::future::pending::<()>().await;
        });
        let error = reqwest::Client::builder()
            .timeout(Duration::from_millis(50))
            .build()
            .unwrap()
            .get(format!(
                "http://{address}/internal/endpoint?token=not-for-display"
            ))
            .send()
            .await
            .unwrap_err();
        server.abort();
        let error = anyhow::Error::new(agentpm_sdk::SdkError::Http(error))
            .context("internal request context");
        let message = registry_error(
            error,
            "export Skill @example/tool",
            "http://registry.invalid",
        )
        .to_string();
        assert!(
            message.contains("Request timed out") && message.contains("export Skill @example/tool"),
            "{message}"
        );
        assert!(
            !message.contains("endpoint")
                && !message.contains("token=")
                && !message.contains("internal"),
            "{message}"
        );
    }

    #[test]
    fn local_non_transport_errors_keep_their_context() {
        let error = anyhow::anyhow!("missing dependency").context("loading manifest");
        assert_eq!(
            format!(
                "{:#}",
                registry_error(error, "install", "http://registry.invalid")
            ),
            "loading manifest: missing dependency"
        );
    }
}
