use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[test]
fn machine_run_success_writes_single_stdout_envelope() {
    let python = available_command(&["python3", "python"]).expect("python required for tests");
    let root = TestProject::new();
    root.write_lock(lock_for("@zack/machine-echo", "0.1.0"));
    root.write_tool(
        "@zack/machine-echo",
        "0.1.0",
        schema_tool_manifest("python3"),
        python_echo_script(),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_agentpm"))
        .current_dir(root.path())
        .env("AGENTPM_PYTHON", python)
        .args([
            "run",
            "@zack/machine-echo@0.1.0",
            "--machine",
            "--input",
            r#"{"message":"hello"}"#,
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", stderr_text(&output));
    assert!(stderr_text(&output).trim().is_empty());
    let stdout = stdout_text(&output);
    let lines = stdout.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 1, "stdout: {stdout}");
    let envelope: Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(envelope["schema_version"], 1);
    assert_eq!(envelope["status"], "success");
    assert_eq!(envelope["tool"]["package"], "@zack/machine-echo");
    assert_eq!(envelope["tool"]["version"], "0.1.0");
    assert_eq!(envelope["output"]["message"], "hello");
}

#[test]
fn machine_run_accepts_json_arguments_from_stdin() {
    let python = available_command(&["python3", "python"]).expect("python required for tests");
    let root = TestProject::new();
    root.write_lock(lock_for("@zack/machine-echo", "0.1.0"));
    root.write_tool(
        "@zack/machine-echo",
        "0.1.0",
        schema_tool_manifest("python3"),
        python_echo_script(),
    );

    let mut child = Command::new(env!("CARGO_BIN_EXE_agentpm"))
        .current_dir(root.path())
        .env("AGENTPM_PYTHON", python)
        .args(["run", "@zack/machine-echo@0.1.0", "--machine"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"message":"stdin"}"#)
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(output.status.success(), "stderr: {}", stderr_text(&output));
    assert!(stderr_text(&output).trim().is_empty());
    let envelope = single_stdout_envelope(&output);
    assert_eq!(envelope["schema_version"], 1);
    assert_eq!(envelope["status"], "success");
    assert_eq!(envelope["tool"]["package"], "@zack/machine-echo");
    assert_eq!(envelope["tool"]["version"], "0.1.0");
    assert_eq!(envelope["output"]["message"], "stdin");
}

#[test]
fn machine_run_failure_writes_envelope_to_stdout_and_diagnostics_to_stderr() {
    let root = TestProject::new();
    root.write_lock(lock_for("@zack/machine-echo", "0.1.0"));
    root.write_tool(
        "@zack/machine-echo",
        "0.1.0",
        schema_tool_manifest("python3"),
        python_echo_script(),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_agentpm"))
        .current_dir(root.path())
        .args([
            "run",
            "@zack/machine-echo@0.1.0",
            "--machine",
            "--input",
            r#"{"wrong":"shape"}"#,
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = stdout_text(&output);
    let lines = stdout.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 1, "stdout: {stdout}");
    let envelope: Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(envelope["schema_version"], 1);
    assert_eq!(envelope["status"], "error");
    assert_eq!(envelope["error"]["category"], "schema");
    assert!(
        envelope["error"]["message"]
            .as_str()
            .unwrap()
            .contains("tool input failed schema validation")
    );
    assert!(stderr_text(&output).contains("tool input failed schema validation"));
}

#[test]
fn machine_input_schema_category_ignores_instance_error_marker_text() {
    let root = TestProject::new();
    root.write_lock(lock_for("@zack/numeric-input", "0.1.0"));
    root.write_tool(
        "@zack/numeric-input",
        "0.1.0",
        numeric_input_tool_manifest("python3"),
        python_echo_script(),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_agentpm"))
        .current_dir(root.path())
        .args([
            "run",
            "@zack/numeric-input@0.1.0",
            "--machine",
            "--input",
            r#"{"n":"runtime minimum version not satisfied"}"#,
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let envelope = single_stdout_envelope(&output);
    assert_eq!(envelope["status"], "error");
    assert_eq!(envelope["error"]["category"], "schema");
    assert!(
        envelope["error"]["message"]
            .as_str()
            .unwrap()
            .contains("runtime minimum version not satisfied")
    );
}

#[test]
fn machine_output_schema_category_ignores_instance_error_marker_text() {
    let root = TestProject::new();
    root.write_lock(lock_for("@zack/status-output", "0.1.0"));
    root.write_tool(
        "@zack/status-output",
        "0.1.0",
        status_output_tool_manifest("python3"),
        python_runtime_marker_output_script(),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_agentpm"))
        .current_dir(root.path())
        .args([
            "run",
            "@zack/status-output@0.1.0",
            "--machine",
            "--input",
            "{}",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let envelope = single_stdout_envelope(&output);
    assert_eq!(envelope["status"], "error");
    assert_eq!(envelope["error"]["category"], "malformed_output");
    assert!(
        envelope["error"]["message"]
            .as_str()
            .unwrap()
            .contains("runtime minimum version not satisfied")
    );
}

#[test]
fn machine_output_limit_overrun_has_stable_category() {
    let root = TestProject::new();
    root.write_lock(lock_for("@zack/large-output", "0.1.0"));
    root.write_tool(
        "@zack/large-output",
        "0.1.0",
        loose_tool_manifest("python3"),
        python_large_output_script(),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_agentpm"))
        .current_dir(root.path())
        .args([
            "run",
            "@zack/large-output@0.1.0",
            "--machine",
            "--input",
            "{}",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let envelope = single_stdout_envelope(&output);
    assert_eq!(envelope["status"], "error");
    assert_eq!(envelope["error"]["category"], "output_limit");
    assert!(
        envelope["error"]["message"]
            .as_str()
            .unwrap()
            .contains("tool output exceeded limit")
    );
}

#[cfg(unix)]
#[test]
fn terminating_machine_run_kills_nested_tool_child_process_group() {
    let root = TestProject::new();
    root.write_lock(lock_for("@zack/signal-tool", "0.1.0"));
    root.write_tool(
        "@zack/signal-tool",
        "0.1.0",
        loose_tool_manifest("python3"),
        python_nested_sleep_script(),
    );
    let child_pid_path = root
        .tool_dir("@zack/signal-tool", "0.1.0")
        .join("child.pid");

    let mut child = Command::new(env!("CARGO_BIN_EXE_agentpm"))
        .current_dir(root.path())
        .args([
            "run",
            "@zack/signal-tool@0.1.0",
            "--machine",
            "--input",
            "{}",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let nested_pid = wait_for_pid_file(&child_pid_path);
    unsafe {
        libc::kill(child.id() as i32, libc::SIGTERM);
    }
    let _ = child.wait().unwrap();

    for _ in 0..20 {
        if unsafe { libc::kill(nested_pid, 0) } != 0 {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }

    unsafe {
        libc::kill(nested_pid, libc::SIGKILL);
    }
    panic!("nested child process {nested_pid} survived parent SIGTERM");
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

fn schema_tool_manifest(command: &str) -> String {
    format!(
        r#"{{
  "kind": "tool",
  "name": "machine-echo",
  "version": "0.1.0",
  "description": "Machine echo fixture.",
  "entrypoint": {{
    "command": "{command}",
    "args": ["script.py"],
    "cwd": ".",
    "timeout_ms": 5000,
    "env": {{}}
  }},
  "runtime": {{
    "type": "python"
  }},
  "inputs": {{
    "type": "object",
    "required": ["message"],
    "properties": {{
      "message": {{ "type": "string" }}
    }}
  }},
  "outputs": {{
    "type": "object",
    "required": ["message"],
    "properties": {{
      "message": {{ "type": "string" }}
    }}
  }}
}}"#
    )
}

fn loose_tool_manifest(command: &str) -> String {
    format!(
        r#"{{
  "kind": "tool",
  "name": "signal-tool",
  "version": "0.1.0",
  "description": "Signal cleanup fixture.",
  "entrypoint": {{
    "command": "{command}",
    "args": ["script.py"],
    "cwd": ".",
    "timeout_ms": 30000,
    "env": {{}}
  }},
  "runtime": {{
    "type": "python"
  }},
  "inputs": {{ "type": "object" }},
  "outputs": {{ "type": "object" }}
}}"#
    )
}

fn numeric_input_tool_manifest(command: &str) -> String {
    format!(
        r#"{{
  "kind": "tool",
  "name": "numeric-input",
  "version": "0.1.0",
  "description": "Numeric input fixture.",
  "entrypoint": {{
    "command": "{command}",
    "args": ["script.py"],
    "cwd": ".",
    "timeout_ms": 5000,
    "env": {{}}
  }},
  "runtime": {{
    "type": "python"
  }},
  "inputs": {{
    "type": "object",
    "required": ["n"],
    "properties": {{
      "n": {{ "type": "number" }}
    }}
  }},
  "outputs": {{
    "type": "object"
  }}
}}"#
    )
}

fn status_output_tool_manifest(command: &str) -> String {
    format!(
        r#"{{
  "kind": "tool",
  "name": "status-output",
  "version": "0.1.0",
  "description": "Status output fixture.",
  "entrypoint": {{
    "command": "{command}",
    "args": ["script.py"],
    "cwd": ".",
    "timeout_ms": 5000,
    "env": {{}}
  }},
  "runtime": {{
    "type": "python"
  }},
  "inputs": {{
    "type": "object"
  }},
  "outputs": {{
    "type": "object",
    "required": ["status"],
    "properties": {{
      "status": {{ "type": "number" }}
    }}
  }}
}}"#
    )
}

fn python_echo_script() -> &'static str {
    r#"import json, sys
payload = json.load(sys.stdin)
print(json.dumps({"message": payload["message"]}))
"#
}

fn python_runtime_marker_output_script() -> &'static str {
    r#"import json, sys
json.load(sys.stdin)
print(json.dumps({"status": "runtime minimum version not satisfied"}))
"#
}

fn python_large_output_script() -> &'static str {
    r#"import json, sys
json.load(sys.stdin)
sys.stdout.write('{"payload":"' + ('x' * (1024 * 1024 + 1)) + '"}')
"#
}

fn python_nested_sleep_script() -> &'static str {
    r#"import json, pathlib, subprocess, sys, time
json.load(sys.stdin)
child = subprocess.Popen(["sleep", "30"])
pathlib.Path("child.pid").write_text(str(child.pid))
sys.stdout.flush()
time.sleep(30)
"#
}

fn stdout_text(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_text(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn single_stdout_envelope(output: &std::process::Output) -> Value {
    let stdout = stdout_text(output);
    let lines = stdout.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 1, "stdout: {stdout}");
    serde_json::from_str(lines[0]).unwrap()
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
        let root = std::env::temp_dir().join(format!("agentpm-run-machine-test-{unique}-{id}"));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write_lock(&self, content: impl AsRef<str>) {
        fs::write(self.root.join("agent.lock"), content.as_ref()).unwrap();
    }

    fn write_tool(&self, package: &str, version: &str, manifest: String, script: &str) {
        let dir = self.tool_dir(package, version);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("agent.json"), manifest).unwrap();
        fs::write(dir.join("script.py"), script).unwrap();
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

#[cfg(unix)]
fn wait_for_pid_file(path: &Path) -> i32 {
    for _ in 0..400 {
        if let Ok(raw) = fs::read_to_string(path)
            && let Ok(pid) = raw.trim().parse::<i32>()
        {
            return pid;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("timed out waiting for {}", path.display());
}
