use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// The structured error body returned by the API when status != 2xx
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorBody {
    #[serde(alias = "code", alias = "error_code", alias = "type")]
    pub code: Option<String>, // e.g., "unauthorized", "not_found"
    #[serde(alias = "message", alias = "error", alias = "title")]
    pub message: Option<String>, // human-readable message
    #[serde(default)]
    pub details: Option<serde_json::Value>, // extra context (optional)
}

impl fmt::Display for ApiErrorBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = self.code.as_deref().unwrap_or("api_error");
        let msg = self.message.as_deref().unwrap_or("request failed");
        write!(f, "{code}: {msg}")?;
        if let Some(d) = &self.details {
            // keep it short; show one-line JSON if present
            let s = d.to_string();
            if !s.is_empty() && s != "null" && s != "{}" {
                write!(f, " — {s}")?;
            }
        }
        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum SdkError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("unauthorized")]
    Unauthorized,

    #[error("not found")]
    NotFound,

    #[error("rate limited (retry_after={retry_after:?} seconds)")]
    RateLimited { retry_after: Option<u64> },

    #[error("HTTP {status}: {body}")]
    Api { status: u16, body: ApiErrorBody },

    #[error("{0}")]
    Other(String),
}

/// Convenience alias used throughout the SDK
pub type Result<T> = std::result::Result<T, SdkError>;

pub type SdkResult<T> = std::result::Result<T, SdkError>;
