//! Consumer-side replay of portable Python resolutions. No install-time solving.
use crate::{
    manifest::read_lock_or_default,
    python_resolver::{ManagedUv, UV_VERSION},
    runner::{ResolvedTool, RunOptions, RunnerManifest, resolve_interpreter_command},
    semver::types::{Lock, PackageKind, package_key},
};
use agentpm_sdk::{
    models::install::PackageArtifact,
    python::{
        MAX_RESOLUTION_METADATA_BYTES, PythonReleaseMetadata, PythonResolution, ResolutionInputs,
    },
    target::Target,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

pub(crate) const ERR_ENVIRONMENT: &str = "managed Python environment is missing or stale";

// -I ignores caller PYTHONPATH/user-site while retaining the venv's packages.
const INSPECT: &str = r#"
import json, os, platform, sys, sysconfig
implementation = sys.implementation.version
implementation_version = '.'.join(str(v) for v in implementation[:3])
if implementation.releaselevel != 'final':
    implementation_version += {'alpha':'a', 'beta':'b', 'candidate':'rc'}[implementation.releaselevel] + str(implementation.serial)
data = {
    "executable": os.path.abspath(sys.executable),
    "base_executable": os.path.realpath(getattr(sys, "_base_executable", sys.executable)),
    "prefix": os.path.realpath(sys.prefix),
    "base_prefix": os.path.realpath(sys.base_prefix),
    "abi": json.dumps({"soabi":sysconfig.get_config_var("SOABI"), "flags":getattr(sys,"abiflags",""), "gil_disabled":sysconfig.get_config_var("Py_GIL_DISABLED"), "extension":sysconfig.get_config_var("EXT_SUFFIX")}, sort_keys=True),
    "version": platform.python_version(),
    "markers": {
        "implementation_name": sys.implementation.name,
        "implementation_version": implementation_version,
        "os_name": os.name,
        "platform_machine": platform.machine(),
        "platform_python_implementation": platform.python_implementation(),
        "platform_release": platform.release(),
        "platform_system": platform.system(),
        "platform_version": platform.version(),
        "python_full_version": platform.python_version(),
        "python_version": '.'.join(platform.python_version_tuple()[:2]),
        "sys_platform": sys.platform,
    },
    "packages": [],
}
if len(sys.argv) > 1:
    import importlib.metadata, re
    data["packages"] = sorted([
        [re.sub(r"[-_.]+", "-", d.metadata["Name"]).lower(), d.version]
        for d in importlib.metadata.distributions()
    ])
print(json.dumps(data))
"#;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Interpreter {
    executable: String,
    base_executable: String,
    prefix: String,
    base_prefix: String,
    abi: String,
    version: String,
    markers: Value,
    packages: Vec<(String, String)>,
}

// Inspection is a live snapshot for marker evaluation and runtime validation.
// Only stable selection/ABI inputs belong in the directory and readiness key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct InterpreterIdentity {
    executable: String,
    implementation: String,
    python_version: String,
    abi: String,
}

impl Interpreter {
    fn identity(&self) -> Result<InterpreterIdentity> {
        let marker = |name| {
            self.markers[name]
                .as_str()
                .map(str::to_owned)
                .with_context(|| format!("missing Python interpreter marker {name}"))
        };
        Ok(InterpreterIdentity {
            executable: self.executable.clone(),
            implementation: marker("implementation_name")?,
            python_version: marker("python_version")?,
            abi: self.abi.clone(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Fingerprint {
    tool: String,
    version: String,
    target: String,
    resolution_digest: String,
    uv_version: String,
    interpreter: InterpreterIdentity,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadyState {
    version: u32,
    fingerprint: Fingerprint,
    packages: Vec<(String, String)>,
}

#[derive(Debug)]
pub(crate) struct ManagedPython {
    pub(crate) python: PathBuf,
    pub(crate) directory: PathBuf,
    // Hold the shared lock throughout Tool execution. A killed installer releases
    // its OS lock automatically, so its incomplete environment can be retried.
    _lock: fs::File,
}

struct ProvisionEngine {
    uv: ManagedUv,
    cache: PathBuf,
    index_arguments: Vec<String>,
}

fn inspect(command: &str, inventory: bool) -> Result<Interpreter> {
    let mut process = Command::new(command);
    process.args(["-I", "-c", INSPECT]);
    if inventory {
        process.arg("inventory");
    }
    let output = process.output().context("inspecting Python interpreter")?;
    ensure!(
        output.status.success(),
        "Python interpreter inspection failed for {command}"
    );
    let mut interpreter: Interpreter =
        serde_json::from_slice(&output.stdout).context("invalid Python interpreter inspection")?;
    // Use the host's path normalization for comparisons (notably Windows' \\?\
    // prefix). Preserve executable itself so the selected venv is still honored.
    for value in [
        &mut interpreter.base_executable,
        &mut interpreter.prefix,
        &mut interpreter.base_prefix,
    ] {
        *value = Path::new(value)
            .canonicalize()?
            .to_string_lossy()
            .into_owned();
    }
    Ok(interpreter)
}

fn has_dependencies(manifest: &RunnerManifest) -> bool {
    manifest.runtime.as_ref().is_some_and(|runtime| {
        runtime.runtime_type == "python"
            && runtime
                .dependencies
                .as_ref()
                .is_some_and(|requirements| !requirements.is_empty())
    })
}

fn read_metadata(tool_dir: &Path, manifest: &RunnerManifest) -> Result<PythonReleaseMetadata> {
    let path = tool_dir.join(agentpm_sdk::python::RESOLUTION_METADATA_FILE);
    let info = fs::symlink_metadata(&path)?;
    ensure!(
        info.is_file() && info.len() <= MAX_RESOLUTION_METADATA_BYTES as u64,
        "Python resolution metadata must be a bounded regular file"
    );
    let metadata: PythonReleaseMetadata = serde_json::from_slice(&fs::read(path)?)?;
    metadata.validate()?;
    let runtime = manifest
        .runtime
        .as_ref()
        .context("missing Python runtime")?;
    let inputs = ResolutionInputs::new(
        runtime.dependencies.as_deref().unwrap_or_default(),
        runtime
            .version
            .as_deref()
            .context("missing Python minimum version")?,
        UV_VERSION,
    )?;
    ensure!(
        inputs.matches(&metadata.python_resolution),
        "installed Tool manifest and Python resolution disagree"
    );
    Ok(metadata)
}

fn fingerprint(
    tool: &str,
    version: &str,
    resolution: &PythonResolution,
    interpreter: &Interpreter,
) -> Result<Fingerprint> {
    Ok(Fingerprint {
        tool: tool.into(),
        version: version.into(),
        target: Target::current_triple().into(),
        resolution_digest: resolution.digest()?,
        uv_version: UV_VERSION.into(),
        interpreter: interpreter.identity()?,
    })
}

fn directory(project: &Path, identity: &Fingerprint) -> Result<PathBuf> {
    let (namespace, name) = identity
        .tool
        .strip_prefix('@')
        .and_then(|s| s.split_once('/'))
        .context("invalid installed Tool identity")?;
    for component in [namespace, name, identity.target.as_str()] {
        ensure!(
            !component.is_empty()
                && component != "."
                && component != ".."
                && component
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)),
            "invalid Python environment path component"
        );
    }
    let version: semver::Version = identity.version.parse()?;
    let key = format!("{:x}", Sha256::digest(serde_json::to_vec(identity)?));
    Ok(project
        .join(".agentpm/runtimes/python")
        .join(namespace)
        .join(name)
        .join(version.to_string())
        .join(&identity.target)
        .join(key))
}

fn check_directory_chain(project: &Path, path: &Path) -> Result<()> {
    let relative = path.strip_prefix(project)?;
    let mut current = project.to_path_buf();
    for part in relative.components() {
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(info) => ensure!(
                info.is_dir() && !info.file_type().is_symlink(),
                "Python environment directory must not be a symlink: {}",
                current.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn python_path(directory: &Path) -> PathBuf {
    directory.join(if cfg!(windows) {
        "venv/Scripts/python.exe"
    } else {
        "venv/bin/python"
    })
}

fn expected_packages(
    resolution: &PythonResolution,
    interpreter: &Interpreter,
) -> Result<Vec<(String, String)>> {
    let mut values = resolution
        .packages_for_environment(&interpreter.markers)?
        .iter()
        .map(|p| (p.name.clone(), p.version.clone()))
        .collect::<Vec<_>>();
    values.sort();
    Ok(values)
}

fn inventory_matches(
    directory: &Path,
    source: &Interpreter,
    expected: &[(String, String)],
) -> Result<()> {
    let actual = inspect(&python_path(directory).to_string_lossy(), true)?;
    ensure!(
        Path::new(&actual.prefix) == directory.join("venv").canonicalize()?
            && actual.base_executable == source.base_executable
            && actual.base_prefix == source.base_prefix
            && actual.version == source.version
            && actual.abi == source.abi
            && actual.markers == source.markers,
        "managed environment interpreter no longer matches the selected Python"
    );
    ensure!(
        actual.packages == expected,
        "managed environment distribution set differs from the recorded Python resolution"
    );
    Ok(())
}

fn ready(
    directory: &Path,
    identity: &Fingerprint,
    source: &Interpreter,
    expected: &[(String, String)],
) -> Result<()> {
    let path = directory.join("ready.json");
    let info = fs::symlink_metadata(&path)?;
    ensure!(
        info.is_file() && info.len() <= MAX_RESOLUTION_METADATA_BYTES as u64,
        "invalid Python environment completion record"
    );
    let state: ReadyState = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(
        state.version == 1 && state.fingerprint == *identity && state.packages == expected,
        "Python environment fingerprint changed"
    );
    inventory_matches(directory, source, expected)
}

fn open_lock(directory: &Path, create: bool) -> Result<fs::File> {
    let path = directory.join("provision.lock");
    if path.symlink_metadata().is_ok() {
        ensure!(
            fs::symlink_metadata(&path)?.is_file(),
            "invalid Python provisioning lock"
        );
    }
    Ok(fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(create)
        .truncate(false)
        .open(path)?)
}

fn requirements(resolution: &PythonResolution, interpreter: &Interpreter) -> Result<String> {
    let mut text = String::new();
    for package in resolution.packages_for_environment(&interpreter.markers)? {
        text.push_str(&format!("{}=={}", package.name, package.version));
        for hash in &package.hashes {
            text.push_str(&format!(" --hash={hash}"));
        }
        text.push('\n');
    }
    Ok(text)
}

fn provisioning_failure(stderr: &[u8], identity: &Fingerprint) -> anyhow::Error {
    // This classification is coupled to the pinned uv's stderr wording. Pin
    // adoption must run the real-uv missing-wheel/hash/unavailable tests in the
    // Resolver Contract workflow; synthetic prose fixtures alone are insufficient.
    let text = String::from_utf8_lossy(stderr);
    let lower = text.to_ascii_lowercase();
    let category = if lower.contains("hash mismatch")
        || lower.contains("hashes do not match")
        || lower.contains("hashes mismatch")
        || lower.contains("hash checking")
    {
        "Python distribution digest mismatch; refusing installation"
    } else if lower.contains("no wheels")
        || lower.contains("no compatible wheel")
        || lower.contains("no usable wheels")
        || lower.contains("no binary distribution")
        || lower.contains("source distributions are disabled")
        || lower.contains("no source distribution or wheel")
    {
        "No compatible Python wheel for the locked distribution"
    } else if lower.contains("not found in the package registry")
        || lower.contains("no version")
        || lower.contains("was not found")
        || lower.contains("404 not found")
        || lower.contains("410 gone")
    {
        "Locked Python distribution is unavailable from the index"
    } else {
        "Python environment provisioning failed"
    };
    anyhow::anyhow!(
        "{category} for {}@{} on {} (Python {}): {}",
        identity.tool,
        identity.version,
        identity.target,
        identity.interpreter.python_version,
        text.trim()
    )
}

fn uv_command(uv: &ManagedUv, directory: &Path) -> Result<Command> {
    let mut command = uv.command()?;
    command
        .arg("--no-config")
        .current_dir(directory)
        .env_remove("VIRTUAL_ENV")
        .env_remove("CONDA_PREFIX")
        .env_remove("PYTHONHOME")
        .env_remove("PYTHONPATH");
    Ok(command)
}

fn provision_with(
    project: &Path,
    tool: &str,
    version: &str,
    command: &str,
    resolution: &PythonResolution,
    refresh: bool,
    acquire: impl FnOnce() -> Result<ProvisionEngine>,
) -> Result<PathBuf> {
    let interpreter = inspect(command, false)?;
    let identity = fingerprint(tool, version, resolution, &interpreter)?;
    let expected = expected_packages(resolution, &interpreter)?;
    let project = project.canonicalize()?;
    let directory = directory(&project, &identity)?;
    check_directory_chain(&project, python_path(&directory).parent().unwrap())?;
    fs::create_dir_all(&directory)?;
    let lock = open_lock(&directory, true)?;
    lock.try_lock().context(
        "Python environment is being provisioned or used; retry install after the Tool finishes",
    )?;
    let engine = acquire()?;
    let uv = &engine.uv;
    if !refresh && ready(&directory, &identity, &interpreter, &expected).is_ok() {
        return Ok(python_path(&directory));
    }
    let ready_path = directory.join("ready.json");
    match fs::remove_file(&ready_path) {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    let python = python_path(&directory);
    // Recreate the venv scaffolding after any incomplete attempt, including a
    // process killed during venv creation. Existing distributions are then synced.
    let output = uv_command(uv, &directory)?
        .args([
            "venv",
            "--no-project",
            "--allow-existing",
            "--no-python-downloads",
            "--no-managed-python",
            "--python",
        ])
        .arg(&identity.interpreter.executable)
        .arg("--cache-dir")
        .arg(&engine.cache)
        .arg(directory.join("venv"))
        .output()?;
    ensure!(
        output.status.success(),
        "could not create managed Python environment: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let requirements_path =
        directory.join(format!(".requirements-{:032x}.txt", rand::random::<u128>()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&requirements_path)?;
    file.write_all(requirements(resolution, &interpreter)?.as_bytes())?;
    drop(file);
    let output = uv_command(uv, &directory)?
        .args([
            "pip",
            "sync",
            "--require-hashes",
            "--only-binary",
            ":all:",
            "--no-python-downloads",
            "--no-managed-python",
            "--no-sources",
            "--allow-empty-requirements",
            "--link-mode",
            "copy",
            "--no-progress",
            "--python",
        ])
        .arg(&python)
        .arg("--cache-dir")
        .arg(&engine.cache)
        .args(&engine.index_arguments)
        .args(if refresh { vec!["--reinstall"] } else { vec![] })
        .arg(&requirements_path)
        .output();
    let _ = fs::remove_file(&requirements_path);
    let output = output.context("executing pinned Python provisioning engine")?;
    if !output.status.success() {
        return Err(provisioning_failure(&output.stderr, &identity));
    }
    let check = uv_command(uv, &directory)?
        .args(["pip", "check", "--python"])
        .arg(&python)
        .output()?;
    ensure!(
        check.status.success(),
        "locked Python set has missing or incompatible dependencies; no unrecorded packages were added: {}",
        String::from_utf8_lossy(&check.stderr)
    );
    inventory_matches(&directory, &interpreter, &expected)?;
    let state = ReadyState {
        version: 1,
        fingerprint: identity,
        packages: expected,
    };
    let temporary = directory.join(format!(".ready-{:032x}.json", rand::random::<u128>()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(&state)?)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temporary, ready_path)?;
    Ok(python)
}

pub(crate) fn provision_installed(
    project: &Path,
    tool_dir: &Path,
    artifact: &PackageArtifact,
    base_url: &str,
    refresh: bool,
) -> Result<()> {
    let Some(expected) = &artifact.python_resolution else {
        return Ok(());
    };
    let manifest: RunnerManifest = serde_json::from_slice(&fs::read(tool_dir.join("agent.json"))?)?;
    ensure!(
        manifest.kind == "tool" && has_dependencies(&manifest),
        "Python resolution requires a dependency-bearing Python Tool"
    );
    ensure!(
        manifest.version == artifact.version
            && (manifest.name == artifact.name
                || artifact.name.rsplit('/').next() == Some(manifest.name.as_str())),
        "installed Tool identity differs from the install plan"
    );
    let metadata = read_metadata(tool_dir, &manifest)?;
    ensure!(
        metadata.python_resolution == *expected,
        "artifact Python resolution differs from the Registry plan"
    );
    let mut vendored = false;
    let mut entry_limit = false;
    let mut depth_limit = false;
    let mut unreadable = 0;
    for (index, entry) in walkdir::WalkDir::new(tool_dir)
        .max_depth(4)
        .into_iter()
        .enumerate()
    {
        if index == 2000 {
            entry_limit = true;
            break;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                unreadable += 1;
                continue;
            }
        };
        if matches!(
            entry.file_name().to_str(),
            Some("_vendor" | "site-packages")
        ) || entry.file_name().to_string_lossy().ends_with(".dist-info")
        {
            vendored = true;
            break;
        }
        depth_limit |= entry.depth() == 4 && entry.file_type().is_dir();
    }
    if vendored {
        eprintln!(
            "Warning: {} declares Python dependencies and includes vendored dependency content; verify portability and avoid conflicting package versions.",
            artifact.name
        );
    } else if entry_limit || depth_limit || unreadable > 0 {
        let mut skipped = Vec::new();
        if entry_limit {
            skipped.push("entries beyond the 2000-entry limit".to_owned());
        }
        if depth_limit {
            skipped.push("directory contents below depth 4".to_owned());
        }
        if unreadable > 0 {
            skipped.push(format!("{unreadable} unreadable entries"));
        }
        eprintln!(
            "Warning: {} vendored-content scan is incomplete; skipped {}. Vendored dependencies may be present; verify portability and avoid conflicting package versions.",
            artifact.name,
            skipped.join(", ")
        );
    }
    let command = resolve_interpreter_command(
        &manifest.entrypoint.command,
        manifest.runtime.as_ref(),
        &HashMap::new(),
    )?;
    provision_with(
        project,
        &artifact.name,
        &artifact.version,
        &command,
        expected,
        refresh,
        || {
            Ok(ProvisionEngine {
                uv: ManagedUv::acquire(base_url)?,
                cache: crate::config::project_dirs()
                    .context("cannot locate Python distribution cache")?
                    .cache_dir()
                    .join("python-index"),
                index_arguments: Vec::new(),
            })
        },
    )?;
    Ok(())
}

pub(crate) fn for_run(
    project: &Path,
    resolved: &ResolvedTool,
    command: &str,
) -> Result<Option<ManagedPython>> {
    if !has_dependencies(&resolved.manifest) {
        return Ok(None);
    }
    (|| -> Result<ManagedPython> {
        let lock = read_lock_or_default(project)?;
        let metadata = read_metadata(&resolved.tool_dir, &resolved.manifest)?;
        if let Lock::V2(value) = lock
            && let Some(package) = value.packages.get(&package_key(PackageKind::Tool, &resolved.package, &resolved.version.to_string())) {
            ensure!(package.python_resolution.as_ref() == Some(&metadata.python_resolution), "installed Python metadata differs from agent.lock");
        }
        let interpreter = inspect(command, false)?;
        let identity = fingerprint(&resolved.package, &resolved.version.to_string(), &metadata.python_resolution, &interpreter)?;
        let expected = expected_packages(&metadata.python_resolution, &interpreter)?;
        let project = project.canonicalize()?;
        let directory = directory(&project, &identity)?;
        check_directory_chain(&project, python_path(&directory).parent().unwrap())?;
        let lock = open_lock(&directory, false)?;
        lock.try_lock_shared().context("Python environment provisioning is in progress")?;
        ready(&directory, &identity, &interpreter, &expected)?;
        Ok(ManagedPython { python: python_path(&directory), directory: directory.join("venv"), _lock: lock })
    })().map(Some).with_context(|| format!("{ERR_ENVIRONMENT} for {}@{}; run `agentpm install` with the selected AGENTPM_PYTHON to provision it (use --refresh to repair damaged packages)", resolved.package, resolved.version))
}

pub(crate) fn check_tool_ready(project: &Path, name: &str, version: &str) -> Result<()> {
    let spec = crate::runner::parse_tool_spec(&format!("{name}@{version}"))?;
    let resolved = crate::runner::resolve_installed_tool(project, &spec)?;
    let command = resolve_interpreter_command(
        &resolved.manifest.entrypoint.command,
        resolved.manifest.runtime.as_ref(),
        &RunOptions::default().env_overrides,
    )?;
    for_run(project, &resolved, &command)?;
    Ok(())
}

#[cfg(test)]
mod tests;
