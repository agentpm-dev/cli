use crate::error::{ApiErrorBody, Result, SdkError};
use reqwest::Client;
use std::time::Duration;

#[derive(Clone)]
pub struct AgentPmClient {
    http: Client,
    base_url: String,
}

impl AgentPmClient {
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        Ok(Self {
            http: Client::builder()
                .timeout(Duration::from_secs(10))
                .user_agent("agentpm-cli/0.1")
                .build()?,
            base_url: base_url.into(),
        })
    }

    /// Minimal example: GET /whoami -> String (replace with types::User later)
    pub async fn whoami(&self) -> Result<String> {
        let url = format!("{}/whoami", self.base_url.trim_end_matches('/'));
        let resp = self.http.get(url).send().await?;
        let status = resp.status(); // capture before consuming body

        if resp.status().is_success() {
            // TODO: Replace with `types::User` if your API returns structured JSON
            let txt = resp.text().await?;
            return Ok(txt);
        }

        // Map common statuses
        match resp.status().as_u16() {
            401 => return Err(SdkError::Unauthorized),
            404 => return Err(SdkError::NotFound),
            429 => {
                let retry_after = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok());
                return Err(SdkError::RateLimited { retry_after });
            }
            _ => {}
        }

        // Try to parse a structured error body; fall back to plain text
        let bytes = resp.bytes().await?;
        if !bytes.is_empty() {
            if let Ok(body) = serde_json::from_slice::<ApiErrorBody>(&bytes) {
                return Err(SdkError::Api(body));
            }
            if let Ok(txt) = String::from_utf8(bytes.to_vec()) {
                return Err(SdkError::Other(txt));
            }
        }

        Err(SdkError::Other(format!("HTTP {}", status)))
    }
}
