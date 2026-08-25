use crate::prelude::*;
use crate::runner::{
    RunOptions, RunnerErrorKind, classify_runner_error, parse_tool_spec, run_installed_tool,
};
use anyhow::{Context, anyhow, bail};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::io::{IsTerminal, Read};
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
pub struct RunArgs {
    /// Tool spec to execute, e.g. @namespace/name, @namespace/name@0.1.0, @namespace/name@latest
    pub spec: String,

    /// Inline JSON input payload
    #[arg(long, conflicts_with = "input_file")]
    pub input: Option<String>,

    /// Path to a JSON input payload file
    #[arg(long, value_name = "PATH", conflicts_with = "input")]
    pub input_file: Option<PathBuf>,

    /// Override the manifest/default timeout for this invocation
    #[arg(long)]
    pub timeout_ms: Option<u64>,

    /// Emit a stable machine-readable JSON result envelope on stdout
    #[arg(long)]
    pub machine: bool,
}

impl RunArgs {
    pub async fn run(self, _base_url: String) -> Result<()> {
        let project_dir = std::env::current_dir().context("reading current directory")?;
        let stdin_is_terminal = std::io::stdin().is_terminal();
        let stdin_bytes = if self.input.is_none() && self.input_file.is_none() && !stdin_is_terminal
        {
            let mut stdin = std::io::stdin().lock();
            let mut buf = Vec::new();
            stdin
                .read_to_end(&mut buf)
                .context("reading JSON input from stdin")?;
            buf
        } else {
            Vec::new()
        };

        if self.machine {
            self.run_machine(project_dir, &stdin_bytes, stdin_is_terminal)
                .await
        } else {
            let input = self.parse_input_value(&stdin_bytes, stdin_is_terminal)?;
            let spec = parse_tool_spec(&self.spec)?;
            let options = self.build_run_options();
            let project_dir_cl = project_dir.clone();
            let stdout = tokio::task::spawn_blocking(move || {
                let result = run_installed_tool(&project_dir_cl, &spec, &input, &options)?;
                Ok::<String, anyhow::Error>(format!("{}\n", serde_json::to_string(&result.output)?))
            })
            .await
            .context("joining run worker thread")??;
            print!("{stdout}");
            Ok(())
        }
    }

    async fn run_machine(
        self,
        project_dir: PathBuf,
        stdin_bytes: &[u8],
        stdin_is_terminal: bool,
    ) -> Result<()> {
        let input = match self.parse_input_value(stdin_bytes, stdin_is_terminal) {
            Ok(input) => input,
            Err(err) => return self.write_machine_error(err),
        };
        let spec = match parse_tool_spec(&self.spec) {
            Ok(spec) => spec,
            Err(err) => return self.write_machine_error(err),
        };
        let options = self.build_run_options();
        let result = tokio::task::spawn_blocking(move || {
            run_installed_tool(&project_dir, &spec, &input, &options)
        })
        .await
        .context("joining run worker thread")
        .and_then(|inner| inner);

        match result {
            Ok(result) => {
                println!(
                    "{}",
                    serde_json::to_string(&json!({
                        "schema_version": 1,
                        "status": "success",
                        "tool": {
                            "package": result.resolved.package,
                            "version": result.resolved.version.to_string()
                        },
                        "output": result.output
                    }))?
                );
                Ok(())
            }
            Err(err) => self.write_machine_error(err),
        }
    }

    fn write_machine_error(&self, err: anyhow::Error) -> Result<()> {
        let category = runner_error_category(&err);
        println!(
            "{}",
            serde_json::to_string(&json!({
                "schema_version": 1,
                "status": "error",
                "error": {
                    "category": category,
                    "message": format!("{err:#}")
                }
            }))?
        );
        eprintln!("{err:#}");
        Err(anyhow!(
            "agentpm run --machine failed with {category} error"
        ))
    }

    fn parse_input_value(&self, stdin_bytes: &[u8], stdin_is_terminal: bool) -> Result<Value> {
        match (&self.input, &self.input_file) {
            (Some(input), None) => {
                serde_json::from_str::<Value>(input).context("invalid JSON provided via --input")
            }
            (None, Some(path)) => {
                let raw = fs::read_to_string(path)
                    .with_context(|| format!("reading JSON input file {}", path.display()))?;
                serde_json::from_str::<Value>(&raw)
                    .with_context(|| format!("invalid JSON in input file {}", path.display()))
            }
            (None, None) => {
                if stdin_is_terminal {
                    bail!(
                        "no JSON input provided; pass --input, --input-file, or pipe JSON to stdin"
                    );
                }
                if stdin_bytes.is_empty() {
                    bail!("no JSON input received on stdin");
                }
                serde_json::from_slice::<Value>(stdin_bytes)
                    .context("invalid JSON provided via stdin")
            }
            _ => unreachable!("clap conflicts prevent --input and --input-file together"),
        }
    }

    fn build_run_options(&self) -> RunOptions {
        RunOptions {
            timeout_ms: self.timeout_ms,
            env_overrides: HashMap::new(),
            #[cfg(unix)]
            cleanup_child_process_group_on_signal: true,
            ..RunOptions::default()
        }
    }
}

fn runner_error_category(err: &anyhow::Error) -> &'static str {
    match classify_runner_error(err) {
        RunnerErrorKind::Resolution => "resolution",
        RunnerErrorKind::Runtime => "runtime",
        RunnerErrorKind::Schema => "schema",
        RunnerErrorKind::Timeout => "timeout",
        RunnerErrorKind::OutputLimit => "output_limit",
        RunnerErrorKind::MalformedOutput => "malformed_output",
        RunnerErrorKind::SubprocessFailure => "subprocess_failure",
        RunnerErrorKind::Other => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{RunResult, ToolSpec};
    use std::path::Path;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    #[cfg(unix)]
    #[test]
    fn run_surface_opts_into_scoped_signal_cleanup() {
        assert!(!RunOptions::default().cleanup_child_process_group_on_signal);

        let args = RunArgs {
            spec: "@zack/echo-json".to_string(),
            input: Some(r#"{"message":"hi"}"#.to_string()),
            input_file: None,
            timeout_ms: None,
            machine: false,
        };

        assert!(
            args.build_run_options()
                .cleanup_child_process_group_on_signal
        );
    }

    #[test]
    fn uses_stdin_input() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            tool_manifest("python3"),
            python_echo_script(),
        );

        let args = RunArgs {
            spec: "@zack/echo-json".to_string(),
            input: None,
            input_file: None,
            timeout_ms: None,
            machine: false,
        };

        let stdout = run_with(
            &args,
            root.path(),
            br#"{"message":"hi"}"#,
            false,
            run_installed_tool,
        )
        .unwrap();

        let json: Value = serde_json::from_str(stdout.trim()).unwrap();
        assert_eq!(json["input"]["message"], "hi");
    }

    #[test]
    fn rejects_empty_non_terminal_stdin() {
        let args = RunArgs {
            spec: "@zack/echo-json".to_string(),
            input: None,
            input_file: None,
            timeout_ms: None,
            machine: false,
        };

        let err = args.parse_input_value(&[], false).unwrap_err();
        assert!(
            format!("{err:#}").contains("no JSON input received on stdin"),
            "{err:#}"
        );
    }

    #[test]
    fn uses_inline_input() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            tool_manifest("python3"),
            python_echo_script(),
        );

        let args = RunArgs {
            spec: "@zack/echo-json".to_string(),
            input: Some(r#"{"message":"inline"}"#.to_string()),
            input_file: None,
            timeout_ms: None,
            machine: false,
        };

        let stdout = run_with(&args, root.path(), &[], true, run_installed_tool).unwrap();

        let json: Value = serde_json::from_str(stdout.trim()).unwrap();
        assert_eq!(json["input"]["message"], "inline");
    }

    #[test]
    fn uses_input_file() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            tool_manifest("python3"),
            python_echo_script(),
        );
        let payload = root.path().join("payload.json");
        fs::write(&payload, r#"{"message":"file"}"#).unwrap();

        let args = RunArgs {
            spec: "@zack/echo-json".to_string(),
            input: None,
            input_file: Some(payload),
            timeout_ms: None,
            machine: false,
        };

        let stdout = run_with(&args, root.path(), &[], true, run_installed_tool).unwrap();

        let json: Value = serde_json::from_str(stdout.trim()).unwrap();
        assert_eq!(json["input"]["message"], "file");
    }

    #[test]
    fn rejects_invalid_inline_json() {
        let args = RunArgs {
            spec: "@zack/echo-json".to_string(),
            input: Some("{bad json}".to_string()),
            input_file: None,
            timeout_ms: None,
            machine: false,
        };

        let err = args.parse_input_value(&[], true).unwrap_err();
        assert!(
            format!("{err:#}").contains("invalid JSON provided via --input"),
            "{err:#}"
        );
    }

    #[test]
    fn rejects_invalid_json_input_file() {
        let root = TestProject::new();
        let payload = root.path().join("bad-payload.json");
        fs::write(&payload, "{bad json}").unwrap();

        let args = RunArgs {
            spec: "@zack/echo-json".to_string(),
            input: None,
            input_file: Some(payload.clone()),
            timeout_ms: None,
            machine: false,
        };

        let err = args.parse_input_value(&[], true).unwrap_err();
        assert!(
            format!("{err:#}")
                .contains(&format!("invalid JSON in input file {}", payload.display())),
            "{err:#}"
        );
    }

    #[test]
    fn resolves_unversioned_locked_spec() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            tool_manifest("python3"),
            python_echo_script(),
        );

        let args = RunArgs {
            spec: "@zack/echo-json".to_string(),
            input: Some(r#"{"message":"locked"}"#.to_string()),
            input_file: None,
            timeout_ms: None,
            machine: false,
        };

        let stdout = run_with(&args, root.path(), &[], true, run_installed_tool).unwrap();
        let json: Value = serde_json::from_str(stdout.trim()).unwrap();
        assert_eq!(json["input"]["message"], "locked");
    }

    #[test]
    fn resolves_exact_version() {
        let root = TestProject::new();
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            tool_manifest("python3"),
            python_echo_script_with_version("0.1.0"),
        );
        root.write_tool(
            "@zack/echo-json",
            "0.2.0",
            tool_manifest("python3"),
            python_echo_script_with_version("0.2.0"),
        );

        let args = RunArgs {
            spec: "@zack/echo-json@0.1.0".to_string(),
            input: Some(r#"{"message":"exact"}"#.to_string()),
            input_file: None,
            timeout_ms: None,
            machine: false,
        };

        let stdout = run_with(&args, root.path(), &[], true, run_installed_tool).unwrap();
        let json: Value = serde_json::from_str(stdout.trim()).unwrap();
        assert_eq!(json["version"], "0.1.0");
    }

    #[test]
    fn surfaces_missing_tool_errors() {
        let root = TestProject::new();
        let args = RunArgs {
            spec: "@zack/missing-tool@0.1.0".to_string(),
            input: Some("{}".to_string()),
            input_file: None,
            timeout_ms: None,
            machine: false,
        };

        let err = run_with(&args, root.path(), &[], true, run_installed_tool).unwrap_err();
        assert!(
            format!("{err:#}").contains("installed tool not found"),
            "{err:#}"
        );
    }

    #[test]
    fn surfaces_missing_required_env_errors() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/requires-env", "0.1.0"));
        root.write_tool(
            "@zack/requires-env",
            "0.1.0",
            tool_manifest_with_env("python3", true, None),
            python_echo_script(),
        );

        let args = RunArgs {
            spec: "@zack/requires-env".to_string(),
            input: Some("{}".to_string()),
            input_file: None,
            timeout_ms: None,
            machine: false,
        };

        let err = run_with(&args, root.path(), &[], true, run_installed_tool).unwrap_err();
        assert!(
            format!("{err:#}").contains("missing required environment variables"),
            "{err:#}"
        );
    }

    #[test]
    fn surfaces_timeout_errors() {
        let python = available_command(&["python3", "python"]).expect("python required for tests");
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/slow-tool", "0.1.0"));
        root.write_tool(
            "@zack/slow-tool",
            "0.1.0",
            tool_manifest("python3"),
            python_sleep_script(),
        );

        let args = RunArgs {
            spec: "@zack/slow-tool".to_string(),
            input: Some("{}".to_string()),
            input_file: None,
            timeout_ms: Some(100),
            machine: false,
        };

        let err = run_with(
            &args,
            root.path(),
            &[],
            true,
            move |project_dir, spec, input, options| {
                let mut options = options.clone();
                options
                    .env_overrides
                    .insert("AGENTPM_PYTHON".to_string(), python.clone());
                run_installed_tool(project_dir, spec, input, &options)
            },
        )
        .unwrap_err();
        assert!(format!("{err:#}").contains("timed out"), "{err:#}");
    }

    #[test]
    fn accepts_latest_and_range_specs() {
        let root = TestProject::new();
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            tool_manifest("python3"),
            python_echo_script_with_version("0.1.0"),
        );
        root.write_tool(
            "@zack/echo-json",
            "0.1.5",
            tool_manifest("python3"),
            python_echo_script_with_version("0.1.5"),
        );
        root.write_tool(
            "@zack/echo-json",
            "0.2.0",
            tool_manifest("python3"),
            python_echo_script_with_version("0.2.0"),
        );

        let latest = RunArgs {
            spec: "@zack/echo-json@latest".to_string(),
            input: Some("{}".to_string()),
            input_file: None,
            timeout_ms: None,
            machine: false,
        };
        let latest = run_with(&latest, root.path(), &[], true, run_installed_tool).unwrap();
        let latest_json: Value = serde_json::from_str(latest.trim()).unwrap();
        assert_eq!(latest_json["version"], "0.2.0");

        let range = RunArgs {
            spec: "@zack/echo-json@^0.1".to_string(),
            input: Some("{}".to_string()),
            input_file: None,
            timeout_ms: None,
            machine: false,
        };
        let range = run_with(&range, root.path(), &[], true, run_installed_tool).unwrap();
        let range_json: Value = serde_json::from_str(range.trim()).unwrap();
        assert_eq!(range_json["version"], "0.1.5");
    }

    #[test]
    fn executes_node_tool_via_run_command() {
        let node = available_command(&["node", "nodejs"]).expect("node required for tests");
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-node", "0.1.0"));
        root.write_tool(
            "@zack/echo-node",
            "0.1.0",
            tool_manifest("node"),
            node_echo_script(),
        );

        let args = RunArgs {
            spec: "@zack/echo-node".to_string(),
            input: Some(r#"{"message":"node"}"#.to_string()),
            input_file: None,
            timeout_ms: None,
            machine: false,
        };

        let stdout = run_with(
            &args,
            root.path(),
            &[],
            true,
            move |project_dir, spec, input, options| {
                let mut options = options.clone();
                options
                    .env_overrides
                    .insert("AGENTPM_NODE".to_string(), node.clone());
                run_installed_tool(project_dir, spec, input, &options)
            },
        )
        .unwrap();
        let json: Value = serde_json::from_str(stdout.trim()).unwrap();
        assert_eq!(json["input"]["message"], "node");
    }

    fn run_with<F>(
        args: &RunArgs,
        project_dir: &Path,
        stdin_bytes: &[u8],
        stdin_is_terminal: bool,
        invoke: F,
    ) -> Result<String>
    where
        F: FnOnce(&Path, &ToolSpec, &Value, &RunOptions) -> Result<RunResult>,
    {
        let input = args.parse_input_value(stdin_bytes, stdin_is_terminal)?;
        let spec = parse_tool_spec(&args.spec)?;
        let options = args.build_run_options();
        let result = invoke(project_dir, &spec, &input, &options)?;
        Ok(format!("{}\n", serde_json::to_string(&result.output)?))
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

    fn python_echo_script() -> String {
        python_echo_script_with_version("")
    }

    fn python_echo_script_with_version(version: &str) -> String {
        if version.is_empty() {
            r#"import json, os, sys
payload = json.load(sys.stdin)
print(json.dumps({"input": payload, "envValue": os.environ.get("SPECIAL_TOKEN", "")}))
"#
            .to_string()
        } else {
            format!(
                r#"import json, os, sys
payload = json.load(sys.stdin)
print(json.dumps({{"input": payload, "envValue": os.environ.get("SPECIAL_TOKEN", ""), "version": "{version}"}}))
"#
            )
        }
    }

    fn python_sleep_script() -> &'static str {
        r#"import json, sys, time
json.load(sys.stdin)
time.sleep(1.0)
print(json.dumps({"ok": True}))
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
            let root = std::env::temp_dir().join(format!("agentpm-run-cmd-test-{unique}-{id}"));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn path(&self) -> &Path {
            &self.root
        }

        fn write_lock(&self, content: impl AsRef<str>) {
            fs::write(self.root.join("agent.lock"), content.as_ref()).unwrap();
        }

        fn write_tool(
            &self,
            package: &str,
            version: &str,
            manifest: String,
            script: impl AsRef<str>,
        ) {
            let dir = self.tool_dir(package, version);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("agent.json"), &manifest).unwrap();
            let script_name = if manifest.contains("\"command\": \"node") {
                "script.js"
            } else {
                "script.py"
            };
            fs::write(dir.join(script_name), script.as_ref()).unwrap();
        }

        fn tool_dir(&self, package: &str, version: &str) -> PathBuf {
            let (namespace, name) = split_package(package);
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

    fn split_package(package: &str) -> (String, String) {
        let mut parts = package.trim_start_matches('@').splitn(2, '/');
        (
            parts.next().unwrap().to_string(),
            parts.next().unwrap().to_string(),
        )
    }
}
