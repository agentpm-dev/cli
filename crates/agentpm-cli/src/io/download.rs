use agentpm_sdk::models::install as sdkm;
use anyhow::{Context, Result, anyhow, bail};
use futures::{StreamExt, stream::FuturesUnordered};
use hex::FromHex;
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::{fs, io::AsyncReadExt, io::AsyncWriteExt, task};

#[cfg(test)]
#[path = "download/target_tests.rs"]
mod target_tests;

#[derive(Clone, Copy)]
pub struct InstallRoots<'a> {
    pub tools_dir: &'a Path,
    pub agents_dir: &'a Path,
    pub skills_dir: &'a Path,
    pub knowledge_dir: &'a Path,
    pub memory_dir: &'a Path,
    pub profiles_dir: &'a Path,
    pub loops_dir: &'a Path,
}

/// Return canonical + alias command names for a runtime type.
/// Primary first so the message shows the expected name first.
fn runtime_cmd_candidates(rt_type: &str) -> Option<Vec<&'static str>> {
    match rt_type.to_ascii_lowercase().as_str() {
        "node" | "nodejs" => Some(vec!["node", "nodejs"]),
        "python" | "python3" => Some(vec!["python", "python3"]),
        _ => None, // unknown runtime: don't attempt to run anything
    }
}

/// Downloads all artifacts (in parallel), verifies integrity, and extracts to tools_dir.
/// - Uses cache_dir to store .tgz tarballs.
/// - If `refresh` is false and a cached file matches integrity and tool dir already exists, it skips work.
pub async fn download_and_extract_all(
    init: &sdkm::InstallInitResponse,
    cache_dir: &Path,
    install_roots: InstallRoots<'_>,
    refresh: bool,
    quiet: bool,
) -> Result<()> {
    download_and_extract_with_client(init, cache_dir, install_roots, refresh, quiet, None).await
}

pub async fn download_and_extract_with_client(
    init: &sdkm::InstallInitResponse,
    cache_dir: &Path,
    install_roots: InstallRoots<'_>,
    refresh: bool,
    quiet: bool,
    registry: Option<&agentpm_sdk::AgentPmClient>,
) -> Result<()> {
    fs::create_dir_all(cache_dir).await?;
    fs::create_dir_all(install_roots.tools_dir).await?;
    fs::create_dir_all(install_roots.agents_dir).await?;
    fs::create_dir_all(install_roots.skills_dir).await?;
    fs::create_dir_all(install_roots.knowledge_dir).await?;
    fs::create_dir_all(install_roots.memory_dir).await?;
    fs::create_dir_all(install_roots.profiles_dir).await?;
    fs::create_dir_all(install_roots.loops_dir).await?;

    let client = Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .read_timeout(Duration::from_secs(30))
        .build()?;
    let mut futs = FuturesUnordered::new();
    let mut scheduled = BTreeSet::new();

    for art in &init.artifacts {
        let artifact_key = format!("{:?}:{}@{}", art.kind, art.name, art.version);
        if !scheduled.insert(artifact_key) {
            continue;
        }

        let pkg = art.name.clone();
        let ver = art.version.clone();
        let integrity = art.payload_integrity().to_owned();
        let url = art.presigned_url.clone();

        let runtime_type = art.runtime.as_ref().map(|r| r.r#type.clone());
        let runtime_version = art.runtime.as_ref().map(|r| r.version.clone());

        //  warn if runtime missing or version too low (non-fatal)
        if let Some(rt) = runtime_type.as_deref()
            && art.python_resolution.is_none()
        {
            warn_if_runtime_mismatch(&pkg, &ver, rt, runtime_version.as_deref(), quiet);
        }

        let cache_name = cache_filename(art);
        let cache_path = cache_dir.join(cache_name);
        let install_dir = resolved_package_dir(
            art.kind,
            install_roots.tools_dir,
            install_roots.agents_dir,
            install_roots.skills_dir,
            install_roots.knowledge_dir,
            install_roots.memory_dir,
            install_roots.profiles_dir,
            install_roots.loops_dir,
            &pkg,
            &ver,
        )?;

        // Short-circuit if cached and already extracted (unless refresh)
        let valid_cache = verify_sha256(&cache_path, &integrity).await.is_ok()
            && match art.size {
                Some(size) => fs::metadata(&cache_path)
                    .await
                    .is_ok_and(|m| m.len() == size),
                None => true,
            };
        if !refresh
            && valid_cache
            && dir_has_files(&install_dir).await?
            && local_selection_matches(&install_dir, art)?
        {
            continue;
        }

        let client_cl = client.clone();
        futs.push(async move {
            // (Re)download if missing or refresh requested
            if refresh || !valid_cache {
                let transfer = download_checked(&client_cl, &url, &cache_path, art.size, Some(&integrity)).await;
                if let Err(error) = transfer {
                    if error.downcast_ref::<ExpiredDownload>().is_some() {
                        let registry = registry.context("Download URL expired or access denied; retry agentpm install to obtain fresh URLs")?;
                        let renewed = registry.install_renew(init).await?;
                        let replacement = renewed.artifacts.iter().find(|a| a.kind == art.kind && a.name == pkg && a.version == ver)
                            .context("renewal omitted the selected artifact")?;
                        download_checked(&client_cl, &replacement.presigned_url, &cache_path, art.size, Some(&integrity)).await?;
                    } else { return Err(error.context(format!("downloading {pkg}"))); }
                }
            }

            // Verify integrity
            ensure_sha256(&cache_path, &integrity)
                .await
                .with_context(|| format!("integrity check failed for {}", pkg))?;

            // Extract
            extract_verified(&cache_path, &install_dir, Some(art), MAX_EXTRACTED_BYTES, MAX_EXTRACTED_ENTRIES)
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
    download_checked(client, url, dest, None, None).await
}

#[derive(Debug)]
struct ExpiredDownload;
impl std::fmt::Display for ExpiredDownload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Download URL expired or access denied; retry agentpm install for fresh URLs"
        )
    }
}
impl std::error::Error for ExpiredDownload {}

async fn download_checked(
    client: &Client,
    url: &str,
    dest: &Path,
    size: Option<u64>,
    digest: Option<&str>,
) -> Result<()> {
    download_with_timeout(client, url, dest, size, digest, Duration::from_secs(30)).await
}

async fn download_with_timeout(
    client: &Client,
    url: &str,
    dest: &Path,
    size: Option<u64>,
    digest: Option<&str>,
    idle: Duration,
) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).await?;
    }
    let tmp = dest.with_extension(format!("{:032x}.part", rand::random::<u128>()));
    let result = async {
        let mut resp = tokio::time::timeout(idle, client.get(url).send())
            .await
            .context("download made no progress for 30 seconds")?
            .map_err(reqwest::Error::without_url)?;
        if matches!(resp.status().as_u16(), 401 | 403) {
            return Err(ExpiredDownload.into());
        }
        resp = resp
            .error_for_status()
            .map_err(reqwest::Error::without_url)?;
        if let (Some(expected), Some(received)) = (size, resp.content_length()) {
            anyhow::ensure!(
                expected == received,
                "downloaded artifact size differs from signed inventory"
            );
        }
        let mut file = fs::File::create(&tmp).await?;
        let mut received = 0u64;
        while let Some(chunk) = tokio::time::timeout(idle, resp.chunk())
            .await
            .context("download made no progress for 30 seconds")?
            .map_err(reqwest::Error::without_url)?
        {
            received += chunk.len() as u64;
            anyhow::ensure!(
                size.is_none_or(|expected| received <= expected),
                "download exceeds signed artifact size"
            );
            file.write_all(&chunk).await?;
        }
        anyhow::ensure!(
            size.is_none_or(|expected| received == expected),
            "downloaded artifact size differs from signed inventory"
        );
        file.flush().await?;
        drop(file);
        if let Some(digest) = digest {
            ensure_sha256(&tmp, digest).await?;
        }
        fs::rename(&tmp, dest).await?;
        Ok(())
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(&tmp).await;
    }
    result
}

/// Compute and compare sha256 with a 64-char lowercase hex integrity.
/// Returns Ok(()) if matches, Err otherwise.
pub async fn ensure_sha256(path: &Path, integrity_hex: &str) -> Result<()> {
    let want = parse_integrity_hex(integrity_hex)?;
    let have = sha256_file(path).await?;
    if have == want {
        Ok(())
    } else {
        bail!("sha256 mismatch")
    }
}

/// Same as ensure_sha256 but used in a "can we skip?" path.
pub async fn verify_sha256(path: &Path, integrity_hex: &str) -> Result<()> {
    ensure_sha256(path, integrity_hex).await
}

fn parse_integrity_hex(integrity_hex: &str) -> Result<Vec<u8>> {
    let s = integrity_hex.trim();
    if s.len() != 64 {
        bail!("invalid sha256 hex length (expected 64 chars)");
    }
    let bytes = <Vec<u8>>::from_hex(s).map_err(|e| anyhow!("invalid sha256 hex: {}", e))?;
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
/// - Rejects unsupported links/types and limits decompression/entry counts.
pub async fn extract_tar_gz(tar_gz_path: &Path, dest_dir: &Path) -> Result<()> {
    extract_verified(
        tar_gz_path,
        dest_dir,
        None,
        MAX_EXTRACTED_BYTES,
        MAX_EXTRACTED_ENTRIES,
    )
    .await
}

const MAX_EXTRACTED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_EXTRACTED_ENTRIES: usize = 15_000;

async fn extract_verified(
    tar_gz_path: &Path,
    dest_dir: &Path,
    artifact: Option<&sdkm::PackageArtifact>,
    max_bytes: u64,
    max_entries: usize,
) -> Result<()> {
    let tar_gz_path = tar_gz_path.to_path_buf();
    let dest_dir = dest_dir.to_path_buf();
    let artifact = artifact.cloned();

    // Do the blocking tar I/O in a blocking thread
    task::spawn_blocking(move || {
        use std::io::Read;
        let parent = dest_dir
            .parent()
            .context("install directory has no parent")?;
        std::fs::create_dir_all(parent)?;
        let stage = parent.join(format!(".extract-{:032x}", rand::random::<u128>()));
        std::fs::create_dir(&stage)?;
        let result = (|| -> Result<()> {
            let file = std::fs::File::open(&tar_gz_path)?;
            let gz = flate2::read::GzDecoder::new(file);
            let mut archive = tar::Archive::new(gz.take(max_bytes + 1));
            let mut total = 0u64;

            for (index, entry) in archive.entries()?.enumerate() {
                anyhow::ensure!(index < max_entries, "archive exceeds entry-count limit");
                let mut entry = entry?;

                let entry_type = entry.header().entry_type();
                // Disallow links for safety in MVP
                anyhow::ensure!(
                    entry_type.is_file() || entry_type.is_dir(),
                    "archive contains unsupported symlink/hardlink or entry type"
                );
                total = total
                    .checked_add(entry.size())
                    .context("archive size overflow")?;
                anyhow::ensure!(
                    total <= max_bytes,
                    "archive exceeds decompressed byte limit"
                );

                let raw_path = entry.path()?;
                let safe_rel = sanitize_entry_path(&raw_path)?;
                let out_path = stage.join(&safe_rel);

                // Ensure parent exists
                if let Some(parent) = out_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                entry.unpack(&out_path)?;
            }
            let mut remainder = archive.into_inner();
            std::io::copy(&mut remainder, &mut std::io::sink())?;
            anyhow::ensure!(
                remainder.limit() > 0,
                "archive exceeds decompressed byte limit"
            );
            if let Some(artifact) = &artifact {
                validate_extracted(&stage, artifact)?;
            }
            let backup = parent.join(format!(".previous-{:032x}", rand::random::<u128>()));
            let previous = dest_dir.exists();
            if previous {
                std::fs::rename(&dest_dir, &backup)?;
            }
            if let Err(error) = std::fs::rename(&stage, &dest_dir) {
                if previous {
                    let _ = std::fs::rename(&backup, &dest_dir);
                }
                return Err(error.into());
            }
            if previous {
                std::fs::remove_dir_all(&backup)?;
            }
            if let Some(artifact) = &artifact
                && artifact.selected_artifact.is_some()
            {
                std::fs::write(selection_path(&dest_dir)?, selection_bytes(artifact)?)?;
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_dir_all(&stage);
        }
        result
    })
    .await?
}

fn selection_path(directory: &Path) -> Result<PathBuf> {
    Ok(directory
        .parent()
        .context("missing package parent")?
        .join(format!(
            ".{}.selection.json",
            directory
                .file_name()
                .context("missing version")?
                .to_string_lossy()
        )))
}

fn selection_bytes(artifact: &sdkm::PackageArtifact) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec(
        &serde_json::json!({"kind": artifact.kind, "name": artifact.name, "version": artifact.version,
        "release_integrity": artifact.integrity, "target": agentpm_sdk::target::Target::current()?, "artifact": artifact.selected_artifact}),
    )?)
}

fn local_selection_matches(directory: &Path, artifact: &sdkm::PackageArtifact) -> Result<bool> {
    if artifact.selected_artifact.is_none() {
        return Ok(true);
    }
    Ok(std::fs::read(selection_path(directory)?).ok().as_deref()
        == Some(selection_bytes(artifact)?.as_slice())
        && validate_extracted(directory, artifact).is_ok())
}

fn validate_extracted(directory: &Path, artifact: &sdkm::PackageArtifact) -> Result<()> {
    if let Some(selected) = &artifact.selected_artifact {
        let proof = artifact
            .verification
            .as_ref()
            .context("missing verified manifest")?;
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(directory.join("agent.json"))?)?;
        anyhow::ensure!(
            manifest == proof.manifest,
            "extracted manifest differs from verified release"
        );
        let descriptor: agentpm_sdk::release::ArtifactMetadata = serde_json::from_slice(
            &std::fs::read(directory.join(agentpm_sdk::release::ARTIFACT_METADATA_FILE))?,
        )?;
        anyhow::ensure!(
            descriptor.target == selected.target && descriptor.format == selected.format,
            "embedded artifact descriptor differs from selected artifact"
        );
    }
    Ok(())
}

pub(crate) fn validate_local_selection(directory: &Path) -> Result<()> {
    let state_path = selection_path(directory)?;
    let descriptor_path = directory.join(agentpm_sdk::release::ARTIFACT_METADATA_FILE);
    if !descriptor_path.exists() && !state_path.exists() {
        return Ok(());
    }
    let message =
        "installed artifact target/state changed; run agentpm install again on this machine";
    let state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(state_path).context(message)?)?;
    anyhow::ensure!(
        state["target"] == serde_json::to_value(agentpm_sdk::target::Target::current()?)?,
        "{message}"
    );
    let selected: agentpm_sdk::release::ReleaseArtifact =
        serde_json::from_value(state["artifact"].clone())?;
    let descriptor: agentpm_sdk::release::ArtifactMetadata =
        serde_json::from_slice(&std::fs::read(descriptor_path).context(message)?)?;
    anyhow::ensure!(
        selected.target == descriptor.target && selected.format == descriptor.format,
        "{message}"
    );
    Ok(())
}

/// Rejects absolute paths, Windows prefixes, and any `..` components.
/// Returns a cleaned relative path to be joined under the destination.
fn sanitize_entry_path(p: &Path) -> Result<PathBuf> {
    anyhow::ensure!(
        !p.to_string_lossy().contains(['\\', ':']),
        "tar entry contains a Windows path/prefix"
    );
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

/// Try to run `<cmd> --version` (and for python also `-V`) and return the raw version string.
fn get_cmd_version(cmd: &str) -> io::Result<Option<String>> {
    // Try `--version` first (works for node & modern python)
    let out = Command::new(cmd)
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    let ver_text = match out {
        Ok(o) => {
            if !o.stdout.is_empty() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else if !o.stderr.is_empty() {
                Some(String::from_utf8_lossy(&o.stderr).trim().to_string())
            } else {
                None
            }
        }
        Err(e) => {
            // If command not found, bubble up; otherwise try python's -V
            if e.kind() == io::ErrorKind::NotFound {
                return Err(e);
            }
            None
        }
    };

    if ver_text.is_some() {
        return Ok(ver_text);
    }

    // Fallback for older Python: `python -V` prints to stderr
    let out = Command::new(cmd)
        .arg("-V")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match out {
        Ok(o) => {
            if !o.stdout.is_empty() {
                Ok(Some(String::from_utf8_lossy(&o.stdout).trim().to_string()))
            } else if !o.stderr.is_empty() {
                Ok(Some(String::from_utf8_lossy(&o.stderr).trim().to_string()))
            } else {
                Ok(None)
            }
        }
        Err(e) => {
            if e.kind() == io::ErrorKind::NotFound {
                Err(e)
            } else {
                Ok(None)
            }
        }
    }
}

/// Very small parser: extract major(.minor) from a free-form version string.
/// Examples:
///   "v20.11.1" -> (20, Some(11))
///   "Python 3.11.7" -> (3, Some(11))
///   "20" -> (20, None)
fn parse_major_minor(text: &str) -> Option<(u64, Option<u64>)> {
    // Find first digit run
    let mut digits = String::new();
    let mut minor = String::new();
    let mut seen_dot = false;
    for c in text.chars() {
        if c.is_ascii_digit() {
            if !seen_dot {
                digits.push(c);
            } else {
                minor.push(c);
            }
        } else if c == '.' && !digits.is_empty() && !seen_dot {
            seen_dot = true;
        } else if !digits.is_empty() {
            break;
        }
    }
    if digits.is_empty() {
        return None;
    }
    let major: u64 = digits.parse().ok()?;
    let minor_num = if seen_dot && !minor.is_empty() {
        Some(minor.parse().ok()?)
    } else {
        None
    };
    Some((major, minor_num))
}

/// Compare installed vs requested (>= semantics).
/// If requested has only major, compare majors.
/// If requested has major.minor, compare lexicographically (major then minor).
fn is_version_sufficient(installed: &str, requested: &str) -> Option<bool> {
    let (i_maj, i_min) = parse_major_minor(installed)?;
    let (r_maj, r_min) = parse_major_minor(requested)?;

    if i_maj > r_maj {
        return Some(true);
    }
    if i_maj < r_maj {
        return Some(false);
    }
    // majors equal
    match r_min {
        None => Some(true), // only major required, equal majors OK
        Some(rm) => {
            let im = i_min.unwrap_or(0);
            Some(im >= rm)
        }
    }
}

/// Emit a warning (never errors) if runtime is missing or below requested version.
fn warn_if_runtime_mismatch(
    pkg: &str,
    ver: &str,
    rt_type: &str,
    rt_version: Option<&str>,
    quiet: bool,
) {
    let Some(candidates) = runtime_cmd_candidates(rt_type) else {
        eprintln!(
            "ℹ️  agentpm: {pkg}@{ver}: runtime \"{rt_type}\" is not one of the known types \
(node/nodejs/python/python3); skipping local availability check."
        );
        return;
    };

    let mut found_cmd: Option<(&str, String)> = None; // (cmd, version_text or "")

    for &cmd in &candidates {
        match get_cmd_version(cmd) {
            Ok(Some(ver)) => {
                found_cmd = Some((cmd, ver));
                break;
            }
            Ok(None) => {
                // Command exists but no version text; treat as found with unknown version
                found_cmd = Some((cmd, String::new()));
                break;
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                // try next alias
            }
            Err(_) => {
                // exists but couldn't run version; treat as found w/ unknown version
                found_cmd = Some((cmd, String::new()));
                break;
            }
        }
    }

    if found_cmd.is_none() {
        if !quiet {
            eprintln!(
                "⚠️  agentpm: {pkg}@{ver}: runtime \"{rt_type}\" not found on PATH (tried: {}). Installation will continue, but this tool may not run here.",
                candidates.join(", ")
            );
        }
        return;
    }

    if let Some(req) = rt_version {
        let (cmd, installed_text) = found_cmd.unwrap();
        if let Some(ok) = is_version_sufficient(&installed_text, req) {
            if !ok && !quiet {
                eprintln!(
                    "⚠️  agentpm: {pkg}@{ver}: runtime \"{rt_type}\" appears below requested version (found \"{}\" via `{}`, need >= {}). Continuing.",
                    installed_text, cmd, req
                );
            }
        } else if !quiet {
            // Couldn't parse version—still provide a soft heads-up
            if !installed_text.is_empty() {
                eprintln!(
                    "ℹ️  agentpm: {pkg}@{ver}: couldn't parse {rt_type} version from \"{}\" (via `{}`); requested >= {}. Continuing.",
                    installed_text, cmd, req
                );
            } else {
                eprintln!(
                    "ℹ️  agentpm: {pkg}@{ver}: {rt_type} detected via `{}` but version couldn't be determined; requested >= {}. Continuing.",
                    cmd, req
                );
            }
        }
    }
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
    let identity = serde_json::to_vec(&(
        art.kind,
        &art.name,
        &art.version,
        art.selected_artifact.as_ref().map(|a| a.target),
        art.payload_integrity(),
    ))
    .unwrap();
    format!("{:x}.tgz", Sha256::digest(identity))
}

/// Resolve to .agentpm/tools/<owner>/<name>/<version>
fn resolved_tool_dir(base: &Path, package: &str, version: &str) -> Result<PathBuf> {
    semver::Version::parse(version).context("invalid package version")?;
    let (owner, name) = split_package(package)?;
    Ok(base.join(owner).join(name).join(version))
}

fn resolved_agent_dir(base: &Path, package: &str, version: &str) -> Result<PathBuf> {
    semver::Version::parse(version).context("invalid package version")?;
    let (owner, name) = split_package(package)?;
    Ok(base.join(owner).join(name).join(version))
}

#[allow(clippy::too_many_arguments)]
fn resolved_package_dir(
    kind: sdkm::PackageKind,
    tools_base: &Path,
    agents_base: &Path,
    skills_base: &Path,
    knowledge_base: &Path,
    memory_base: &Path,
    profiles_base: &Path,
    loops_base: &Path,
    package: &str,
    version: &str,
) -> Result<PathBuf> {
    match kind {
        sdkm::PackageKind::Tool => resolved_tool_dir(tools_base, package, version),
        sdkm::PackageKind::Agent => resolved_agent_dir(agents_base, package, version),
        sdkm::PackageKind::Skill => resolved_agent_dir(skills_base, package, version),
        sdkm::PackageKind::Knowledge => resolved_agent_dir(knowledge_base, package, version),
        sdkm::PackageKind::Memory => resolved_agent_dir(memory_base, package, version),
        sdkm::PackageKind::Profile => resolved_agent_dir(profiles_base, package, version),
        sdkm::PackageKind::Loop => resolved_agent_dir(loops_base, package, version),
        sdkm::PackageKind::Template => {
            bail!("template packages are not installable with `agentpm install`; use `agentpm new`")
        }
    }
}

/// Split "@owner/name" into (owner, name)
fn split_package(package: &str) -> Result<(String, String)> {
    if !package.starts_with('@') {
        bail!("package must be of form @owner/name");
    }
    let mut parts = package[1..].splitn(2, '/');
    let owner = parts.next().ok_or_else(|| anyhow!("invalid package"))?;
    let name = parts.next().ok_or_else(|| anyhow!("invalid package"))?;
    for part in [owner, name] {
        anyhow::ensure!(
            !part.is_empty() && part != "." && part != ".." && !part.contains(['/', '\\', ':']),
            "invalid package path component"
        );
    }
    Ok((owner.to_string(), name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentpm_sdk::models::install::{PackageArtifact, PackageKind};

    #[test]
    fn resolves_tool_install_dir_under_tools_layout() {
        let path = resolved_package_dir(
            PackageKind::Tool,
            Path::new(".agentpm/tools"),
            Path::new(".agentpm/agents"),
            Path::new(".agentpm/skills"),
            Path::new(".agentpm/knowledge"),
            Path::new(".agentpm/memory"),
            Path::new(".agentpm/profiles"),
            Path::new(".agentpm/loops"),
            "@zack/capitalize",
            "0.1.0",
        )
        .unwrap();

        assert_eq!(path, PathBuf::from(".agentpm/tools/zack/capitalize/0.1.0"));
    }

    #[test]
    fn resolves_agent_install_dir_under_agents_layout() {
        let path = resolved_package_dir(
            PackageKind::Agent,
            Path::new(".agentpm/tools"),
            Path::new(".agentpm/agents"),
            Path::new(".agentpm/skills"),
            Path::new(".agentpm/knowledge"),
            Path::new(".agentpm/memory"),
            Path::new(".agentpm/profiles"),
            Path::new(".agentpm/loops"),
            "@zack/support-agent",
            "0.1.0",
        )
        .unwrap();

        assert_eq!(
            path,
            PathBuf::from(".agentpm/agents/zack/support-agent/0.1.0")
        );
    }

    #[test]
    fn resolves_skill_install_dir_under_skills_layout() {
        let path = resolved_package_dir(
            PackageKind::Skill,
            Path::new(".agentpm/tools"),
            Path::new(".agentpm/agents"),
            Path::new(".agentpm/skills"),
            Path::new(".agentpm/knowledge"),
            Path::new(".agentpm/memory"),
            Path::new(".agentpm/profiles"),
            Path::new(".agentpm/loops"),
            "@zack/triage-skill",
            "0.1.0",
        )
        .unwrap();

        assert_eq!(
            path,
            PathBuf::from(".agentpm/skills/zack/triage-skill/0.1.0")
        );
    }

    #[test]
    fn rejects_template_artifacts_in_normal_install_path() {
        let err = resolved_package_dir(
            PackageKind::Template,
            Path::new(".agentpm/tools"),
            Path::new(".agentpm/agents"),
            Path::new(".agentpm/skills"),
            Path::new(".agentpm/knowledge"),
            Path::new(".agentpm/memory"),
            Path::new(".agentpm/profiles"),
            Path::new(".agentpm/loops"),
            "@zack/research-template",
            "0.1.0",
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("template packages are not installable with"),
            "{err:#}"
        );
    }

    #[test]
    fn resolves_knowledge_install_dir_under_knowledge_layout() {
        let path = resolved_package_dir(
            PackageKind::Knowledge,
            Path::new(".agentpm/tools"),
            Path::new(".agentpm/agents"),
            Path::new(".agentpm/skills"),
            Path::new(".agentpm/knowledge"),
            Path::new(".agentpm/memory"),
            Path::new(".agentpm/profiles"),
            Path::new(".agentpm/loops"),
            "@zack/python-docs",
            "0.1.0",
        )
        .unwrap();

        assert_eq!(
            path,
            PathBuf::from(".agentpm/knowledge/zack/python-docs/0.1.0")
        );
    }

    #[test]
    fn resolves_memory_install_dir_under_memory_layout() {
        let path = resolved_package_dir(
            PackageKind::Memory,
            Path::new(".agentpm/tools"),
            Path::new(".agentpm/agents"),
            Path::new(".agentpm/skills"),
            Path::new(".agentpm/knowledge"),
            Path::new(".agentpm/memory"),
            Path::new(".agentpm/profiles"),
            Path::new(".agentpm/loops"),
            "@zack/support-memory",
            "0.1.0",
        )
        .unwrap();

        assert_eq!(
            path,
            PathBuf::from(".agentpm/memory/zack/support-memory/0.1.0")
        );
    }

    #[test]
    fn resolves_profile_install_dir_under_profiles_layout() {
        let path = resolved_package_dir(
            PackageKind::Profile,
            Path::new(".agentpm/tools"),
            Path::new(".agentpm/agents"),
            Path::new(".agentpm/skills"),
            Path::new(".agentpm/knowledge"),
            Path::new(".agentpm/memory"),
            Path::new(".agentpm/profiles"),
            Path::new(".agentpm/loops"),
            "@zack/support-persona",
            "0.1.0",
        )
        .unwrap();

        assert_eq!(
            path,
            PathBuf::from(".agentpm/profiles/zack/support-persona/0.1.0")
        );
    }

    #[test]
    fn resolves_loop_install_dir_under_loops_layout() {
        let path = resolved_package_dir(
            PackageKind::Loop,
            Path::new(".agentpm/tools"),
            Path::new(".agentpm/agents"),
            Path::new(".agentpm/skills"),
            Path::new(".agentpm/knowledge"),
            Path::new(".agentpm/memory"),
            Path::new(".agentpm/profiles"),
            Path::new(".agentpm/loops"),
            "@zack/incident-response-loop",
            "0.1.0",
        )
        .unwrap();

        assert_eq!(
            path,
            PathBuf::from(".agentpm/loops/zack/incident-response-loop/0.1.0")
        );
    }

    #[test]
    fn cache_filename_binds_kind_identity_and_digest() {
        let art = PackageArtifact {
            python_resolution: None,
            kind: PackageKind::Agent,
            name: "@zack/support-agent".to_string(),
            version: "0.1.0".to_string(),
            integrity: "abc".to_string(),
            presigned_url: "https://example.test".to_string(),
            size: None,
            content_type: None,
            signing: None,
            verification: None,
            runtime: None,
            selected_artifact: None,
        };

        let name = cache_filename(&art);
        assert_eq!(name.len(), 68);
        let mut changed = art.clone();
        changed.kind = PackageKind::Tool;
        assert_ne!(name, cache_filename(&changed));
        changed = art.clone();
        changed.integrity = "def".into();
        assert_ne!(name, cache_filename(&changed));
        changed = art.clone();
        changed.version = "0.2.0".into();
        assert_ne!(name, cache_filename(&changed));
        let mut left = art.clone();
        left.name = "@a-b/c".into();
        let mut right = art;
        right.name = "@a/b-c".into();
        assert_ne!(cache_filename(&left), cache_filename(&right));
    }
}
