use crate::error::{ApiErrorBody, Result, SdkError};
use reqwest::multipart;
use reqwest::{Client, Response};
use serde_json::Value;
use std::time::Duration;

#[derive(Clone)]
pub struct AgentPmClient {
    http: Client,
    base_url: String,
    token: Option<String>,
}

type SdkResult<T> = std::result::Result<T, SdkError>;

impl AgentPmClient {
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        Ok(Self {
            http: Client::builder()
                .timeout(Duration::from_secs(10))
                .user_agent("agentpm-cli/0.1")
                .build()?,
            base_url: base_url.into(),
            token: None,
        })
    }

    /// Attach a PAT for bearer auth.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    fn api_base(&self) -> String {
        let b = self.base_url.trim_end_matches('/');
        if b.contains("://api.") {
            b.to_string()
        } else if let Some((scheme, rest)) = b.split_once("://") {
            format!("{}://api.{}", scheme, rest.trim_start_matches("www."))
        } else {
            format!("https://api.{}", b.trim_start_matches("www."))
        }
    }

    fn auth(&self, rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(t) = &self.token {
            rb.bearer_auth(t)
        } else {
            rb
        }
    }

    /// GET /whoami -> String (replace with types::User later)
    pub async fn whoami(&self) -> Result<String> {
        let url = format!("{}/whoami", self.api_base());
        let resp = self.auth(self.http.get(url)).send().await?;

        // TODO: Replace with types::User if your API returns structured JSON
        let resp = self.ensure_success(resp).await?;
        resp.text()
            .await
            .map_err(|e| SdkError::Other(e.to_string()))
    }

    /// POST /v1/tools/publish (multipart form)
    /// - metadata: JSON string
    /// - artifact: .tar.gz
    pub async fn publish_tool_from_path(
        &self,
        metadata: &Value,
        artifact_path: impl AsRef<std::path::Path>,
        suggested_filename: &str,
    ) -> Result<crate::types::PublishReceipt> {
        let url = format!("{}/v1/tools/publish", self.api_base());

        // MVP: read into memory (simple and fine for small/med artifacts)
        let bytes = tokio::fs::read(&artifact_path).await.map_err(|e| {
            SdkError::Other(format!(
                "reading artifact {}: {}",
                artifact_path.as_ref().display(),
                e
            ))
        })?;

        let part = multipart::Part::bytes(bytes)
            .file_name(suggested_filename.to_string())
            .mime_str("application/gzip")?;

        let form = multipart::Form::new()
            .text("metadata", serde_json::to_string(metadata)?)
            .part("artifact", part);

        let resp = self
            .auth(self.http.post(url))
            .multipart(form)
            .send()
            .await?;

        let resp = self.ensure_success(resp).await?;
        resp.json::<crate::types::PublishReceipt>()
            .await
            .map_err(|e| SdkError::Other(e.to_string()))
    }

    /// Centralized error mapping; returns the same Response on success.
    async fn ensure_success(&self, resp: Response) -> SdkResult<Response> {
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }

        // Grab Retry-After (before consuming the body)
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        // Consume body once for error details
        let bytes = resp.bytes().await.unwrap_or_default();

        // Map common statuses first
        match status.as_u16() {
            401 => return Err(SdkError::Unauthorized),
            404 => return Err(SdkError::NotFound),
            429 => return Err(SdkError::RateLimited { retry_after }),
            _ => {}
        }

        // Try a structured JSON error
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
