//! Portable author resolution, committed local Tool lock state and managed resolver acquisition.
use agentpm_sdk::{
    python::{PythonPackage, PythonResolution, ResolutionInputs},
    target::Target,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

pub const UV_VERSION: &str = env!("AGENTPM_UV_VERSION");
const MAX_BINARY_BYTES: u64 = 200 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct PinTable {
    version: String,
    artifacts: BTreeMap<String, Pin>,
}
#[derive(Debug, Deserialize)]
struct Pin {
    sha256: String,
}

#[derive(Debug)]
pub(crate) struct ManagedUv {
    path: PathBuf,
    digest: Option<String>,
}

fn check_version(path: &Path) -> Result<()> {
    let output = Command::new(path)
        .arg("--version")
        .output()
        .context("Python resolver could not execute; check AGENTPM_UV_PATH or the managed cache")?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut words = text.split_whitespace();
    ensure!(
        output.status.success() && words.next() == Some("uv") && words.next() == Some(UV_VERSION),
        "Python resolver version mismatch: AgentPM requires uv {UV_VERSION}; provision that exact version with AGENTPM_UV_PATH"
    );
    Ok(())
}

fn binary_digest(path: &Path) -> Result<String> {
    ensure!(
        fs::symlink_metadata(path)?.file_type().is_file(),
        "managed uv must be a regular file"
    );
    ensure!(
        fs::metadata(path)?.len() <= MAX_BINARY_BYTES,
        "managed uv exceeds the binary size limit"
    );
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    std::io::copy(&mut file, &mut digest)?;
    Ok(format!("{:x}", digest.finalize()))
}

fn resolver_response(
    response: reqwest::Result<reqwest::blocking::Response>,
    location: &str,
    target: &str,
) -> Result<reqwest::blocking::Response> {
    let response = response.map_err(|error| {
        anyhow::anyhow!(
            "Python resolver download network/proxy failure reaching AgentPM {location}: {}",
            error.without_url()
        )
    })?;
    let status = response.status();
    if matches!(status.as_u16(), 403 | 404) {
        anyhow::bail!(
            "Python resolver version/target is not mirrored or is inaccessible: uv {UV_VERSION} for {target} (HTTP {} from {location}). Check API resolver-route deployment, the deployed resolver inventory, mirrored object and storage permissions.",
            status.as_u16()
        );
    }
    ensure!(
        status.is_success(),
        "Python resolver storage HTTP failure: HTTP {} from {location} for uv {UV_VERSION}, {target}; check registry/storage service availability",
        status.as_u16()
    );
    Ok(response)
}

impl ManagedUv {
    pub(crate) fn acquire(base_url: &str) -> Result<Self> {
        let cache = crate::config::project_dirs()
            .context("cannot locate per-user resolver cache")?
            .cache_dir()
            .to_path_buf();
        Self::acquire_at(
            base_url,
            &cache,
            Target::current_triple(),
            std::env::var_os("AGENTPM_UV_PATH").map(PathBuf::from),
        )
    }

    fn acquire_at(
        base_url: &str,
        cache: &Path,
        target: &str,
        override_path: Option<PathBuf>,
    ) -> Result<Self> {
        if let Some(path) = override_path {
            let path = path.canonicalize().context(
                "AGENTPM_UV_PATH must name an existing uv binary file; PATH lookup is not used",
            )?;
            check_version(&path)?;
            return Ok(Self { path, digest: None });
        }
        let table: PinTable =
            serde_json::from_str(include_str!("../../../platforms/uv-pins.json"))?;
        Self::acquire_pinned(base_url, cache, target, &table)
    }

    fn acquire_pinned(
        base_url: &str,
        cache: &Path,
        target: &str,
        table: &PinTable,
    ) -> Result<Self> {
        ensure!(
            table.version == UV_VERSION,
            "compiled uv version and pin table disagree"
        );
        let pin = table.artifacts.get(target).with_context(|| format!(
            "AgentPM cannot provision its managed Python resolver for {target}. Supported resolver targets: {}. Provide compatible uv {UV_VERSION} with AGENTPM_UV_PATH; no network was attempted.",
            table.artifacts.keys().cloned().collect::<Vec<_>>().join(", ")))?;
        let filename = if target.ends_with("windows-msvc") {
            "uv.exe"
        } else {
            "uv"
        };
        let directory = cache.join("runtimes/uv").join(UV_VERSION).join(target);
        let path = directory.join(filename);
        if path.try_exists()? {
            ensure!(
                binary_digest(&path)? == pin.sha256,
                "cached Python resolver digest mismatch; remove the corrupted managed uv and retry"
            );
            check_version(&path)?;
            return Ok(Self {
                path,
                digest: Some(pin.sha256.clone()),
            });
        }
        // This required download honors standard proxy settings. A total deadline also bounds idle reads.
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(120))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()?;
        #[derive(Deserialize)]
        struct Download {
            presigned_url: String,
        }
        let endpoint = format!(
            "{}/v1/tools/install/resolvers/uv/{UV_VERSION}/{target}",
            base_url.trim_end_matches('/')
        );
        let download: Download =
            resolver_response(client.get(endpoint).send(), "registry storage", target)?
                .json()
                .map_err(|error| {
                    anyhow::anyhow!(
                        "invalid AgentPM resolver download response: {}",
                        error.without_url()
                    )
                })?;
        let url: reqwest::Url = download.presigned_url.parse()?;
        ensure!(
            url.scheme() == "https"
                || (url.scheme() == "http"
                    && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "::1"))),
            "resolver download requires HTTPS"
        );
        let response = resolver_response(client.get(url).send(), "storage", target)?;
        let mut bytes = Vec::new();
        response
            .take(MAX_BINARY_BYTES + 1)
            .read_to_end(&mut bytes)
            .context("Python resolver download interrupted")?;
        ensure!(
            bytes.len() as u64 <= MAX_BINARY_BYTES,
            "Python resolver binary is too large"
        );
        ensure!(
            format!("{:x}", Sha256::digest(&bytes)) == pin.sha256,
            "downloaded Python resolver digest mismatch; refusing to cache or execute it"
        );
        fs::create_dir_all(&directory)?;
        let temporary = directory.join(format!(".{:032x}-{filename}", rand::random::<u128>()));
        let result = (|| -> Result<()> {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            // Linux rejects executing a file that is still open for writing (ETXTBSY).
            drop(file);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&temporary, fs::Permissions::from_mode(0o755))?;
            }
            check_version(&temporary)?;
            match fs::hard_link(&temporary, &path) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => ensure!(
                    binary_digest(&path)? == pin.sha256,
                    "concurrent managed resolver digest mismatch"
                ),
                Err(error) => return Err(error.into()),
            }
            Ok(())
        })();
        let _ = fs::remove_file(&temporary);
        result?;
        Ok(Self {
            path,
            digest: Some(pin.sha256.clone()),
        })
    }

    /// Obtain a freshly verified command for one resolver/provisioning execution.
    pub(crate) fn command(&self) -> Result<Command> {
        if let Some(digest) = &self.digest {
            ensure!(
                binary_digest(&self.path)? == *digest,
                "managed Python resolver digest mismatch before execution"
            );
        }
        check_version(&self.path)?;
        let mut command = Command::new(&self.path);
        // Do not let a caller's uv project/settings change the declared AgentPM resolution inputs.
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("UV_")
                || key.to_string_lossy().starts_with("AGENTPM_")
            {
                command.env_remove(key);
            }
        }
        Ok(command)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct PyLock {
    lock_version: String,
    #[serde(default)]
    packages: Vec<PyPackage>,
}
#[derive(Deserialize)]
struct PyPackage {
    name: String,
    version: Option<String>,
    marker: Option<String>,
    #[serde(default)]
    wheels: Vec<Distribution>,
    sdist: Option<Distribution>,
    directory: Option<toml::Value>,
    vcs: Option<toml::Value>,
    archive: Option<toml::Value>,
}
#[derive(Deserialize)]
struct Distribution {
    hashes: BTreeMap<String, String>,
}

pub(crate) fn from_pylock(raw: &str, inputs: &ResolutionInputs) -> Result<PythonResolution> {
    let lock: PyLock = toml::from_str(raw).context("invalid structured uv pylock output")?;
    ensure!(lock.lock_version == "1.0", "unsupported uv pylock version");
    let packages = lock
        .packages
        .into_iter()
        .map(|package| {
            ensure!(
                package.directory.is_none() && package.vcs.is_none() && package.archive.is_none(),
                "Python resolution must contain index distributions, not local/URL sources"
            );
            let mut hashes = Vec::new();
            for distribution in package.wheels.into_iter().chain(package.sdist) {
                hashes.push(format!(
                    "sha256:{}",
                    distribution
                        .hashes
                        .get("sha256")
                        .context("resolved distribution has no retrievable SHA-256 digest")?
                ));
            }
            Ok(PythonPackage {
                name: package.name,
                version: package
                    .version
                    .context("resolved distribution has no exact version")?,
                hashes,
                marker: package.marker,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    PythonResolution::new(
        inputs.python_requires.clone(),
        inputs.requirements.clone(),
        packages,
    )
}

fn resolve(uv: &ManagedUv, inputs: &ResolutionInputs) -> Result<PythonResolution> {
    let directory = std::env::temp_dir().join(format!(
        "agentpm-python-resolution-{:032x}",
        rand::random::<u128>()
    ));
    fs::create_dir(&directory)?;
    let result = (|| -> Result<PythonResolution> {
        #[derive(Serialize)]
        struct Project<'a> {
            project: Metadata<'a>,
            tool: ToolConfig,
        }
        #[derive(Serialize)]
        struct ToolConfig {
            uv: UvConfig,
        }
        #[derive(Serialize)]
        #[serde(rename_all = "kebab-case")]
        struct Metadata<'a> {
            name: &'a str,
            version: &'a str,
            requires_python: &'a str,
            dependencies: &'a [String],
        }
        #[derive(Serialize)]
        struct UvConfig {
            environments: Vec<String>,
        }
        let environments = Target::SUPPORTED
            .iter()
            .map(|t| t.marker().to_owned())
            .collect();
        let project = Project {
            project: Metadata {
                name: "agentpm-resolution-input",
                version: "0.0.0",
                requires_python: &inputs.python_requires,
                dependencies: &inputs.requirements,
            },
            tool: ToolConfig {
                uv: UvConfig { environments },
            },
        };
        fs::write(directory.join("pyproject.toml"), toml::to_string(&project)?)?;
        let cache = crate::config::project_dirs()
            .context("cannot locate per-user Python index cache")?
            .cache_dir()
            .join("python-index");
        // Lock without executing source builds; export the exact fresh lock without that
        // install-policy filter so source-distribution digests remain in the portable record.
        let commands: Vec<Vec<String>> = serde_json::from_str(include_str!(
            "../../../platforms/uv-resolution-commands.json"
        ))?;
        for arguments in commands {
            let output = uv
                .command()?
                .current_dir(&directory)
                .env("HOME", &directory)
                .env("USERPROFILE", &directory)
                .env("APPDATA", &directory)
                .env("XDG_CONFIG_HOME", &directory)
                .env("UV_NO_SYSTEM_CONFIG", "1")
                .env("UV_CACHE_DIR", &cache)
                .args(arguments)
                .output()
                .context("Python resolver executed but failed to start")?;
            ensure!(
                output.status.success(),
                "Python resolver executed but failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        from_pylock(&fs::read_to_string(directory.join("pylock.toml"))?, inputs)
    })();
    let _ = fs::remove_dir_all(&directory);
    result
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorResolution {
    #[serde(default)]
    input_fingerprint: Option<String>,
    resolution_digest: String,
    resolution: PythonResolution,
}

pub(crate) fn prepare(
    manifest: &serde_json::Value,
    directory: &Path,
    base_url: &str,
    refresh: bool,
) -> Result<Option<PythonResolution>> {
    let cache_dir = crate::config::project_dirs()
        .context("cannot locate Python resolution cache")?
        .cache_dir()
        .join("python-resolutions");
    prepare_with_cache(manifest, directory, &cache_dir, refresh, || {
        ManagedUv::acquire(base_url)
    })
}

#[cfg(test)]
fn prepare_with(
    manifest: &serde_json::Value,
    directory: &Path,
    refresh: bool,
    acquire: impl FnOnce() -> Result<ManagedUv>,
) -> Result<Option<PythonResolution>> {
    prepare_with_cache(
        manifest,
        directory,
        &directory.join("cache/python-resolutions"),
        refresh,
        acquire,
    )
}

fn prepare_with_cache(
    manifest: &serde_json::Value,
    directory: &Path,
    cache_dir: &Path,
    refresh: bool,
    acquire: impl FnOnce() -> Result<ManagedUv>,
) -> Result<Option<PythonResolution>> {
    if manifest.get("kind").and_then(serde_json::Value::as_str) != Some("tool") {
        return Ok(None);
    }
    let mut lock = crate::manifest::read_lock_or_default(directory)?;
    let Some(runtime) = manifest.get("runtime") else {
        return Ok(None);
    };
    let Some(roots) = runtime.get("dependencies") else {
        return Ok(None);
    };
    ensure!(
        runtime.get("type").and_then(serde_json::Value::as_str) == Some("python"),
        "runtime.dependencies is Python-only"
    );
    let requirements: Vec<String> = serde_json::from_value(roots.clone())?;
    if requirements.is_empty() {
        return Ok(None);
    }
    ensure!(
        !lock.is_v1(),
        "Preparing Python dependencies requires a modern agent.lock; run agentpm install to migrate the legacy lock first; no resolver acquisition was attempted"
    );
    let version = runtime
        .get("version")
        .and_then(serde_json::Value::as_str)
        .context("Python runtime.version is required")?;
    let inputs = ResolutionInputs::new(&requirements, version, UV_VERSION)?;
    let fingerprint = inputs.fingerprint()?;
    if !refresh && let Some(resolution) = current_locked_resolution(manifest, &lock, &inputs)? {
        return Ok(Some(resolution));
    }
    // This is disposable acceleration only; a committed lock never needs this cache.
    let path = cache_dir.join(format!("{fingerprint}.json"));
    if !refresh && path.try_exists()? {
        let cached: AuthorResolution = serde_json::from_slice(&fs::read(&path)?)
            .context("invalid cached Python resolution; use --refresh-python")?;
        ensure!(
            cached.resolution_digest == cached.resolution.digest()?,
            "corrupted Python resolution; use --refresh-python"
        );
        if cached.input_fingerprint.as_deref() == Some(&fingerprint)
            && inputs.matches(&cached.resolution)
        {
            save_author_resolution(
                manifest,
                directory,
                &mut lock,
                &cached.resolution,
                &fingerprint,
            )?;
            return Ok(Some(cached.resolution));
        }
    }
    let uv = acquire()?;
    let resolution = resolve(&uv, &inputs)?;
    let record = AuthorResolution {
        input_fingerprint: Some(fingerprint),
        resolution_digest: resolution.digest()?,
        resolution: resolution.clone(),
    };
    fs::create_dir_all(cache_dir)?;
    let temporary = cache_dir.join(format!(".{:032x}.tmp", rand::random::<u128>()));
    fs::write(&temporary, serde_json::to_vec_pretty(&record)?)?;
    fs::rename(&temporary, &path)?;
    save_author_resolution(
        manifest,
        directory,
        &mut lock,
        &resolution,
        record.input_fingerprint.as_deref().unwrap(),
    )?;
    Ok(Some(resolution))
}

fn current_locked_resolution(
    manifest: &serde_json::Value,
    lock: &crate::semver::types::Lock,
    inputs: &ResolutionInputs,
) -> Result<Option<PythonResolution>> {
    let crate::semver::types::Lock::V2(lock) = lock else {
        return Ok(None);
    };
    let Some(root) = lock.roots.get("local:tool") else {
        return Ok(None);
    };
    if root.name.as_deref() != manifest.get("name").and_then(serde_json::Value::as_str)
        || root.version.as_deref() != manifest.get("version").and_then(serde_json::Value::as_str)
    {
        return Ok(None);
    }
    if root.python_input_fingerprint.as_deref() != Some(inputs.fingerprint()?.as_str()) {
        return Ok(None);
    }
    Ok(root
        .python_resolution
        .as_ref()
        .filter(|r| inputs.matches(r))
        .cloned())
}

fn save_author_resolution(
    manifest: &serde_json::Value,
    directory: &Path,
    lock: &mut crate::semver::types::Lock,
    resolution: &PythonResolution,
    fingerprint: &str,
) -> Result<()> {
    use crate::semver::types::{Lock, LockedRoot, minimum_lock_version};
    if lock.is_v1() {
        anyhow::bail!(
            "Preparing Python dependencies requires a modern agent.lock; run agentpm install to migrate the legacy lock first"
        );
    }
    let Lock::V2(value) = lock else {
        unreachable!()
    };
    let name = manifest
        .get("name")
        .and_then(serde_json::Value::as_str)
        .context("Tool name is required")?;
    let version = manifest
        .get("version")
        .and_then(serde_json::Value::as_str)
        .context("Tool version is required")?;
    if value
        .roots
        .get("local:agent")
        .is_some_and(|r| r.name.as_deref() == Some(name) && r.version.as_deref() == Some(version))
    {
        value.roots.remove("local:agent");
    }
    value.roots.insert(
        "local:tool".into(),
        LockedRoot {
            name: Some(name.into()),
            version: Some(version.into()),
            python_resolution: Some(resolution.clone()),
            python_input_fingerprint: Some(fingerprint.into()),
            ..Default::default()
        },
    );
    value.lockfile_version = value
        .lockfile_version
        .max(minimum_lock_version(&value.packages, &value.roots));
    value.generated = chrono::Utc::now();
    crate::manifest::write_lock(directory, lock)
}

/// Live publication must have reviewed, current portable state before any resolver acquisition.
pub(crate) fn require_publish_resolution(
    manifest: &serde_json::Value,
    directory: &Path,
) -> Result<()> {
    if manifest.get("kind").and_then(serde_json::Value::as_str) != Some("tool") {
        return Ok(());
    }
    let lock = crate::manifest::read_lock_or_default(directory)?;
    let Some(roots) = manifest.pointer("/runtime/dependencies") else {
        return Ok(());
    };
    let requirements: Vec<String> = serde_json::from_value(roots.clone())?;
    if requirements.is_empty() {
        return Ok(());
    }
    let version = manifest
        .pointer("/runtime/version")
        .and_then(serde_json::Value::as_str)
        .context("Python runtime.version is required")?;
    let inputs = ResolutionInputs::new(&requirements, version, UV_VERSION)?;
    ensure!(
        current_locked_resolution(manifest, &lock, &inputs)?.is_some(),
        "Python resolution in agent.lock is missing or stale. Run agentpm publish --dry-run --refresh-python, review and commit agent.lock, then publish again; no resolver acquisition or upload was attempted."
    );
    Ok(())
}

#[cfg(test)]
mod tests;
