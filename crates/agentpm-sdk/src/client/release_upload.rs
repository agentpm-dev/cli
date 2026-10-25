//! Upload transport only; release-building commands and signing come later.
use super::*;
use futures_util::StreamExt;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio_util::io::ReaderStream;

#[derive(Debug, Deserialize)]
pub struct ReleaseUpload {
    pub release_id: String,
    pub resumed: bool,
    pub expires_at: String,
    pub max_bytes: u64,
    #[serde(default)]
    pub max_total_bytes: Option<u64>,
    pub artifacts: Vec<ArtifactUpload>,
}

#[derive(Debug, Deserialize)]
pub struct ArtifactUpload {
    pub artifact_id: String,
    pub target: String,
    pub put_url: String,
    pub headers: HashMap<String, String>,
    pub expires_at: String,
    pub status: String,
    pub max_bytes: u64,
}

impl AgentPmClient {
    pub async fn init_release(&self, metadata: &Value) -> SdkResult<ReleaseUpload> {
        let response = self
            .auth(self.http.post(format!(
                "{}/v1/tools/publish/releases/init",
                self.api_base()
            )))
            .json(metadata)
            .send()
            .await?;
        Ok(self.ensure_success(response).await?.json().await?)
    }

    pub async fn renew_release_artifact(
        &self,
        release_id: &str,
        artifact_id: &str,
    ) -> SdkResult<ArtifactUpload> {
        let response = self
            .auth(self.http.post(format!(
                "{}/v1/tools/publish/releases/{release_id}/artifacts/{artifact_id}/presign",
                self.api_base()
            )))
            .json(&serde_json::json!({}))
            .send()
            .await?;
        Ok(self.ensure_success(response).await?.json().await?)
    }

    pub async fn upload_release_artifact(
        &self,
        instruction: &ArtifactUpload,
        path: impl AsRef<Path>,
    ) -> SdkResult<()> {
        self.stream_artifact(
            &instruction.put_url,
            &instruction.headers,
            path.as_ref(),
            instruction.max_bytes,
        )
        .await
    }

    pub async fn finalize_release(&self, release_id: &str) -> SdkResult<PublishReceipt> {
        let response = self
            .auth(self.http.post(format!(
                "{}/v1/tools/publish/releases/{release_id}/finalize",
                self.api_base()
            )))
            .json(&serde_json::json!({}))
            .send()
            .await?;
        Ok(self.ensure_success(response).await?.json().await?)
    }

    pub(super) async fn stream_artifact(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        path: &Path,
        maximum: u64,
    ) -> SdkResult<()> {
        let file = tokio::fs::File::open(path).await.map_err(|error| {
            SdkError::Other(format!("opening artifact {}: {error}", path.display()))
        })?;
        let size = file
            .metadata()
            .await
            .map_err(|error| SdkError::Other(error.to_string()))?
            .len();
        if size > maximum {
            return Err(SdkError::Other(format!(
                "artifact size {size} exceeds max_bytes {maximum} (server policy)"
            )));
        }
        let mut parsed = HeaderMap::new();
        for (name, value) in headers {
            parsed.insert(
                HeaderName::from_bytes(name.as_bytes())
                    .map_err(|error| SdkError::Other(error.to_string()))?,
                HeaderValue::from_str(value).map_err(|error| SdkError::Other(error.to_string()))?,
            );
        }
        let progress = Arc::new(AtomicU64::new(0));
        let observed = progress.clone();
        let stream = ReaderStream::with_capacity(file, 64 * 1024).map(move |chunk| {
            if let Ok(bytes) = &chunk {
                observed.fetch_add(bytes.len() as u64, Ordering::Relaxed);
            }
            chunk
        });
        let pending = self
            .transfer_http
            .put(url)
            .headers(parsed)
            .header(reqwest::header::CONTENT_LENGTH, size)
            .body(reqwest::Body::wrap_stream(stream))
            .send();
        tokio::pin!(pending);
        let mut clock = tokio::time::interval(Duration::from_secs(30));
        clock.tick().await; // Consume the immediate tick; four idle periods mean 120 seconds.
        let mut previous = 0;
        let mut idle = 0;
        let response = loop {
            tokio::select! {
                response = &mut pending => break response?,
                _ = clock.tick() => {
                    let current = progress.load(Ordering::Relaxed);
                    if current == previous { idle += 1; } else { idle = 0; }
                    if idle >= 4 { return Err(SdkError::Other("artifact upload made no progress; retry or renew its upload URL".into())); }
                    previous = current;
                }
            }
        };
        if !response.status().is_success() {
            return Err(SdkError::Other(format!(
                "S3 PUT failed: {}",
                response.status()
            )));
        }
        Ok(())
    }
}
