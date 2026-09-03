use crate::manifest::{Entrypoint, read_lock_or_default};
use crate::runtime_version::{extract_runtime_version, parse_runtime_version};
use anyhow::{Context, Result, anyhow, bail};
use jsonschema::{Draft, JSONSchema};
use semver::{Version, VersionReq};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
#[cfg(unix)]
use std::sync::Mutex;
#[cfg(unix)]
use std::sync::atomic::AtomicI32;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[allow(dead_code)]
const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 1024 * 1024;
pub(crate) const ERR_LOCKFILE_MISSING: &str = "agent.lock not found";
pub(crate) const ERR_INSTALLED_TOOL_NOT_FOUND: &str = "installed tool not found";
pub(crate) const ERR_INSTALLED_TOOL_DIR_MISSING: &str = "installed tool directory does not exist";
pub(crate) const ERR_LOCKFILE_DEPENDENCY_MISSING: &str = "not found in agent.lock";
pub(crate) const ERR_NO_INSTALLED_VERSIONS_FOUND: &str = "no installed versions found";
pub(crate) const ERR_NO_INSTALLED_VERSION_SATISFIES: &str = "no installed version of ";
const ERR_REQUIRED_ENV_MISSING: &str = "missing required environment variables";
const ERR_RUNTIME_TYPE_MISMATCH: &str = "runtime/type mismatch";
const ERR_INTERPRETER_UNSUPPORTED: &str = "unsupported interpreter override or command";
const ERR_INTERPRETER_OVERRIDE_MISMATCH: &str = "interpreter override mismatch";
const ERR_INTERPRETER_NOT_EXECUTABLE: &str = "interpreter not found or not executable";
const ERR_INTERPRETER_HEALTHCHECK_FAILED: &str = "interpreter command failed health check";
const ERR_RUNTIME_VERSION_UNSATISFIED: &str = "runtime minimum version not satisfied";
const ERR_RUNTIME_VERSION_UNREADABLE: &str = "runtime version could not be read";
const ERR_TOOL_INPUT_SCHEMA_INVALID: &str = "tool input failed schema validation";
const ERR_TOOL_INPUT_SCHEMA_MALFORMED: &str = "tool input schema is invalid";
const ERR_TOOL_OUTPUT_SCHEMA_INVALID: &str = "tool output failed schema validation";
const ERR_TOOL_OUTPUT_SCHEMA_MALFORMED: &str = "tool output schema is invalid";
const ERR_TOOL_OUTPUT_LIMIT_EXCEEDED: &str = "tool output exceeded limit";
const ERR_TOOL_TIMED_OUT: &str = "tool execution timed out";
const ERR_TOOL_OUTPUT_NOT_JSON: &str = "tool stdout was not valid JSON";
const ERR_TOOL_EXITED_UNSUCCESSFULLY: &str = "tool exited unsuccessfully";
const ERR_ENTRYPOINT_CWD_MISSING: &str = "entrypoint cwd does not exist";

#[cfg(unix)]
const ACTIVE_CHILD_GROUP_SLOT_COUNT: usize = 1024;
#[cfg(unix)]
static ACTIVE_CHILD_PGIDS: [AtomicI32; ACTIVE_CHILD_GROUP_SLOT_COUNT] =
    [const { AtomicI32::new(0) }; ACTIVE_CHILD_GROUP_SLOT_COUNT];
#[cfg(unix)]
static SIGNAL_HANDLER_STATE: Mutex<SignalHandlerState> = Mutex::new(SignalHandlerState {
    active_guards: 0,
    previous_sigint: None,
    previous_sigterm: None,
});

#[cfg(unix)]
struct SignalHandlerState {
    active_guards: usize,
    previous_sigint: Option<libc::sighandler_t>,
    previous_sigterm: Option<libc::sighandler_t>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunnerErrorKind {
    Resolution,
    Runtime,
    Schema,
    Timeout,
    OutputLimit,
    MalformedOutput,
    SubprocessFailure,
    Other,
}

pub(crate) fn classify_runner_error(err: &anyhow::Error) -> RunnerErrorKind {
    if let Some(classified) = err
        .chain()
        .find_map(|cause| cause.downcast_ref::<ClassifiedRunnerError>())
    {
        return classified.kind;
    }

    let message = format!("{err:#}");
    if message.contains(ERR_TOOL_TIMED_OUT) {
        RunnerErrorKind::Timeout
    } else if message.contains(ERR_TOOL_OUTPUT_LIMIT_EXCEEDED) {
        RunnerErrorKind::OutputLimit
    } else if message.contains(ERR_TOOL_OUTPUT_NOT_JSON) {
        RunnerErrorKind::MalformedOutput
    } else if message.contains(ERR_REQUIRED_ENV_MISSING)
        || message.contains(ERR_RUNTIME_TYPE_MISMATCH)
        || message.contains(ERR_INTERPRETER_UNSUPPORTED)
        || message.contains(ERR_INTERPRETER_OVERRIDE_MISMATCH)
        || message.contains(ERR_INTERPRETER_NOT_EXECUTABLE)
        || message.contains(ERR_INTERPRETER_HEALTHCHECK_FAILED)
        || message.contains(ERR_RUNTIME_VERSION_UNSATISFIED)
        || message.contains(ERR_RUNTIME_VERSION_UNREADABLE)
    {
        RunnerErrorKind::Runtime
    } else if message.contains(ERR_TOOL_INPUT_SCHEMA_INVALID)
        || message.contains(ERR_TOOL_INPUT_SCHEMA_MALFORMED)
        || message.contains(ERR_TOOL_OUTPUT_SCHEMA_MALFORMED)
    {
        RunnerErrorKind::Schema
    } else if message.contains(ERR_INSTALLED_TOOL_NOT_FOUND)
        || message.contains(ERR_INSTALLED_TOOL_DIR_MISSING)
        || message.contains(ERR_LOCKFILE_DEPENDENCY_MISSING)
        || message.contains(ERR_ENTRYPOINT_CWD_MISSING)
    {
        RunnerErrorKind::Resolution
    } else if message.contains(ERR_TOOL_EXITED_UNSUCCESSFULLY) {
        RunnerErrorKind::SubprocessFailure
    } else if message.contains(ERR_TOOL_OUTPUT_SCHEMA_INVALID) {
        RunnerErrorKind::MalformedOutput
    } else {
        RunnerErrorKind::Other
    }
}

#[derive(Debug)]
struct ClassifiedRunnerError {
    kind: RunnerErrorKind,
    message: String,
}

impl fmt::Display for ClassifiedRunnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ClassifiedRunnerError {}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolSelector {
    Locked,
    Exact(Version),
    Latest,
    Range(VersionReq),
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSpec {
    pub package: String,
    pub selector: ToolSelector,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct RunOptions {
    pub timeout_ms: Option<u64>,
    pub env_overrides: HashMap<String, String>,
    pub output_limit_bytes: usize,
    #[cfg(unix)]
    pub cleanup_child_process_group_on_signal: bool,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            timeout_ms: None,
            env_overrides: HashMap::new(),
            output_limit_bytes: DEFAULT_OUTPUT_LIMIT_BYTES,
            #[cfg(unix)]
            cleanup_child_process_group_on_signal: false,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ResolvedTool {
    pub package: String,
    pub version: Version,
    pub tool_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest: RunnerManifest,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct RunResult {
    pub resolved: ResolvedTool,
    pub output: Value,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct RunnerManifest {
    pub kind: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    pub entrypoint: Entrypoint,
    #[serde(default)]
    pub runtime: Option<RuntimeDecl>,
    #[serde(default)]
    pub environment: Option<EnvironmentDecl>,
    #[serde(default)]
    pub inputs: Value,
    #[serde(default)]
    pub outputs: Value,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct RuntimeDecl {
    #[serde(rename = "type")]
    pub runtime_type: String,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
pub struct EnvironmentDecl {
    #[serde(default)]
    pub vars: HashMap<String, EnvVarDecl>,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
pub struct EnvVarDecl {
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct PreparedInvocation {
    command: String,
    args: Vec<String>,
    cwd: PathBuf,
    env: HashMap<String, String>,
    timeout_ms: u64,
}

#[allow(dead_code)]
pub fn parse_tool_spec(spec: &str) -> Result<ToolSpec> {
    let s = spec.trim();
    if !s.starts_with('@') || !s.contains('/') {
        bail!("invalid tool spec: expected @namespace/name");
    }

    let Some(last_at) = s.rfind('@') else {
        bail!("invalid tool spec: expected @namespace/name");
    };

    if last_at == 0 {
        return Ok(ToolSpec {
            package: s.to_string(),
            selector: ToolSelector::Locked,
        });
    }

    let package = &s[..last_at];
    let selector = &s[last_at + 1..];
    if selector.is_empty() {
        bail!("invalid tool spec: missing version selector after '@'");
    }
    if !package.starts_with('@') || !package.contains('/') {
        bail!("invalid tool spec: expected @namespace/name");
    }

    let selector = if selector == "latest" {
        ToolSelector::Latest
    } else if let Ok(exact) = Version::parse(selector) {
        ToolSelector::Exact(exact)
    } else {
        ToolSelector::Range(
            VersionReq::parse(selector)
                .with_context(|| format!("invalid version selector: {selector}"))?,
        )
    };

    Ok(ToolSpec {
        package: package.to_string(),
        selector,
    })
}

#[allow(dead_code)]
pub fn resolve_installed_tool(project_dir: &Path, spec: &ToolSpec) -> Result<ResolvedTool> {
    let tools_root = project_dir.join(".agentpm").join("tools");
    let version = match &spec.selector {
        ToolSelector::Locked => resolve_locked_version(project_dir, &spec.package)?,
        ToolSelector::Exact(version) => {
            let dir = prepared_tool_dir(&tools_root, &spec.package, version);
            if !dir.exists() {
                bail!(
                    "{} for {}@{} at {}",
                    ERR_INSTALLED_TOOL_NOT_FOUND,
                    spec.package,
                    version,
                    dir.display()
                );
            }
            version.clone()
        }
        ToolSelector::Latest => highest_installed_version(&tools_root, &spec.package)?
            .ok_or_else(|| anyhow!("{ERR_NO_INSTALLED_VERSIONS_FOUND} for {}", spec.package))?,
        ToolSelector::Range(req) => {
            highest_matching_installed_version(&tools_root, &spec.package, req)?.ok_or_else(
                || {
                    anyhow!(
                        "{ERR_NO_INSTALLED_VERSION_SATISFIES}{} satisfies {}",
                        spec.package,
                        req
                    )
                },
            )?
        }
    };

    let tool_dir = prepared_tool_dir(&tools_root, &spec.package, &version);
    if !tool_dir.exists() {
        bail!(
            "{} for {}@{}: {}",
            ERR_INSTALLED_TOOL_DIR_MISSING,
            spec.package,
            version,
            tool_dir.display()
        );
    }

    let manifest_path = tool_dir.join("agent.json");
    let manifest_text = fs::read_to_string(&manifest_path)
        .with_context(|| format!("reading {}", manifest_path.display()))?;
    let raw: Value = serde_json::from_str(&manifest_text)
        .with_context(|| format!("parsing JSON from {}", manifest_path.display()))?;
    let manifest: RunnerManifest = serde_json::from_value(raw.clone())
        .with_context(|| format!("parsing manifest at {}", manifest_path.display()))?;

    if manifest.kind != "tool" {
        bail!(
            "manifest at {} has kind=\"{}\"; only kind=\"tool\" can be executed",
            manifest_path.display(),
            manifest.kind
        );
    }

    let entrypoint = raw
        .get("entrypoint")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            anyhow!(
                "manifest at {} is missing entrypoint",
                manifest_path.display()
            )
        })?;
    if !entrypoint.contains_key("command") {
        bail!(
            "manifest at {} is missing entrypoint.command",
            manifest_path.display()
        );
    }
    if !entrypoint.contains_key("args") {
        bail!(
            "manifest at {} is missing entrypoint.args",
            manifest_path.display()
        );
    }
    if manifest.entrypoint.command.trim().is_empty() {
        bail!(
            "manifest at {} has an empty entrypoint.command",
            manifest_path.display()
        );
    }

    Ok(ResolvedTool {
        package: spec.package.clone(),
        version,
        tool_dir,
        manifest_path,
        manifest,
    })
}

#[allow(dead_code)]
pub fn run_installed_tool(
    project_dir: &Path,
    spec: &ToolSpec,
    input: &Value,
    options: &RunOptions,
) -> Result<RunResult> {
    // This runner is intentionally synchronous in Milestone 1.
    // When async CLI commands wire into it (for example `agentpm run` in Milestone 2),
    // call it through `tokio::task::spawn_blocking` rather than on the async executor.
    let resolved = resolve_installed_tool(project_dir, spec)?;
    validate_json_schema(
        &resolved.manifest.inputs,
        input,
        ERR_TOOL_INPUT_SCHEMA_MALFORMED,
        ERR_TOOL_INPUT_SCHEMA_INVALID,
        RunnerErrorKind::Schema,
        RunnerErrorKind::Schema,
        &resolved.package,
    )?;

    let run_dir = create_run_dir(project_dir)?;
    let prepared = prepare_invocation(&resolved, input, options)
        .with_context(|| format!("preparing {}", resolved.package))?;

    let input_bytes = serde_json::to_vec(input).context("serializing JSON input")?;
    #[cfg(unix)]
    let cleanup_child_process_group_on_signal = options.cleanup_child_process_group_on_signal;
    #[cfg(not(unix))]
    let cleanup_child_process_group_on_signal = false;
    let output = execute_with_timeout(
        &prepared.command,
        &prepared.args,
        &prepared.cwd,
        &prepared.env,
        &input_bytes,
        ExecuteLimits {
            timeout_ms: prepared.timeout_ms,
            cleanup_child_process_group_on_signal,
            output_limit_bytes: options.output_limit_bytes,
        },
    )?;

    let combined_size = output.stdout.len() + output.stderr.len();
    if combined_size > options.output_limit_bytes {
        preserve_failure_artifacts(
            &run_dir,
            &input_bytes,
            &output.stdout,
            &output.stderr,
            &format!(
                "tool output exceeded limit: {} bytes > {} bytes",
                combined_size, options.output_limit_bytes
            ),
        )?;
        return Err(ClassifiedRunnerError {
            kind: RunnerErrorKind::OutputLimit,
            message: format!(
                "{}: {} bytes > {} bytes (logs: {})",
                ERR_TOOL_OUTPUT_LIMIT_EXCEEDED,
                combined_size,
                options.output_limit_bytes,
                run_dir.display()
            ),
        }
        .into());
    }

    if !output.status.success() {
        let code = output
            .status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "terminated by signal".to_string());
        preserve_failure_artifacts(
            &run_dir,
            &input_bytes,
            &output.stdout,
            &output.stderr,
            &format!("{ERR_TOOL_EXITED_UNSUCCESSFULLY}: {code}"),
        )?;
        bail!(
            "{}: {} (logs: {})",
            ERR_TOOL_EXITED_UNSUCCESSFULLY,
            code,
            run_dir.display()
        );
    }

    let child_stderr = output.stderr.clone();
    let parsed = serde_json::from_slice(&output.stdout)
        .with_context(|| format!("{} (logs: {})", ERR_TOOL_OUTPUT_NOT_JSON, run_dir.display()));
    let output = match parsed {
        Ok(v) => v,
        Err(err) => {
            preserve_failure_artifacts(
                &run_dir,
                &input_bytes,
                &output.stdout,
                &output.stderr,
                &err.to_string(),
            )?;
            return Err(err);
        }
    };

    if let Err(err) = validate_json_schema(
        &resolved.manifest.outputs,
        &output,
        ERR_TOOL_OUTPUT_SCHEMA_MALFORMED,
        ERR_TOOL_OUTPUT_SCHEMA_INVALID,
        RunnerErrorKind::Schema,
        RunnerErrorKind::MalformedOutput,
        &resolved.package,
    ) {
        preserve_failure_artifacts(
            &run_dir,
            &input_bytes,
            &serde_json::to_vec(&output).unwrap_or_default(),
            &child_stderr,
            &err.to_string(),
        )?;
        return Err(err);
    }

    let _ = fs::remove_dir_all(&run_dir);

    Ok(RunResult { resolved, output })
}

#[allow(dead_code)]
fn validate_json_schema(
    schema: &Value,
    instance: &Value,
    malformed_label: &str,
    invalid_label: &str,
    malformed_kind: RunnerErrorKind,
    invalid_kind: RunnerErrorKind,
    package: &str,
) -> Result<()> {
    if schema.is_null() {
        return Ok(());
    }

    let compiled = match JSONSchema::options()
        .with_draft(Draft::Draft202012)
        .compile(schema)
    {
        Ok(compiled) => compiled,
        Err(err) => {
            return Err(ClassifiedRunnerError {
                kind: malformed_kind,
                message: format!("{malformed_label} for {package}: {err}"),
            }
            .into());
        }
    };

    if let Err(errors) = compiled.validate(instance) {
        let messages = errors
            .map(|error| {
                format!(
                    "{} at instance {} vs schema {}",
                    error, error.instance_path, error.schema_path
                )
            })
            .collect::<Vec<_>>();
        return Err(ClassifiedRunnerError {
            kind: invalid_kind,
            message: format!(
                "{invalid_label} for {package}:\n- {}",
                messages.join("\n- ")
            ),
        }
        .into());
    }

    Ok(())
}

#[allow(dead_code)]
fn resolve_locked_version(project_dir: &Path, package: &str) -> Result<Version> {
    let lock_path = project_dir.join("agent.lock");
    if !lock_path.exists() {
        bail!(
            "{ERR_LOCKFILE_MISSING} in {}; unversioned specs require a lockfile",
            project_dir.display()
        );
    }

    let lock = read_lock_or_default(project_dir)?;
    let dep = lock
        .find_unique_locked_package(package, crate::semver::types::PackageKind::Tool)?
        .ok_or_else(|| anyhow!("{} {}", package, ERR_LOCKFILE_DEPENDENCY_MISSING))?;

    Version::parse(&dep.version)
        .with_context(|| format!("locked version for {} is not valid semver", package))
}

#[allow(dead_code)]
fn highest_installed_version(tools_root: &Path, package: &str) -> Result<Option<Version>> {
    // Current behavior: `@latest` selects the highest installed semver, including prereleases.
    // This is intentionally left explicit because semver range matching below follows
    // `VersionReq::matches`, which excludes prereleases unless the requirement includes one.
    // Milestone 2 should decide whether `latest` should continue to include prereleases or
    // whether both selectors should align on stable-only behavior by default.
    let mut versions = installed_versions(tools_root, package)?;
    versions.sort();
    Ok(versions.pop())
}

#[allow(dead_code)]
fn highest_matching_installed_version(
    tools_root: &Path,
    package: &str,
    req: &VersionReq,
) -> Result<Option<Version>> {
    // Current behavior: semver range matching uses `VersionReq::matches`, which excludes
    // prereleases unless the requirement itself includes a prerelease component.
    let mut versions = installed_versions(tools_root, package)?;
    versions.sort();
    Ok(versions.into_iter().rev().find(|v| req.matches(v)))
}

#[allow(dead_code)]
fn installed_versions(tools_root: &Path, package: &str) -> Result<Vec<Version>> {
    let (namespace, name) = split_package(package)?;
    let package_dir = tools_root.join(namespace).join(name);
    if !package_dir.exists() {
        return Ok(Vec::new());
    }

    let mut versions = Vec::new();
    for entry in
        fs::read_dir(&package_dir).with_context(|| format!("reading {}", package_dir.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let file_name = entry.file_name();
        let ver = file_name.to_string_lossy();
        if let Ok(parsed) = Version::parse(&ver) {
            versions.push(parsed);
        }
    }
    Ok(versions)
}

#[allow(dead_code)]
fn prepared_tool_dir(tools_root: &Path, package: &str, version: &Version) -> PathBuf {
    let (namespace, name) = split_package(package).expect("validated package ref");
    tools_root
        .join(namespace)
        .join(name)
        .join(version.to_string())
}

#[allow(dead_code)]
fn split_package(package: &str) -> Result<(String, String)> {
    if !package.starts_with('@') {
        bail!("package must be of form @namespace/name");
    }
    let mut parts = package[1..].splitn(2, '/');
    let namespace = parts.next().ok_or_else(|| anyhow!("invalid package"))?;
    let name = parts.next().ok_or_else(|| anyhow!("invalid package"))?;
    Ok((namespace.to_string(), name.to_string()))
}

#[allow(dead_code)]
fn prepare_invocation(
    resolved: &ResolvedTool,
    _input: &Value,
    options: &RunOptions,
) -> Result<PreparedInvocation> {
    let entrypoint = &resolved.manifest.entrypoint;
    let interpreter = resolve_interpreter_command(
        entrypoint.command.as_str(),
        resolved.manifest.runtime.as_ref(),
    )?;

    let mut env: HashMap<String, String> = std::env::vars().collect();

    if let Some(env_decl) = &resolved.manifest.environment {
        for (key, decl) in &env_decl.vars {
            if !env.contains_key(key)
                && let Some(default) = &decl.default
            {
                env.insert(key.clone(), default.clone());
            }
        }
    }

    for (key, value) in &entrypoint.env {
        env.insert(key.clone(), value.clone());
    }
    for (key, value) in &options.env_overrides {
        env.insert(key.clone(), value.clone());
    }

    if let Some(env_decl) = &resolved.manifest.environment {
        let missing: Vec<String> = env_decl
            .vars
            .iter()
            .filter(|(key, decl)| decl.required && !env.contains_key(*key))
            .map(|(key, _)| key.clone())
            .collect();
        if !missing.is_empty() {
            bail!(
                "{} for {}: {}",
                ERR_REQUIRED_ENV_MISSING,
                resolved.package,
                missing.join(", ")
            );
        }
    }

    let cwd = resolved.tool_dir.join(&entrypoint.cwd);
    if !cwd.exists() {
        bail!(
            "{} for {}: {}",
            ERR_ENTRYPOINT_CWD_MISSING,
            resolved.package,
            cwd.display()
        );
    }

    Ok(PreparedInvocation {
        command: interpreter,
        args: entrypoint.args.clone(),
        cwd,
        env,
        timeout_ms: options.timeout_ms.unwrap_or(entrypoint.timeout_ms),
    })
}

#[allow(dead_code)]
fn resolve_interpreter_command(command: &str, runtime: Option<&RuntimeDecl>) -> Result<String> {
    let requested = interpreter_family(command)
        .ok_or_else(|| anyhow!("unsupported entrypoint.command: {command}"))?;

    if let Some(runtime) = runtime {
        let expected = interpreter_family(runtime.runtime_type.as_str())
            .ok_or_else(|| anyhow!("unsupported runtime.type: {}", runtime.runtime_type))?;
        if expected != requested {
            bail!(
                "{}: runtime.type={} but entrypoint.command={}",
                ERR_RUNTIME_TYPE_MISMATCH,
                runtime.runtime_type,
                command
            );
        }
    }

    let override_key = match requested.as_str() {
        "node" => Some("AGENTPM_NODE"),
        "python" => Some("AGENTPM_PYTHON"),
        _ => None,
    };

    let resolved = override_key
        .and_then(|key| std::env::var(key).ok())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| command.to_string());

    let actual = interpreter_family(&resolved)
        .or_else(|| basename_interpreter_family(&resolved))
        .ok_or_else(|| anyhow!("{ERR_INTERPRETER_UNSUPPORTED}: {resolved}"))?;

    if actual != requested {
        bail!(
            "{}: expected {} family, got {}",
            ERR_INTERPRETER_OVERRIDE_MISMATCH,
            requested,
            actual
        );
    }

    let version_output = ensure_interpreter_available(&resolved)?;
    if let Some(runtime) = runtime
        && let Some(required_version) = runtime.version.as_deref()
    {
        enforce_runtime_minimum_version(
            &resolved,
            runtime.runtime_type.as_str(),
            required_version,
            &version_output,
        )?;
    }
    Ok(resolved)
}

#[allow(dead_code)]
fn interpreter_family(command: &str) -> Option<String> {
    match canonical_interpreter(command).as_str() {
        "node" | "nodejs" => Some("node".to_string()),
        "python" | "python3" => Some("python".to_string()),
        _ => None,
    }
}

#[allow(dead_code)]
fn basename_interpreter_family(command: &str) -> Option<String> {
    let path = Path::new(command);
    path.file_name()
        .and_then(OsStr::to_str)
        .and_then(interpreter_family)
}

#[allow(dead_code)]
fn canonical_interpreter(command: &str) -> String {
    let lowered = command.to_ascii_lowercase();
    lowered
        .strip_suffix(".exe")
        .or_else(|| lowered.strip_suffix(".cmd"))
        .or_else(|| lowered.strip_suffix(".bat"))
        .unwrap_or(&lowered)
        .to_string()
}

#[allow(dead_code)]
fn ensure_interpreter_available(command: &str) -> Result<String> {
    let output = Command::new(command)
        .arg("--version")
        .output()
        .with_context(|| format!("{ERR_INTERPRETER_NOT_EXECUTABLE}: {command}"))?;
    if !output.status.success() {
        bail!("{ERR_INTERPRETER_HEALTHCHECK_FAILED}: {command}");
    }
    let mut text = String::new();
    text.push_str(&String::from_utf8_lossy(&output.stdout));
    if !output.stderr.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    Ok(text)
}

#[allow(dead_code)]
fn enforce_runtime_minimum_version(
    command: &str,
    runtime_type: &str,
    required_version: &str,
    version_output: &str,
) -> Result<()> {
    let required = parse_runtime_version(required_version).with_context(|| {
        format!("{ERR_RUNTIME_VERSION_UNREADABLE}: invalid required version {required_version}")
    })?;
    let actual = extract_runtime_version(version_output).ok_or_else(|| {
        anyhow!("{ERR_RUNTIME_VERSION_UNREADABLE}: {runtime_type} from {command}")
    })?;

    if actual < required {
        bail!(
            "{}: {} requires >= {}, got {} from {}",
            ERR_RUNTIME_VERSION_UNSATISFIED,
            runtime_type,
            required,
            actual,
            command
        );
    }

    Ok(())
}

#[allow(dead_code)]
struct ExecuteLimits {
    timeout_ms: u64,
    cleanup_child_process_group_on_signal: bool,
    output_limit_bytes: usize,
}

#[allow(dead_code)]
fn execute_with_timeout(
    command: &str,
    args: &[String],
    cwd: &Path,
    env: &HashMap<String, String>,
    input: &[u8],
    limits: ExecuteLimits,
) -> Result<Output> {
    let mut cmd = Command::new(command);
    cmd.args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Keep the tool in its own process group so timeout cleanup can kill the group.
        unsafe {
            cmd.pre_exec(|| {
                if libc::setpgid(0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    let mut child = cmd
        .spawn()
        .with_context(|| format!("spawning tool process via {command}"))?;

    #[cfg(unix)]
    let _signal_guard = if limits.cleanup_child_process_group_on_signal {
        Some(ActiveChildProcessGroupSignalGuard::new(child.id())?)
    } else {
        None
    };

    let stdout = child.stdout.take().context("capturing child stdout")?;
    let stderr = child.stderr.take().context("capturing child stderr")?;
    let retained_limit = limits.output_limit_bytes.saturating_add(1);
    let retained_total = Arc::new(AtomicUsize::new(0));
    let output_exceeded = Arc::new(AtomicBool::new(false));
    let stdout_reader = spawn_output_reader(
        stdout,
        retained_limit,
        Arc::clone(&retained_total),
        Arc::clone(&output_exceeded),
    );
    let stderr_reader = spawn_output_reader(
        stderr,
        retained_limit,
        Arc::clone(&retained_total),
        Arc::clone(&output_exceeded),
    );

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input)
            .context("writing JSON input to child stdin")?;
    }

    let start = Instant::now();
    let timeout = Duration::from_millis(limits.timeout_ms);
    loop {
        if child
            .try_wait()
            .context("waiting for tool process")?
            .is_some()
        {
            break;
        }

        if start.elapsed() >= timeout {
            #[cfg(unix)]
            {
                let _ = kill_process_group(child.id());
            }
            let _ = child.kill();
            child.wait().context("waiting for timed-out tool process")?;
            let stdout = join_output_reader(stdout_reader)?;
            let stderr = join_output_reader(stderr_reader)?;
            bail!(
                "{} after {} ms (stdout {} bytes, stderr {} bytes)",
                ERR_TOOL_TIMED_OUT,
                limits.timeout_ms,
                stdout.len(),
                stderr.len()
            );
        }

        std::thread::sleep(Duration::from_millis(10));
    }

    let status = child.wait().context("waiting for tool process")?;
    let stdout = join_output_reader(stdout_reader)?;
    let stderr = join_output_reader(stderr_reader)?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

#[allow(dead_code)]
fn spawn_output_reader<R>(
    mut reader: R,
    retained_limit: usize,
    retained_total: Arc<AtomicUsize>,
    output_exceeded: Arc<AtomicBool>,
) -> std::thread::JoinHandle<std::io::Result<Vec<u8>>>
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut retained = Vec::new();
        let mut chunk = [0_u8; 8192];
        loop {
            let read = reader.read(&mut chunk)?;
            if read == 0 {
                break;
            }

            let previous = retained_total.fetch_add(read, Ordering::SeqCst);
            if previous + read > retained_limit {
                output_exceeded.store(true, Ordering::SeqCst);
            }
            if previous < retained_limit {
                let remaining = retained_limit - previous;
                retained.extend_from_slice(&chunk[..read.min(remaining)]);
            }
        }
        Ok(retained)
    })
}

#[allow(dead_code)]
fn join_output_reader(
    reader: std::thread::JoinHandle<std::io::Result<Vec<u8>>>,
) -> Result<Vec<u8>> {
    reader
        .join()
        .map_err(|_| anyhow!("tool output reader thread panicked"))?
        .context("reading tool process output")
}

#[cfg(unix)]
#[allow(dead_code)]
fn kill_process_group(pid: u32) -> Result<()> {
    let rc = unsafe { libc::killpg(pid as i32, libc::SIGKILL) };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() != std::io::ErrorKind::NotFound {
            return Err(err).context("killing timed-out process group");
        }
    }
    Ok(())
}

#[cfg(unix)]
struct ActiveChildProcessGroupSignalGuard {
    pgid: i32,
    slot: usize,
}

#[cfg(unix)]
impl ActiveChildProcessGroupSignalGuard {
    fn new(pid: u32) -> Result<Self> {
        let pgid = pid as i32;
        let slot = register_active_child_process_group(pgid)?;
        install_signal_handlers();
        Ok(Self { pgid, slot })
    }
}

#[cfg(unix)]
impl Drop for ActiveChildProcessGroupSignalGuard {
    fn drop(&mut self) {
        unregister_active_child_process_group(self.slot, self.pgid);
        restore_signal_handlers_if_idle();
    }
}

#[cfg(unix)]
fn register_active_child_process_group(pgid: i32) -> Result<usize> {
    for (index, slot) in ACTIVE_CHILD_PGIDS.iter().enumerate() {
        if slot
            .compare_exchange(0, pgid, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            return Ok(index);
        }
    }
    bail!("too many active tool process groups for signal cleanup")
}

#[cfg(unix)]
fn unregister_active_child_process_group(slot: usize, pgid: i32) {
    ACTIVE_CHILD_PGIDS[slot]
        .compare_exchange(pgid, 0, Ordering::SeqCst, Ordering::SeqCst)
        .ok();
}

#[cfg(unix)]
fn install_signal_handlers() {
    let mut state = SIGNAL_HANDLER_STATE
        .lock()
        .expect("signal handler state poisoned");
    if state.active_guards == 0 {
        state.previous_sigint = Some(unsafe {
            libc::signal(
                libc::SIGINT,
                terminate_active_process_groups as *const () as libc::sighandler_t,
            )
        });
        state.previous_sigterm = Some(unsafe {
            libc::signal(
                libc::SIGTERM,
                terminate_active_process_groups as *const () as libc::sighandler_t,
            )
        });
    }
    state.active_guards += 1;
}

#[cfg(unix)]
fn restore_signal_handlers_if_idle() {
    let mut state = SIGNAL_HANDLER_STATE
        .lock()
        .expect("signal handler state poisoned");
    state.active_guards = state.active_guards.saturating_sub(1);
    if state.active_guards == 0 {
        if let Some(previous) = state.previous_sigint.take() {
            unsafe {
                libc::signal(libc::SIGINT, previous);
            }
        }
        if let Some(previous) = state.previous_sigterm.take() {
            unsafe {
                libc::signal(libc::SIGTERM, previous);
            }
        }
    }
}

#[cfg(unix)]
extern "C" fn terminate_active_process_groups(signal: libc::c_int) {
    for slot in &ACTIVE_CHILD_PGIDS {
        let pgid = slot.swap(0, Ordering::SeqCst);
        if pgid > 0 {
            unsafe {
                libc::killpg(pgid, libc::SIGKILL);
            }
        }
    }
    unsafe {
        libc::_exit(128 + signal);
    }
}

#[allow(dead_code)]
fn create_run_dir(project_dir: &Path) -> Result<PathBuf> {
    let base = project_dir.join(".agentpm").join("runs");
    fs::create_dir_all(&base).with_context(|| format!("creating {}", base.display()))?;
    let name = format!(
        "run-{}-{}",
        chrono::Utc::now().format("%Y%m%d%H%M%S%3f"),
        std::process::id()
    );
    let dir = base.join(name);
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    Ok(dir)
}

#[allow(dead_code)]
fn preserve_failure_artifacts(
    run_dir: &Path,
    input: &[u8],
    stdout: &[u8],
    stderr: &[u8],
    error: &str,
) -> Result<()> {
    fs::write(run_dir.join("input.json"), input)
        .with_context(|| format!("writing {}", run_dir.join("input.json").display()))?;
    fs::write(run_dir.join("child.stdout"), stdout)
        .with_context(|| format!("writing {}", run_dir.join("child.stdout").display()))?;
    fs::write(run_dir.join("child.stderr"), stderr)
        .with_context(|| format!("writing {}", run_dir.join("child.stderr").display()))?;
    fs::write(run_dir.join("error.txt"), error)
        .with_context(|| format!("writing {}", run_dir.join("error.txt").display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn parses_tool_specs() {
        let locked = parse_tool_spec("@zack/echo-json").unwrap();
        assert_eq!(locked.package, "@zack/echo-json");
        assert!(matches!(locked.selector, ToolSelector::Locked));

        let exact = parse_tool_spec("@zack/echo-json@0.1.0").unwrap();
        assert!(
            matches!(exact.selector, ToolSelector::Exact(v) if v == Version::parse("0.1.0").unwrap())
        );

        let latest = parse_tool_spec("@zack/echo-json@latest").unwrap();
        assert!(matches!(latest.selector, ToolSelector::Latest));

        let range = parse_tool_spec("@zack/echo-json@^0.1").unwrap();
        assert!(matches!(range.selector, ToolSelector::Range(_)));
    }

    #[test]
    fn resolves_locked_latest_and_range_versions() {
        let root = TestProject::new();
        root.write_lock(
            r#"{
  "lockfile_version": 1,
  "generated": "2026-05-03T00:00:00Z",
  "dependencies": {
    "@zack/echo-json": {
      "version": "0.1.0",
      "integrity": "abc"
    }
  }
}"#,
        );
        root.write_tool_manifest("@zack/echo-json", "0.1.0", tool_manifest("python3"));
        root.write_tool_manifest("@zack/echo-json", "0.1.5", tool_manifest("python3"));
        root.write_tool_manifest("@zack/echo-json", "0.2.0", tool_manifest("python3"));

        let locked =
            resolve_installed_tool(root.path(), &parse_tool_spec("@zack/echo-json").unwrap())
                .unwrap();
        assert_eq!(locked.version, Version::parse("0.1.0").unwrap());

        let latest = resolve_installed_tool(
            root.path(),
            &parse_tool_spec("@zack/echo-json@latest").unwrap(),
        )
        .unwrap();
        assert_eq!(latest.version, Version::parse("0.2.0").unwrap());

        let range = resolve_installed_tool(
            root.path(),
            &parse_tool_spec("@zack/echo-json@^0.1").unwrap(),
        )
        .unwrap();
        assert_eq!(range.version, Version::parse("0.1.5").unwrap());
    }

    #[test]
    fn unversioned_spec_requires_lockfile() {
        let root = TestProject::new();
        root.write_tool_manifest("@zack/echo-json", "0.1.0", tool_manifest("python3"));

        let err = resolve_installed_tool(root.path(), &parse_tool_spec("@zack/echo-json").unwrap())
            .unwrap_err();

        assert!(
            format!("{err:#}").contains("agent.lock not found"),
            "{err:#}"
        );
    }

    #[test]
    fn exact_version_requires_installed_prepared_tool() {
        let root = TestProject::new();
        root.write_tool_manifest("@zack/echo-json", "0.1.0", tool_manifest("python3"));

        let err = resolve_installed_tool(
            root.path(),
            &parse_tool_spec("@zack/echo-json@0.2.0").unwrap(),
        )
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("installed tool not found"),
            "{err:#}"
        );
    }

    #[test]
    fn rejects_non_tool_manifests() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/not-a-tool", "0.1.0"));
        root.write_tool_manifest(
            "@zack/not-a-tool",
            "0.1.0",
            r#"{
  "kind": "agent",
  "name": "not-a-tool",
  "version": "0.1.0",
  "entrypoint": {
    "command": "python3",
    "args": ["script.py"],
    "cwd": ".",
    "timeout_ms": 5000,
    "env": {}
  }
}"#
            .to_string(),
        );

        let err =
            resolve_installed_tool(root.path(), &parse_tool_spec("@zack/not-a-tool").unwrap())
                .unwrap_err();

        assert!(
            format!("{err:#}").contains("only kind=\"tool\" can be executed"),
            "{err:#}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn signal_cleanup_tracks_multiple_active_process_groups() {
        let first = register_active_child_process_group(111_111).unwrap();
        let second = register_active_child_process_group(222_222).unwrap();

        let active_pgids = ACTIVE_CHILD_PGIDS
            .iter()
            .map(|slot| slot.load(Ordering::SeqCst))
            .filter(|pgid| *pgid > 0)
            .collect::<Vec<_>>();

        assert_ne!(first, second);
        assert!(active_pgids.contains(&111_111));
        assert!(active_pgids.contains(&222_222));

        unregister_active_child_process_group(first, 111_111);
        unregister_active_child_process_group(second, 222_222);
    }

    #[test]
    fn fails_when_required_env_is_missing() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/requires-env", "0.1.0"));
        root.write_tool(
            "@zack/requires-env",
            "0.1.0",
            tool_manifest_with_env("python3", true, None),
            python_echo_script(),
        );

        let err = run_installed_tool(
            root.path(),
            &parse_tool_spec("@zack/requires-env").unwrap(),
            &serde_json::json!({}),
            &RunOptions::default(),
        )
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("missing required environment variables"),
            "{err:#}"
        );
    }

    #[test]
    fn applies_environment_defaults() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/uses-env-default", "0.1.0"));
        root.write_tool(
            "@zack/uses-env-default",
            "0.1.0",
            tool_manifest_with_env("python3", false, Some("fallback-token")),
            python_echo_script(),
        );

        let result = run_installed_tool(
            root.path(),
            &parse_tool_spec("@zack/uses-env-default").unwrap(),
            &serde_json::json!({}),
            &RunOptions::default(),
        )
        .unwrap();

        assert_eq!(result.output["envValue"], "fallback-token");
    }

    #[test]
    fn rejects_runtime_interpreter_mismatch() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/bad-runtime", "0.1.0"));
        root.write_tool(
            "@zack/bad-runtime",
            "0.1.0",
            tool_manifest_with_runtime("python3", "node"),
            python_echo_script(),
        );

        let err = run_installed_tool(
            root.path(),
            &parse_tool_spec("@zack/bad-runtime").unwrap(),
            &serde_json::json!({}),
            &RunOptions::default(),
        )
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("runtime/type mismatch"),
            "{err:#}"
        );
    }

    #[test]
    fn rejects_runtime_minimum_version_mismatch() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/future-python", "0.1.0"));
        root.write_tool(
            "@zack/future-python",
            "0.1.0",
            tool_manifest_with_runtime_version("python3", "python", "999.0.0"),
            python_echo_script(),
        );

        let err = run_installed_tool(
            root.path(),
            &parse_tool_spec("@zack/future-python").unwrap(),
            &serde_json::json!({}),
            &RunOptions::default(),
        )
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("runtime minimum version not satisfied"),
            "{err:#}"
        );
        assert_eq!(classify_runner_error(&err), RunnerErrorKind::Runtime);
    }

    #[test]
    fn rejects_input_schema_before_launching_tool() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/schema-input", "0.1.0"));
        root.write_tool(
            "@zack/schema-input",
            "0.1.0",
            tool_manifest_with_schemas(
                "python3",
                r#"{"type":"object","required":["message"],"properties":{"message":{"type":"string"}}}"#,
                r#"{"type":"object"}"#,
            ),
            python_marker_script(),
        );

        let err = run_installed_tool(
            root.path(),
            &parse_tool_spec("@zack/schema-input").unwrap(),
            &serde_json::json!({}),
            &RunOptions::default(),
        )
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("tool input failed schema validation"),
            "{err:#}"
        );
        assert_eq!(classify_runner_error(&err), RunnerErrorKind::Schema);
        assert!(
            !root
                .tool_dir("@zack/schema-input", "0.1.0")
                .join("launched.txt")
                .exists(),
            "invalid input should fail before child process launch"
        );
    }

    #[test]
    fn rejects_output_schema_after_parsing_successful_tool_json() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/schema-output", "0.1.0"));
        root.write_tool(
            "@zack/schema-output",
            "0.1.0",
            tool_manifest_with_schemas(
                "python3",
                r#"{"type":"object"}"#,
                r#"{"type":"object","required":["message"],"properties":{"message":{"type":"string"}}}"#,
            ),
            python_wrong_output_script(),
        );

        let err = run_installed_tool(
            root.path(),
            &parse_tool_spec("@zack/schema-output").unwrap(),
            &serde_json::json!({}),
            &RunOptions::default(),
        )
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("tool output failed schema validation"),
            "{err:#}"
        );
        assert_eq!(
            classify_runner_error(&err),
            RunnerErrorKind::MalformedOutput
        );
        let run_dir = single_run_dir(root.path());
        assert_eq!(
            fs::read_to_string(run_dir.join("child.stderr")).unwrap(),
            "diagnostic from stderr\n"
        );
    }

    #[test]
    fn malformed_output_schema_is_classified_as_schema_error() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/malformed-output-schema", "0.1.0"));
        root.write_tool(
            "@zack/malformed-output-schema",
            "0.1.0",
            tool_manifest_with_schemas(
                "python3",
                r#"{"type":"object"}"#,
                r#"{"type":"not-a-json-schema-type"}"#,
            ),
            python_domain_failure_script(),
        );

        let err = run_installed_tool(
            root.path(),
            &parse_tool_spec("@zack/malformed-output-schema").unwrap(),
            &serde_json::json!({}),
            &RunOptions::default(),
        )
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("tool output schema is invalid"),
            "{err:#}"
        );
        assert_eq!(classify_runner_error(&err), RunnerErrorKind::Schema);
    }

    #[test]
    fn treats_schema_valid_domain_failure_payload_as_success() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/domain-output", "0.1.0"));
        root.write_tool(
            "@zack/domain-output",
            "0.1.0",
            tool_manifest_with_schemas(
                "python3",
                r#"{"type":"object"}"#,
                r#"{"type":"object","required":["ok"],"properties":{"ok":{"type":"boolean"},"error":{"type":"string"}}}"#,
            ),
            python_domain_failure_script(),
        );

        let result = run_installed_tool(
            root.path(),
            &parse_tool_spec("@zack/domain-output").unwrap(),
            &serde_json::json!({}),
            &RunOptions::default(),
        )
        .unwrap();

        assert_eq!(result.output["ok"], false);
        assert_eq!(result.output["error"], "domain-level failure");
    }

    #[test]
    fn rejects_malformed_json_output() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/malformed-output", "0.1.0"));
        root.write_tool(
            "@zack/malformed-output",
            "0.1.0",
            tool_manifest("python3"),
            python_malformed_output_script(),
        );

        let err = run_installed_tool(
            root.path(),
            &parse_tool_spec("@zack/malformed-output").unwrap(),
            &serde_json::json!({}),
            &RunOptions::default(),
        )
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("tool stdout was not valid JSON"),
            "{err:#}"
        );
        assert_eq!(
            classify_runner_error(&err),
            RunnerErrorKind::MalformedOutput
        );
    }

    #[test]
    fn rejects_subprocess_failure() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/failing-tool", "0.1.0"));
        root.write_tool(
            "@zack/failing-tool",
            "0.1.0",
            tool_manifest("python3"),
            python_exit_failure_script(),
        );

        let err = run_installed_tool(
            root.path(),
            &parse_tool_spec("@zack/failing-tool").unwrap(),
            &serde_json::json!({}),
            &RunOptions::default(),
        )
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("tool exited unsuccessfully"),
            "{err:#}"
        );
        assert_eq!(
            classify_runner_error(&err),
            RunnerErrorKind::SubprocessFailure
        );
    }

    #[test]
    fn rejects_output_limit_overrun() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/large-output", "0.1.0"));
        root.write_tool(
            "@zack/large-output",
            "0.1.0",
            tool_manifest("python3"),
            python_large_output_script(),
        );
        let options = RunOptions {
            output_limit_bytes: 16,
            ..RunOptions::default()
        };

        let err = run_installed_tool(
            root.path(),
            &parse_tool_spec("@zack/large-output").unwrap(),
            &serde_json::json!({}),
            &options,
        )
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("tool output exceeded limit"),
            "{err:#}"
        );
        assert_eq!(classify_runner_error(&err), RunnerErrorKind::OutputLimit);
    }

    #[cfg(unix)]
    #[test]
    fn timeout_kills_nested_child_process_group() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/nested-timeout", "0.1.0"));
        root.write_tool(
            "@zack/nested-timeout",
            "0.1.0",
            tool_manifest("python3"),
            python_nested_sleep_script(),
        );
        let pid_path = root
            .tool_dir("@zack/nested-timeout", "0.1.0")
            .join("child.pid");
        let options = RunOptions {
            timeout_ms: Some(1000),
            ..RunOptions::default()
        };

        let err = run_installed_tool(
            root.path(),
            &parse_tool_spec("@zack/nested-timeout").unwrap(),
            &serde_json::json!({}),
            &options,
        )
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("tool execution timed out"),
            "{err:#}"
        );
        let child_pid = fs::read_to_string(pid_path)
            .unwrap()
            .trim()
            .parse::<i32>()
            .unwrap();
        for _ in 0..20 {
            if unsafe { libc::kill(child_pid, 0) } != 0 {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        panic!("nested child process {child_pid} survived timeout cleanup");
    }

    #[test]
    fn executes_simple_python_tool() {
        let python = available_command(&["python3", "python"]).expect("python required for tests");
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-python", "0.1.0"));
        root.write_tool(
            "@zack/echo-python",
            "0.1.0",
            tool_manifest("python3"),
            python_echo_script(),
        );

        let mut options = RunOptions::default();
        options
            .env_overrides
            .insert("AGENTPM_PYTHON".to_string(), python);

        let result = run_installed_tool(
            root.path(),
            &parse_tool_spec("@zack/echo-python").unwrap(),
            &serde_json::json!({"message":"hi"}),
            &options,
        )
        .unwrap();

        assert_eq!(result.output["input"]["message"], "hi");
        assert_eq!(result.output["envValue"], "");
    }

    #[test]
    fn executes_simple_node_tool() {
        let node = available_command(&["node", "nodejs"]).expect("node required for tests");
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-node", "0.1.0"));
        root.write_tool(
            "@zack/echo-node",
            "0.1.0",
            tool_manifest("node"),
            node_echo_script(),
        );

        let mut options = RunOptions::default();
        options
            .env_overrides
            .insert("AGENTPM_NODE".to_string(), node);

        let result = run_installed_tool(
            root.path(),
            &parse_tool_spec("@zack/echo-node").unwrap(),
            &serde_json::json!({"message":"hello"}),
            &options,
        )
        .unwrap();

        assert_eq!(result.output["input"]["message"], "hello");
        assert_eq!(result.output["envValue"], "");
    }

    fn available_command(candidates: &[&str]) -> Option<String> {
        candidates.iter().find_map(|candidate| {
            Command::new(candidate)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .ok()
                .filter(|s| s.success())
                .map(|_| (*candidate).to_string())
        })
    }

    fn lock_for(package: &str, version: &str) -> String {
        format!(
            r#"{{
  "lockfile_version": 1,
  "generated": "2026-05-03T00:00:00Z",
  "dependencies": {{
    "{package}": {{
      "version": "{version}",
      "integrity": "abc"
    }}
  }}
}}"#
        )
    }

    fn tool_manifest(command: &str) -> String {
        tool_manifest_with_env(command, false, None)
    }

    fn tool_manifest_with_runtime(command: &str, runtime_type: &str) -> String {
        format!(
            r#"{{
  "kind": "tool",
  "name": "echo-tool",
  "version": "0.1.0",
  "entrypoint": {{
    "command": "{command}",
    "args": ["script.py"],
    "cwd": ".",
    "timeout_ms": 5000,
    "env": {{}}
  }},
  "runtime": {{
    "type": "{runtime_type}"
  }}
}}"#
        )
    }

    fn tool_manifest_with_runtime_version(
        command: &str,
        runtime_type: &str,
        runtime_version: &str,
    ) -> String {
        format!(
            r#"{{
  "kind": "tool",
  "name": "echo-tool",
  "version": "0.1.0",
  "entrypoint": {{
    "command": "{command}",
    "args": ["script.py"],
    "cwd": ".",
    "timeout_ms": 5000,
    "env": {{}}
  }},
  "runtime": {{
    "type": "{runtime_type}",
    "version": "{runtime_version}"
  }}
}}"#
        )
    }

    fn tool_manifest_with_schemas(command: &str, inputs: &str, outputs: &str) -> String {
        let runtime_type = if command.starts_with("node") {
            "node"
        } else {
            "python"
        };
        format!(
            r#"{{
  "kind": "tool",
  "name": "schema-tool",
  "version": "0.1.0",
  "entrypoint": {{
    "command": "{command}",
    "args": ["script.py"],
    "cwd": ".",
    "timeout_ms": 5000,
    "env": {{}}
  }},
  "runtime": {{
    "type": "{runtime_type}"
  }},
  "inputs": {inputs},
  "outputs": {outputs}
}}"#
        )
    }

    fn tool_manifest_with_env(command: &str, required: bool, default: Option<&str>) -> String {
        let default_field = default
            .map(|v| format!(r#","default":"{v}""#))
            .unwrap_or_default();
        let script_name = if command.starts_with("node") {
            "script.js"
        } else {
            "script.py"
        };
        let runtime_type = if command.starts_with("node") {
            "node"
        } else {
            "python"
        };
        format!(
            r#"{{
  "kind": "tool",
  "name": "echo-tool",
  "version": "0.1.0",
  "entrypoint": {{
    "command": "{command}",
    "args": ["{script_name}"],
    "cwd": ".",
    "timeout_ms": 5000,
    "env": {{}}
  }},
  "runtime": {{
    "type": "{runtime_type}"
  }},
  "environment": {{
    "vars": {{
      "SPECIAL_TOKEN": {{
        "required": {required}{default_field}
      }}
    }}
  }}
}}"#
        )
    }

    fn python_echo_script() -> &'static str {
        r#"import json, os, sys
payload = json.load(sys.stdin)
print(json.dumps({"input": payload, "envValue": os.environ.get("SPECIAL_TOKEN", "")}))
"#
    }

    fn python_marker_script() -> &'static str {
        r#"import json, pathlib, sys
json.load(sys.stdin)
pathlib.Path("launched.txt").write_text("launched")
print(json.dumps({"ok": True}))
"#
    }

    fn python_wrong_output_script() -> &'static str {
        r#"import json, sys
json.load(sys.stdin)
print("diagnostic from stderr", file=sys.stderr)
print(json.dumps({"wrong": True}))
"#
    }

    fn python_domain_failure_script() -> &'static str {
        r#"import json, sys
json.load(sys.stdin)
print(json.dumps({"ok": False, "error": "domain-level failure"}))
"#
    }

    fn python_malformed_output_script() -> &'static str {
        r#"import json, sys
json.load(sys.stdin)
print("not-json")
"#
    }

    fn python_exit_failure_script() -> &'static str {
        r#"import json, sys
json.load(sys.stdin)
print(json.dumps({"error": "boom"}))
sys.exit(7)
"#
    }

    fn python_large_output_script() -> &'static str {
        r#"import json, sys
json.load(sys.stdin)
print(json.dumps({"payload": "x" * 128}))
"#
    }

    fn python_nested_sleep_script() -> &'static str {
        r#"import json, pathlib, subprocess, sys, time
json.load(sys.stdin)
child = subprocess.Popen(["sleep", "10"])
pathlib.Path("child.pid").write_text(str(child.pid))
sys.stdout.flush()
time.sleep(10)
"#
    }

    fn node_echo_script() -> &'static str {
        r#"let data = '';
process.stdin.on('data', chunk => data += chunk);
process.stdin.on('end', () => {
  const payload = JSON.parse(data || '{}');
  process.stdout.write(JSON.stringify({ input: payload, envValue: process.env.SPECIAL_TOKEN || '' }));
});
"#
    }

    fn single_run_dir(project_dir: &Path) -> PathBuf {
        let runs_dir = project_dir.join(".agentpm").join("runs");
        let entries = fs::read_dir(&runs_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 1, "runs dir: {}", runs_dir.display());
        entries[0].clone()
    }

    struct TestProject {
        root: PathBuf,
    }

    impl TestProject {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!("agentpm-runner-test-{unique}-{id}"));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn path(&self) -> &Path {
            &self.root
        }

        fn write_lock(&self, content: impl AsRef<str>) {
            fs::write(self.root.join("agent.lock"), content.as_ref()).unwrap();
        }

        fn write_tool_manifest(&self, package: &str, version: &str, manifest: String) {
            let (namespace, name) = split_package(package).unwrap();
            let dir = self
                .root
                .join(".agentpm")
                .join("tools")
                .join(namespace)
                .join(name)
                .join(version);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("agent.json"), manifest).unwrap();
        }

        fn write_tool(&self, package: &str, version: &str, manifest: String, script: &str) {
            self.write_tool_manifest(package, version, manifest.clone());
            let dir = self.tool_dir(package, version);
            let script_name = if manifest.contains("\"command\": \"node") {
                "script.js"
            } else {
                "script.py"
            };
            fs::write(dir.join(script_name), script).unwrap();
        }

        fn tool_dir(&self, package: &str, version: &str) -> PathBuf {
            let (namespace, name) = split_package(package).unwrap();
            self.root
                .join(".agentpm")
                .join("tools")
                .join(namespace)
                .join(name)
                .join(version)
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
