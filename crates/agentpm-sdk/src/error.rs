use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The structured error body returned by the API when status != 2xx
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub code: Option<String>,               // e.g., "unauthorized", "not_found"
    pub message: Option<String>,            // human-readable message
    pub details: Option<serde_json::Value>, // extra context (optional)
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

    #[error("api error: {0:?}")]
    Api(ApiErrorBody),

    #[error("{0}")]
    Other(String),
}

/// Convenience alias used throughout the SDK
pub type Result<T> = std::result::Result<T, SdkError>;
