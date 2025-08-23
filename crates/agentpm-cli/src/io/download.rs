use agentpm_sdk::models::install as sdkm;
use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use futures::{StreamExt, stream::FuturesUnordered};
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};
use tokio::{fs, io::AsyncReadExt, io::AsyncWriteExt, task};

/// Downloads all artifacts (in parallel), verifies integrity, and extracts to tools_dir.
/// - Uses cache_dir to store .tgz tarballs.
/// - If `refresh` is false and a cached file matches integrity and tool dir already exists, it skips work.
pub async fn download_and_extract_all(
    init: &sdkm::InstallInitResponse,
    cache_dir: &Path,
    tools_dir: &Path,
    refresh: bool,
) -> Result<()> {
    fs::create_dir_all(cache_dir).await?;
    fs::create_dir_all(tools_dir).await?;

    let client = Client::new();
    let mut futs = FuturesUnordered::new();

    for art in &init.artifacts {
        let pkg = art.name.clone();
        let ver = art.version.clone();
        let integrity = art.integrity.clone();
        let url = art.presigned_url.clone();

        let cache_name = cache_filename(art);
        let cache_path = cache_dir.join(cache_name);
        let tool_dir = resolved_tool_dir(tools_dir, &pkg, &ver)?;

        // Short-circuit if cached and already extracted (unless refresh)
        if !refresh
            && try_exists(&cache_path).await?
            && verify_sha256(&cache_path, &integrity).await.is_ok()
            && dir_has_files(&tool_dir).await?
        {
            continue;
        }

        let client_cl = client.clone();
        futs.push(async move {
            // (Re)download if missing or refresh requested
            if refresh || !try_exists(&cache_path).await? {
                download_to(&client_cl, &url, &cache_path)
                    .await
                    .with_context(|| format!("downloading {}", pkg))?;
            }

            // Verify integrity
            ensure_sha256(&cache_path, &integrity)
                .await
                .with_context(|| format!("integrity check failed for {}", pkg))?;

            // Extract
            extract_tar_gz(&cache_path, &tool_dir)
                .await
                .with_context(|| format!("extracting {}@{}", pkg, ver))?;

            Ok::<(), anyhow::Error>(())
        });
    }

    while let Some(res) = futs.next().await {
        res?;
    }
    Ok(())
}

/// Stream download to a file (atomic via .part then rename).
pub async fn download_to(client: &Client, url: &str, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).await?;
    }
    let tmp = dest.with_extension("part");

    let mut resp = client.get(url).send().await?.error_for_status()?;
    let mut file = fs::File::create(&tmp).await?;
    while let Some(chunk) = resp.chunk().await? {
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    fs::rename(tmp, dest).await?;
    Ok(())
}

/// Compute and compare sha256 with the "sha256-<base64>" integrity string.
/// Returns Ok(()) if matches, Err otherwise.
pub async fn ensure_sha256(path: &Path, integrity: &str) -> Result<()> {
    let want = parse_integrity(integrity)?;
    let have = sha256_file(path).await?;
    if have == want {
        Ok(())
    } else {
        bail!("sha256 mismatch")
    }
}

/// Same as ensure_sha256 but returns Err with context if mismatched (used for skip path).
pub async fn verify_sha256(path: &Path, integrity: &str) -> Result<()> {
    ensure_sha256(path, integrity).await
}

fn parse_integrity(integrity: &str) -> Result<Vec<u8>> {
    let prefix = "sha256-";
    if !integrity.starts_with(prefix) {
        bail!("unsupported integrity (expected sha256-...)");
    }
    let b64 = &integrity[prefix.len()..];
    let bytes = BASE64
        .decode(b64.as_bytes())
        .map_err(|e| anyhow!("invalid base64 in integrity: {}", e))?;
    if bytes.len() != 32 {
        bail!("invalid sha256 length in integrity");
    }
    Ok(bytes)
}

async fn sha256_file(path: &Path) -> Result<Vec<u8>> {
    let mut file = fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 64];
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_vec())
}

/// Extract a .tar.gz into `dest_dir`, safely:
/// - Prevents absolute paths and `..` components
/// - Skips symlinks/hardlinks
pub async fn extract_tar_gz(tar_gz_path: &Path, dest_dir: &Path) -> Result<()> {
    let tar_gz_path = tar_gz_path.to_path_buf();
    let dest_dir = dest_dir.to_path_buf();

    // Do the blocking tar I/O in a blocking thread
    task::spawn_blocking(move || {
        std::fs::create_dir_all(&dest_dir)?;

        let file = std::fs::File::open(&tar_gz_path)?;
        let gz = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(gz);

        for entry in archive.entries()? {
            let mut entry = entry?;

            let entry_type = entry.header().entry_type();
            // Disallow links for safety in MVP
            if entry_type.is_symlink() || entry_type.is_hard_link() {
                continue;
            }

            let raw_path = entry.path()?;
            let safe_rel = sanitize_entry_path(&raw_path)?;
            let out_path = dest_dir.join(&safe_rel);

            // Ensure parent exists
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            entry.unpack(&out_path)?;
        }

        Ok::<(), anyhow::Error>(())
    })
    .await?
}

/// Rejects absolute paths, Windows prefixes, and any `..` components.
/// Returns a cleaned relative path to be joined under the destination.
fn sanitize_entry_path(p: &Path) -> Result<PathBuf> {
    let mut clean = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::Normal(s) => clean.push(s),
            Component::CurDir => {} // ignore
            Component::ParentDir => bail!("tar entry contains parent dir (..)"),
            Component::RootDir | Component::Prefix(_) => bail!("tar entry has absolute/drive path"),
        }
    }
    Ok(clean)
}

/// Return true if directory exists and has at least one entry.
async fn dir_has_files(p: &Path) -> Result<bool> {
    if !try_exists(p).await? {
        return Ok(false);
    }
    let mut rd = fs::read_dir(p).await?;
    Ok(rd.next_entry().await?.is_some())
}

async fn try_exists(p: &Path) -> Result<bool> {
    Ok(fs::metadata(p).await.is_ok())
}

/// Cache name like "zack-summarize-1.3.4.tgz"
fn cache_filename(art: &sdkm::InstallArtifact) -> String {
    let (owner, name) = split_package(&art.name).unwrap_or(("unknown".into(), "unknown".into()));
    format!("{}-{}-{}.tgz", owner, name, art.version)
}

/// Resolve to .agentpm/tools/<owner>/<name>/<version>
fn resolved_tool_dir(base: &Path, package: &str, version: &str) -> Result<PathBuf> {
    let (owner, name) = split_package(package)?;
    Ok(base.join(owner).join(name).join(version))
}

/// Split "@owner/name" into (owner, name)
fn split_package(package: &str) -> Result<(String, String)> {
    if !package.starts_with('@') {
        bail!("package must be of form @owner/name");
    }
    let mut parts = package[1..].splitn(2, '/');
    let owner = parts.next().ok_or_else(|| anyhow!("invalid package"))?;
    let name = parts.next().ok_or_else(|| anyhow!("invalid package"))?;
    Ok((owner.to_string(), name.to_string()))
}
