use crate::error::{ApiErrorBody, Result, SdkError};
use crate::models::device::{DevicePollReq, DeviceStartReq};
use crate::models::install::{InstallInitResponse, ResolveRequest, ResolveResponse};
use crate::models::namespace::{CreateNamespaceSignerReq, RevokeNamespaceSignerReq};
use crate::{
    DevicePollRes, DeviceStartRes, ErrorWire, InitPublish, PendingWire, PublishReceipt,
    SuccessWire, User,
};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, Response, StatusCode, header::CONTENT_TYPE};
use serde_json::Value;
use std::net::IpAddr;
use std::path::Path;
use std::time::Duration;
use url::Url;

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
                .connect_timeout(Duration::from_secs(30))
                .timeout(Duration::from_secs(600))
                .user_agent("agentpm-cli/0.1")
                .no_gzip()
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
        let raw = self.base_url.trim_end_matches('/');

        // Try robust parsing first
        if let Ok(u) = Url::parse(raw) {
            let host = u.host_str().unwrap_or("");

            // localhost or any IP: don't rewrite
            let is_ip = host.parse::<IpAddr>().is_ok();
            let is_local = host.eq_ignore_ascii_case("localhost");
            if is_ip || is_local || host.starts_with("api.") {
                return trim_trailing_slash(u.as_str());
            }

            // Rewrite www/apex → api.<domain>, keep scheme/port/path
            let rewritten = format!("api.{}", host.trim_start_matches("www."));
            let mut out = u;
            // set_host only changes the host; scheme/port/path preserved
            let _ = out.set_host(Some(&rewritten));
            return trim_trailing_slash(out.as_str());
        }

        // Fallback heuristic if parse fails (rare)
        if raw.contains("localhost") || raw.contains("://api.") {
            raw.to_string()
        } else if let Some((scheme, rest)) = raw.split_once("://") {
            format!("{}://api.{}", scheme, rest.trim_start_matches("www."))
        } else {
            format!("https://api.{}", raw.trim_start_matches("www."))
        }
    }

    fn auth(&self, rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(t) = &self.token {
            rb.bearer_auth(t)
        } else {
            rb
        }
    }

    /// GET /whoami
    pub async fn whoami(&self) -> SdkResult<User> {
        let url = format!("{}/v1/whoami", self.api_base());
        let resp = self.auth(self.http.get(url)).send().await?;

        let whoami_resp = self.ensure_success(resp).await?;
        let whoami = whoami_resp
            .json::<User>()
            .await
            .map_err(|e| SdkError::Other(format!("parsing init response: {}", e)))?;

        Ok(whoami)
    }

    /// New flow:
    /// 1) POST /v1/tools/publish/init (JSON metadata)
    /// 2) PUT bytes to S3 presigned URL with returned headers
    /// 3) POST /v1/tools/publish/finalize (JSON { upload_id })
    pub async fn publish_tool_from_path(
        &self,
        metadata: &Value,
        artifact_path: impl AsRef<Path>,
        _suggested_filename: &str, // used only for logs/errors; S3 key is determined server-side
        finalize_extra: Option<serde_json::Value>,
    ) -> SdkResult<PublishReceipt> {
        // Read artifact (MVP: buffer; TODO: streaming variant can come later)
        let bytes = tokio::fs::read(&artifact_path).await.map_err(|e| {
            SdkError::Other(format!(
                "reading artifact {}: {}",
                artifact_path.as_ref().display(),
                e
            ))
        })?;
        let size_bytes = bytes.len() as u64;

        // --- Step 1: init ---
        let init_url = format!("{}/v1/tools/publish/init", self.api_base());
        let init_resp = self
            .auth(self.http.post(init_url))
            .json(metadata)
            .send()
            .await
            .map_err(|e| SdkError::Other(e.to_string()))?;

        let init_resp = self.ensure_success(init_resp).await?;
        let init = init_resp
            .json::<InitPublish>()
            .await
            .map_err(|e| SdkError::Other(format!("parsing init response: {}", e)))?;

        // Optional guard: respect max_bytes if present
        if let Some(max) = init.max_bytes
            && size_bytes > max
        {
            return Err(SdkError::Other(format!(
                "artifact size {} exceeds max_bytes {} (server policy)",
                size_bytes, max
            )));
        }

        // --- Step 2: presigned PUT to S3 ---
        // Build headers exactly as server specified
        let mut hdrs = HeaderMap::new();
        for (k, v) in init.headers.iter() {
            let name = HeaderName::from_bytes(k.as_bytes()).map_err(|e| {
                SdkError::Other(format!("invalid presigned header name {}: {}", k, e))
            })?;
            let val = HeaderValue::from_str(v).map_err(|e| {
                SdkError::Other(format!("invalid presigned header value for {}: {}", k, e))
            })?;
            hdrs.insert(name, val);
        }

        let s3_put = self
            .http
            .put(&init.put_url)
            .headers(hdrs)
            .body(bytes)
            .send()
            .await
            .map_err(|e| SdkError::Other(format!("S3 PUT failed to start: {}", e)))?;

        let status = s3_put.status();
        if !status.is_success() {
            // S3 errors are XML; surface the text for debuggability
            let txt = s3_put.text().await.unwrap_or_default();
            return Err(SdkError::Other(format!(
                "S3 PUT failed: {} {}",
                status, txt
            )));
        }

        // --- Step 3: finalize ---
        let finalize_url = format!("{}/v1/tools/publish/finalize", self.api_base());

        // Always include upload_id; merge any extras supplied by the caller.
        let mut finalize_body = serde_json::json!({ "upload_id": init.upload_id });
        if let Some(extra) = finalize_extra {
            // merge shallowly (keys in extra override if conflict)
            if let Some(map) = finalize_body.as_object_mut()
                && let Some(extra_map) = extra.as_object()
            {
                for (k, v) in extra_map {
                    map.insert(k.clone(), v.clone());
                }
            }
        }

        let finalize_resp = self
            .auth(self.http.post(finalize_url))
            .json(&finalize_body)
            .send()
            .await
            .map_err(|e| SdkError::Other(e.to_string()))?;

        let finalize_resp = self.ensure_success(finalize_resp).await?;
        let receipt = finalize_resp
            .json::<PublishReceipt>()
            .await
            .map_err(|e| SdkError::Other(format!("parsing finalize response: {}", e)))?;

        Ok(receipt)
    }

    pub async fn resolve_install(&self, desired: &ResolveRequest) -> SdkResult<ResolveResponse> {
        let url = format!("{}/v1/tools/install/resolve", self.api_base());
        let resp = self
            .auth(self.http.post(url))
            .json(desired)
            .send()
            .await
            .map_err(|e| SdkError::Other(e.to_string()))?;

        let resp = self.ensure_success(resp).await?;
        let resolve_resp = resp
            .json::<ResolveResponse>()
            .await
            .map_err(|e| SdkError::Other(format!("parsing resolve response: {}", e)))?;

        Ok(resolve_resp)
    }

    pub async fn install_init(&self, plan: &ResolveResponse) -> SdkResult<InstallInitResponse> {
        let url = format!("{}/v1/tools/install/init", self.api_base());
        let resp = self
            .auth(self.http.post(url))
            .json(plan)
            .send()
            .await
            .map_err(|e| SdkError::Other(e.to_string()))?;

        let resp = self.ensure_success(resp).await?;
        let resolve_resp = resp
            .json::<InstallInitResponse>()
            .await
            .map_err(|e| SdkError::Other(format!("parsing init response: {}", e)))?;

        Ok(resolve_resp)
    }

    pub async fn install_finalize(&self, session_id: &str) -> SdkResult<()> {
        let url = format!("{}/v1/tools/install/finalize", self.api_base());
        let resp = self
            .auth(self.http.post(url))
            .json(&serde_json::json!({"session_id": session_id}))
            .send()
            .await
            .map_err(|e| SdkError::Other(e.to_string()))?;

        let _ = self.ensure_success(resp).await?;

        Ok(())
    }

    pub async fn cli_device_start(
        &self,
        scopes: &[String],
        client_name: &str,
        device_meta: Value,
    ) -> SdkResult<DeviceStartRes> {
        let req = DeviceStartReq {
            scopes: scopes.to_vec(),
            client: client_name.to_string(),
            device_meta,
        };

        let url = format!("{}/cli/device/start", self.api_base());
        let resp = self
            .auth(self.http.post(url))
            .json(&req)
            .send()
            .await
            .map_err(|e| SdkError::Other(e.to_string()))?;

        let resp = self.ensure_success(resp).await?;
        let start_resp = resp
            .json::<DeviceStartRes>()
            .await
            .map_err(|e| SdkError::Other(format!("parsing device/start response: {}", e)))?;

        Ok(start_resp)
    }

    pub async fn cli_device_poll(&self, device_code: &str) -> SdkResult<DevicePollRes> {
        let req = DevicePollReq {
            device_code: device_code.to_string(),
        };

        let url = format!("{}/cli/device/poll", self.api_base());
        let resp = self
            .auth(self.http.post(url.clone()))
            .json(&req)
            .send()
            .await
            .map_err(|e| SdkError::Other(e.to_string()))?;

        let status = resp.status();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| SdkError::Other(format!("reading device/poll body: {}", e)))?;

        // 2xx: either pending or success
        if status.is_success() {
            // Peek to decide which wire struct to use
            // Avoid failing on minor shape differences
            let val: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);

            if val.get("pat").is_some() {
                // Success payload
                let ok: SuccessWire = serde_json::from_slice(&bytes)
                    .map_err(|e| SdkError::Other(format!("parsing device/poll success: {}", e)))?;
                return Ok(DevicePollRes::Success {
                    pat: ok.pat,
                    token_type: ok.token_type,
                    scopes: ok.scopes,
                    created_at: ok.created_at,
                });
            }

            if val.get("status").is_some() {
                let p: PendingWire = serde_json::from_slice(&bytes)
                    .map_err(|e| SdkError::Other(format!("parsing device/poll pending: {}", e)))?;
                if p.status == "authorization_pending" {
                    return Ok(DevicePollRes::AuthorizationPending {
                        interval: p.interval,
                    });
                }
                return Err(SdkError::Other(format!(
                    "unexpected device/poll 200 body: {}",
                    String::from_utf8_lossy(&bytes)
                )));
            }

            return Err(SdkError::Other(format!(
                "unexpected device/poll 200 body: {}",
                String::from_utf8_lossy(&bytes)
            )));
        }

        // 400: backend sends structured { error: ... }
        if status == StatusCode::BAD_REQUEST {
            if let Ok(errw) = serde_json::from_slice::<ErrorWire>(&bytes) {
                return Ok(match errw.error.as_str() {
                    "access_denied" => DevicePollRes::Denied,
                    "expired_token" => DevicePollRes::Expired,
                    "server_error" => DevicePollRes::ServerError,
                    _ => {
                        // Unknown 400 error shape; treat as server error so caller can retry/backoff
                        DevicePollRes::ServerError
                    }
                });
            }
            return Err(SdkError::Other(format!(
                "device/poll 400 body: {}",
                String::from_utf8_lossy(&bytes)
            )));
        }

        // 401/404/429: keep parity with ensure_success mappings
        match status.as_u16() {
            401 => return Err(SdkError::Unauthorized),
            404 => return Err(SdkError::NotFound),
            429 => {
                // try to parse Retry-After; we don't have headers now, so just signal rate limit
                return Err(SdkError::RateLimited { retry_after: None });
            }
            _ => {}
        }

        // 5xx: let the caller keep polling briefly
        if status.is_server_error() {
            return Ok(DevicePollRes::ServerError);
        }

        // Everything else → opaque error
        Err(SdkError::Other(format!(
            "device/poll HTTP {}: {}",
            status,
            String::from_utf8_lossy(&bytes)
        )))
    }

    pub async fn create_namespace_signer(
        &self,
        namespace: String,
        label: String,
        public_key_b64: &str,
    ) -> SdkResult<Response> {
        let req = CreateNamespaceSignerReq {
            label,
            public_key_b64: public_key_b64.to_string(),
            algo: "ed25519".parse().unwrap(),
        };

        let url = format!("{}/namespaces/{}/signers", self.api_base(), namespace);
        let resp = self
            .auth(self.http.post(url))
            .json(&req)
            .send()
            .await
            .map_err(|e| SdkError::Other(e.to_string()))?;

        let resp = self.ensure_success(resp).await?;

        Ok(resp)
    }

    pub async fn revoke_namespace_signer(
        &self,
        namespace: String,
        signer_id: String,
    ) -> SdkResult<Response> {
        let req = RevokeNamespaceSignerReq { is_active: false };

        let url = format!(
            "{}/namespaces/{}/signers/{}",
            self.api_base(),
            namespace,
            signer_id
        );
        let resp = self
            .auth(self.http.patch(url))
            .json(&req)
            .send()
            .await
            .map_err(|e| SdkError::Other(e.to_string()))?;

        let resp = self.ensure_success(resp).await?;

        Ok(resp)
    }

    /// Centralized error mapping; returns the same Response on success.
    async fn ensure_success(&self, resp: Response) -> SdkResult<Response> {
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }

        // Copy headers out (no borrows tied to `resp`)
        let headers = resp.headers().clone();

        // Grab headers we care about before consuming the body
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        let ct = headers
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("<none>");

        // Consume body once
        let bytes = resp.bytes().await.unwrap_or_default();

        // Map common statuses
        match status.as_u16() {
            401 => return Err(SdkError::Unauthorized),
            404 => return Err(SdkError::NotFound),
            429 => return Err(SdkError::RateLimited { retry_after }),
            _ => {}
        }

        // Try a structured JSON error first (even if the content-type is wrong)
        if !bytes.is_empty()
            && let Ok(body) = serde_json::from_slice::<ApiErrorBody>(&bytes)
        {
            if body.code.is_some() || body.message.is_some() || body.details.is_some() {
                return Err(SdkError::Api {
                    status: status.as_u16(),
                    body,
                });
            }

            // Fallback: show status, content-type, and body text (truncated)
            let text = String::from_utf8_lossy(&bytes);
            return Err(SdkError::Other(format!(
                "HTTP {} ({ct}): {}",
                status,
                truncate(&text, 2000)
            )));
        }

        Err(SdkError::Other(format!("HTTP {}", status)))
    }
}

fn trim_trailing_slash(s: &str) -> String {
    s.trim_end_matches('/').to_string()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}… [truncated {} bytes]", &s[..max], s.len() - max)
    }
}
