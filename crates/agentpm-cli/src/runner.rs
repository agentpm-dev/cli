use crate::manifest::{Entrypoint, read_lock_or_default};
use anyhow::{Context, Result, anyhow, bail};
use semver::{Version, VersionReq};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

#[allow(dead_code)]
const DEFAULT_OUTPUT_LIMIT_BYTES: usize = 1024 * 1024;

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
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            timeout_ms: None,
            env_overrides: HashMap::new(),
            output_limit_bytes: DEFAULT_OUTPUT_LIMIT_BYTES,
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
    pub entrypoint: Entrypoint,
    #[serde(default)]
    pub runtime: Option<RuntimeDecl>,
    #[serde(default)]
    pub environment: Option<EnvironmentDecl>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeDecl {
    #[serde(rename = "type")]
    pub runtime_type: String,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct EnvironmentDecl {
    #[serde(default)]
    pub vars: HashMap<String, EnvVarDecl>,
}

#[derive(Debug, Clone, Deserialize, Default)]
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
                    "installed tool not found for {}@{} at {}",
                    spec.package,
                    version,
                    dir.display()
                );
            }
            version.clone()
        }
        ToolSelector::Latest => highest_installed_version(&tools_root, &spec.package)?
            .ok_or_else(|| anyhow!("no installed versions found for {}", spec.package))?,
        ToolSelector::Range(req) => {
            highest_matching_installed_version(&tools_root, &spec.package, req)?.ok_or_else(
                || anyhow!("no installed version of {} satisfies {}", spec.package, req),
            )?
        }
    };

    let tool_dir = prepared_tool_dir(&tools_root, &spec.package, &version);
    if !tool_dir.exists() {
        bail!(
            "installed tool directory does not exist for {}@{}: {}",
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
    let run_dir = create_run_dir(project_dir)?;
    let prepared = prepare_invocation(&resolved, input, options)
        .with_context(|| format!("preparing {}", resolved.package))?;

    let input_bytes = serde_json::to_vec(input).context("serializing JSON input")?;
    let output = execute_with_timeout(
        &prepared.command,
        &prepared.args,
        &prepared.cwd,
        &prepared.env,
        &input_bytes,
        prepared.timeout_ms,
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
        bail!(
            "tool output exceeded limit: {} bytes > {} bytes (logs: {})",
            combined_size,
            options.output_limit_bytes,
            run_dir.display()
        );
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
            &format!("tool exited unsuccessfully: {code}"),
        )?;
        bail!(
            "tool exited unsuccessfully: {} (logs: {})",
            code,
            run_dir.display()
        );
    }

    let parsed = serde_json::from_slice(&output.stdout).with_context(|| {
        format!(
            "tool stdout was not valid JSON (logs: {})",
            run_dir.display()
        )
    });
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

    let _ = fs::remove_dir_all(&run_dir);

    Ok(RunResult { resolved, output })
}

#[allow(dead_code)]
fn resolve_locked_version(project_dir: &Path, package: &str) -> Result<Version> {
    let lock_path = project_dir.join("agent.lock");
    if !lock_path.exists() {
        bail!(
            "agent.lock not found in {}; unversioned specs require a lockfile",
            project_dir.display()
        );
    }

    let lock = read_lock_or_default(project_dir)?;
    let dep = lock
        .dependencies
        .get(package)
        .ok_or_else(|| anyhow!("{} not found in agent.lock", package))?;

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
                "missing required environment variables for {}: {}",
                resolved.package,
                missing.join(", ")
            );
        }
    }

    let cwd = resolved.tool_dir.join(&entrypoint.cwd);
    if !cwd.exists() {
        bail!(
            "entrypoint cwd does not exist for {}: {}",
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
                "runtime/type mismatch: runtime.type={} but entrypoint.command={}",
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
        .ok_or_else(|| anyhow!("unsupported interpreter override or command: {resolved}"))?;

    if actual != requested {
        bail!(
            "interpreter override mismatch: expected {} family, got {}",
            requested,
            actual
        );
    }

    ensure_interpreter_available(&resolved)?;
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
fn ensure_interpreter_available(command: &str) -> Result<()> {
    let status = Command::new(command)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("interpreter not found or not executable: {command}"))?;
    if !status.success() {
        bail!("interpreter command failed health check: {command}");
    }
    Ok(())
}

#[allow(dead_code)]
fn execute_with_timeout(
    command: &str,
    args: &[String],
    cwd: &Path,
    env: &HashMap<String, String>,
    input: &[u8],
    timeout_ms: u64,
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

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input)
            .context("writing JSON input to child stdin")?;
    }

    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
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
            let output = child
                .wait_with_output()
                .context("collecting timed-out output")?;
            bail!(
                "tool execution timed out after {} ms (stdout {} bytes, stderr {} bytes)",
                timeout_ms,
                output.stdout.len(),
                output.stderr.len()
            );
        }

        std::thread::sleep(Duration::from_millis(10));
    }

    child.wait_with_output().context("collecting tool output")
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

    fn node_echo_script() -> &'static str {
        r#"let data = '';
process.stdin.on('data', chunk => data += chunk);
process.stdin.on('end', () => {
  const payload = JSON.parse(data || '{}');
  process.stdout.write(JSON.stringify({ input: payload, envValue: process.env.SPECIAL_TOKEN || '' }));
});
"#
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
            let (namespace, name) = split_package(package).unwrap();
            let dir = self
                .root
                .join(".agentpm")
                .join("tools")
                .join(namespace)
                .join(name)
                .join(version);
            let script_name = if manifest.contains("\"command\": \"node") {
                "script.js"
            } else {
                "script.py"
            };
            fs::write(dir.join(script_name), script).unwrap();
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
