#![allow(dead_code)]

use super::action::{
    ActionDispatchResult, ActionDispatcher, ActionFailureCategory, SemanticAction,
};
use super::model::{RuntimeSnapshot, SkillRuntimeSnapshot, ToolRuntimeSnapshot};
use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

const TOOL_CANCELLATION_POLL_INTERVAL: Duration = Duration::from_millis(25);
const TOOL_CANCELLATION_GRACE_PERIOD: Duration = Duration::from_millis(1_500);

#[derive(Debug, Clone)]
pub struct AgentPmActionDispatcher {
    workspace_root: PathBuf,
    agentpm_binary: PathBuf,
    tools: BTreeMap<String, ToolRuntimeSnapshot>,
    skills: BTreeMap<String, SkillRuntimeSnapshot>,
    cancellation_requested: Option<Arc<AtomicBool>>,
}

impl AgentPmActionDispatcher {
    pub fn from_runtime(runtime: &RuntimeSnapshot) -> Result<Self> {
        Self::with_agentpm_binary(
            runtime,
            std::env::current_exe().context("locating agentpm")?,
        )
    }

    pub fn with_agentpm_binary(runtime: &RuntimeSnapshot, agentpm_binary: PathBuf) -> Result<Self> {
        Ok(Self {
            workspace_root: runtime.workspace_root.clone(),
            agentpm_binary,
            tools: runtime
                .tools
                .iter()
                .cloned()
                .map(|tool| (tool.name.clone(), tool))
                .collect(),
            skills: runtime
                .skills
                .iter()
                .cloned()
                .map(|skill| (skill.name.clone(), skill))
                .collect(),
            cancellation_requested: None,
        })
    }

    pub fn with_cancellation_token(mut self, cancellation_requested: Arc<AtomicBool>) -> Self {
        self.cancellation_requested = Some(cancellation_requested);
        self
    }

    fn dispatch_tool(&self, tool_name: &str, arguments: &Value) -> ActionDispatchResult {
        let Some(tool) = self.tools.get(tool_name) else {
            return ActionDispatchResult::failure(format!(
                "Tool `{tool_name}` is not available in the current EffectivePhase"
            ));
        };
        let output = Command::new(&self.agentpm_binary)
            .current_dir(&self.workspace_root)
            .arg("run")
            .arg(format!("{}@{}", tool.name, tool.version))
            .arg("--machine")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                if let Some(mut stdin) = child.stdin.take() {
                    stdin.write_all(arguments.to_string().as_bytes())?;
                }
                wait_for_tool_child(child, self.cancellation_requested.as_deref())
            });
        let output = match output {
            Ok(output) => output,
            Err(err) => {
                if self
                    .cancellation_requested
                    .as_deref()
                    .is_some_and(|token| token.load(Ordering::SeqCst))
                {
                    return ActionDispatchResult::terminal_failure(
                        crate::harness_observability::HarnessTerminalStatus::Cancelled,
                        format!("ToolRuntime cancelled agentpm run for `{tool_name}`"),
                    );
                }
                return ActionDispatchResult::failure_with_category(
                    ActionFailureCategory::Runtime,
                    format!("ToolRuntime failed to invoke agentpm run for `{tool_name}`: {err}"),
                );
            }
        };
        let parsed = match serde_json::from_slice::<MachineRunEnvelope>(&output.stdout) {
            Ok(parsed) => parsed,
            Err(err) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return ActionDispatchResult::failure_with_category(
                    ActionFailureCategory::MalformedOutput,
                    format!(
                        "ToolRuntime could not parse machine output for `{tool_name}`: {err}; stderr: {stderr}"
                    ),
                );
            }
        };
        if parsed.schema_version != 1 {
            return ActionDispatchResult::failure_with_category(
                ActionFailureCategory::MalformedOutput,
                format!(
                    "ToolRuntime received unsupported machine schema_version {} for `{tool_name}`",
                    parsed.schema_version
                ),
            );
        }
        match parsed.status.as_str() {
            "success" => ActionDispatchResult::success(parsed.output.unwrap_or(Value::Null)),
            "error" => {
                let error = parsed.error.unwrap_or(MachineRunError {
                    category: "other".into(),
                    message: "agentpm run failed without a machine error payload".into(),
                });
                let category = ActionFailureCategory::from_machine_category(&error.category)
                    .unwrap_or(ActionFailureCategory::Other);
                ActionDispatchResult::failure_with_category(category, error.message)
            }
            other => ActionDispatchResult::failure_with_category(
                ActionFailureCategory::MalformedOutput,
                format!("ToolRuntime received unknown machine status `{other}` for `{tool_name}`"),
            ),
        }
    }

    fn dispatch_skill_resource(&self, skill_name: &str, resource_id: &str) -> ActionDispatchResult {
        let Some(skill) = self.skills.get(skill_name) else {
            return ActionDispatchResult::failure(format!(
                "Skill `{skill_name}` is not available in the current EffectivePhase"
            ));
        };
        let Some(resource) = skill
            .resources
            .iter()
            .find(|resource| resource.id == resource_id)
        else {
            return ActionDispatchResult::failure(format!(
                "Skill `{skill_name}` does not expose resource `{resource_id}`"
            ));
        };
        let Some(root) = &skill.root else {
            return ActionDispatchResult::failure(format!(
                "Skill `{skill_name}` root is unavailable"
            ));
        };
        let content = match read_safe_skill_resource(root, &resource.path) {
            Ok(content) => content,
            Err(err) => {
                return ActionDispatchResult::failure(format!(
                    "Skill `{skill_name}` resource `{resource_id}` could not be read: {err}"
                ));
            }
        };
        ActionDispatchResult::success(json!({
            "action_kind": "skill_resource_read",
            "ok": true,
            "skill": skill_name,
            "resource": resource_id,
            "content": content,
        }))
    }
}

fn wait_for_tool_child(
    mut child: Child,
    cancellation_requested: Option<&AtomicBool>,
) -> std::io::Result<Output> {
    loop {
        if cancellation_requested.is_some_and(|token| token.load(Ordering::SeqCst)) {
            terminate_tool_child_on_cancellation(&mut child)?;
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "ToolRuntime cancellation requested",
            ));
        }
        if child.try_wait()?.is_some() {
            return child.wait_with_output();
        }
        std::thread::sleep(TOOL_CANCELLATION_POLL_INTERVAL);
    }
}

fn terminate_tool_child_on_cancellation(child: &mut Child) -> std::io::Result<()> {
    request_graceful_tool_child_shutdown(child)?;
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        if started.elapsed() >= TOOL_CANCELLATION_GRACE_PERIOD {
            child.kill()?;
            let _ = child.wait()?;
            return Ok(());
        }
        std::thread::sleep(TOOL_CANCELLATION_POLL_INTERVAL);
    }
}

#[cfg(unix)]
fn request_graceful_tool_child_shutdown(child: &mut Child) -> std::io::Result<()> {
    let rc = unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() != std::io::ErrorKind::NotFound {
            return Err(err);
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn request_graceful_tool_child_shutdown(child: &mut Child) -> std::io::Result<()> {
    child.kill()
}

impl ActionDispatcher for AgentPmActionDispatcher {
    fn dispatch(&mut self, action: &SemanticAction) -> ActionDispatchResult {
        match action {
            SemanticAction::AgentPmTool { tool, arguments } => self.dispatch_tool(tool, arguments),
            SemanticAction::SkillResourceRead { skill, resource } => {
                self.dispatch_skill_resource(skill, resource)
            }
            SemanticAction::ExternalMcpTool { .. } => {
                ActionDispatchResult::failure("External MCP Tool runtime is not available yet")
            }
            SemanticAction::KnowledgeRequest { .. } => {
                ActionDispatchResult::failure("Knowledge runtime is not available yet")
            }
            SemanticAction::MemoryRead { .. } | SemanticAction::MemoryWrite { .. } => {
                ActionDispatchResult::failure("Memory runtime is not available yet")
            }
            SemanticAction::PhaseCompletion { .. } => {
                ActionDispatchResult::failure("Phase completion is handled by the Harness Engine")
            }
        }
    }
}

fn read_safe_skill_resource(root: &Path, relative_path: &str) -> Result<String> {
    if relative_path.is_empty()
        || relative_path.starts_with('/')
        || relative_path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(anyhow!(
            "Skill resource path must stay within the package root: {relative_path}"
        ));
    }
    let root = root
        .canonicalize()
        .with_context(|| format!("canonicalizing Skill root {}", root.display()))?;
    let path = root.join(relative_path);
    let canonical = path
        .canonicalize()
        .with_context(|| format!("canonicalizing Skill resource {}", path.display()))?;
    if !canonical.starts_with(&root) {
        return Err(anyhow!(
            "Skill resource path escapes package root: {relative_path}"
        ));
    }
    std::fs::read_to_string(&canonical)
        .with_context(|| format!("reading Skill resource {}", canonical.display()))
}

#[derive(Debug, Deserialize)]
struct MachineRunEnvelope {
    schema_version: u8,
    status: String,
    #[serde(default)]
    output: Option<Value>,
    #[serde(default)]
    error: Option<MachineRunError>,
}

#[derive(Debug, Deserialize)]
struct MachineRunError {
    category: String,
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness_runtime::model::{RuntimeSnapshot, SkillResourceSnapshot};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[test]
    fn skill_resource_loader_rejects_path_escape() {
        let temp = temp_dir("path-escape");
        std::fs::write(temp.join("inside.md"), "safe").unwrap();
        let err = read_safe_skill_resource(&temp, "../outside.md").unwrap_err();
        assert!(format!("{err:#}").contains("must stay within the package root"));
    }

    #[test]
    fn skill_resource_dispatch_reads_authorized_resource() {
        let temp = temp_dir("resource-dispatch");
        std::fs::write(temp.join("entrypoint.md"), "Use short replies.").unwrap();
        let mut runtime = RuntimeSnapshot::empty("session".into());
        runtime.workspace_root = temp.clone();
        runtime.skills.push(SkillRuntimeSnapshot {
            name: "@zack/support-style".into(),
            version: "0.1.0".into(),
            description: "Support style".into(),
            root: Some(temp.clone()),
            resources: vec![SkillResourceSnapshot {
                id: "entrypoint".into(),
                path: "entrypoint.md".into(),
                kind: "entrypoint".into(),
            }],
            state: "available".into(),
            source: "agent_binding".into(),
        });
        let mut dispatcher =
            AgentPmActionDispatcher::with_agentpm_binary(&runtime, PathBuf::from("agentpm"))
                .unwrap();
        let result = dispatcher.dispatch(&SemanticAction::SkillResourceRead {
            skill: "@zack/support-style".into(),
            resource: "entrypoint".into(),
        });
        assert!(result.ok);
        assert_eq!(result.output["content"], json!("Use short replies."));
    }

    #[test]
    fn tool_runtime_spawns_public_agentpm_run_machine_binary() {
        let agentpm = agentpm_test_binary();
        let python = available_command(&["python3", "python"]).expect("python required for tests");
        let temp = temp_dir("tool-real-agentpm-run");
        write_lock(&temp, lock_for("@zack/harness-echo", "0.1.0"));
        write_tool(
            &temp,
            "@zack/harness-echo",
            "0.1.0",
            schema_tool_manifest("harness-echo", &python),
            python_echo_script(),
        );

        let mut runtime = RuntimeSnapshot::empty("session".into());
        runtime.workspace_root = temp.clone();
        runtime.tools.push(ToolRuntimeSnapshot {
            name: "@zack/harness-echo".into(),
            version: "0.1.0".into(),
            description: "Harness echo fixture.".into(),
            root: Some(tool_dir(&temp, "@zack/harness-echo", "0.1.0")),
            input_schema: json!({
                "type": "object",
                "required": ["message"],
                "properties": {
                    "message": { "type": "string" }
                }
            }),
            state: "available".into(),
            source: "agent_binding".into(),
        });
        let mut dispatcher = AgentPmActionDispatcher::with_agentpm_binary(&runtime, agentpm)
            .expect("dispatcher should initialize");

        let result = dispatcher.dispatch(&SemanticAction::AgentPmTool {
            tool: "@zack/harness-echo".into(),
            arguments: json!({ "message": "through public agentpm run" }),
        });

        assert!(result.ok, "ToolRuntime error: {:?}", result.error);
        assert_eq!(result.output["message"], "through public agentpm run");
    }

    #[cfg(unix)]
    #[test]
    fn tool_dispatch_invokes_machine_subprocess_with_json_stdin() {
        use std::os::unix::fs::PermissionsExt;

        let temp = temp_dir("tool-subprocess");
        let stdin_path = temp.join("stdin.json");
        let script = temp.join("agentpm");
        std::fs::write(
            &script,
            format!(
                r#"#!/bin/sh
cat > "{}"
printf '%s\n' '{{"schema_version":1,"status":"success","output":{{"ok":false,"reason":"domain"}}}}'
"#,
                stdin_path.display()
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).unwrap();

        let mut runtime = RuntimeSnapshot::empty("session".into());
        runtime.workspace_root = temp.clone();
        runtime.tools.push(ToolRuntimeSnapshot {
            name: "@zack/domain-check".into(),
            version: "0.1.0".into(),
            description: "Check a domain condition.".into(),
            root: Some(temp.clone()),
            input_schema: json!({ "type": "object", "additionalProperties": true }),
            state: "available".into(),
            source: "agent_binding".into(),
        });
        let mut dispatcher =
            AgentPmActionDispatcher::with_agentpm_binary(&runtime, script).unwrap();

        let result = dispatcher.dispatch(&SemanticAction::AgentPmTool {
            tool: "@zack/domain-check".into(),
            arguments: json!({ "case": "domain-false" }),
        });

        assert!(result.ok);
        assert_eq!(result.output["ok"], json!(false));
        assert_eq!(
            std::fs::read_to_string(stdin_path).unwrap(),
            json!({ "case": "domain-false" }).to_string()
        );
    }

    #[cfg(unix)]
    #[test]
    fn tool_dispatch_preserves_machine_error_category() {
        use std::os::unix::fs::PermissionsExt;

        let temp = temp_dir("tool-machine-error");
        let script = temp.join("agentpm");
        std::fs::write(
            &script,
            r#"#!/bin/sh
printf '%s\n' '{"schema_version":1,"status":"error","error":{"category":"schema","message":"invalid arguments"}}'
"#,
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).unwrap();

        let mut runtime = RuntimeSnapshot::empty("session".into());
        runtime.workspace_root = temp.clone();
        runtime.tools.push(ToolRuntimeSnapshot {
            name: "@zack/schema-tool".into(),
            version: "0.1.0".into(),
            description: "Schema failure tool.".into(),
            root: Some(temp),
            input_schema: json!({ "type": "object", "additionalProperties": true }),
            state: "available".into(),
            source: "agent_binding".into(),
        });
        let mut dispatcher =
            AgentPmActionDispatcher::with_agentpm_binary(&runtime, script).unwrap();

        let result = dispatcher.dispatch(&SemanticAction::AgentPmTool {
            tool: "@zack/schema-tool".into(),
            arguments: json!({}),
        });

        assert!(!result.ok);
        assert_eq!(result.failure_category, Some(ActionFailureCategory::Schema));
        assert_eq!(result.error.as_deref(), Some("invalid arguments"));
    }

    #[cfg(unix)]
    #[test]
    fn tool_runtime_cancellation_sends_catchable_signal_before_kill() {
        let temp = temp_dir("tool-cancellation-signal");
        let marker = temp.join("term-seen");
        let child = Command::new("sh")
            .arg("-c")
            .arg(r#"trap 'printf term > "$MARKER"; exit 0' TERM; while :; do sleep 1; done"#)
            .env("MARKER", &marker)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let token = Arc::new(AtomicBool::new(false));
        let waiter_token = Arc::clone(&token);
        let waiter =
            std::thread::spawn(move || wait_for_tool_child(child, Some(waiter_token.as_ref())));

        std::thread::sleep(Duration::from_millis(100));
        token.store(true, Ordering::SeqCst);
        let err = waiter.join().unwrap().unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::Interrupted);
        assert_eq!(std::fs::read_to_string(marker).unwrap(), "term");
    }

    #[cfg(unix)]
    #[test]
    fn tool_dispatch_rejects_unsupported_machine_schema_version() {
        use std::os::unix::fs::PermissionsExt;

        let temp = temp_dir("tool-machine-version");
        let script = temp.join("agentpm");
        std::fs::write(
            &script,
            r#"#!/bin/sh
printf '%s\n' '{"schema_version":2,"status":"success","output":{"ok":true}}'
"#,
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).unwrap();

        let mut runtime = RuntimeSnapshot::empty("session".into());
        runtime.workspace_root = temp.clone();
        runtime.tools.push(ToolRuntimeSnapshot {
            name: "@zack/versioned-tool".into(),
            version: "0.1.0".into(),
            description: "Versioned tool.".into(),
            root: Some(temp),
            input_schema: json!({ "type": "object", "additionalProperties": true }),
            state: "available".into(),
            source: "agent_binding".into(),
        });
        let mut dispatcher =
            AgentPmActionDispatcher::with_agentpm_binary(&runtime, script).unwrap();

        let result = dispatcher.dispatch(&SemanticAction::AgentPmTool {
            tool: "@zack/versioned-tool".into(),
            arguments: json!({}),
        });

        assert!(!result.ok);
        assert_eq!(
            result.failure_category,
            Some(ActionFailureCategory::MalformedOutput)
        );
        assert!(
            result
                .error
                .as_deref()
                .unwrap()
                .contains("unsupported machine schema_version 2")
        );
    }

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("agentpm-harness-runtime-{label}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn agentpm_test_binary() -> PathBuf {
        if let Ok(path) = std::env::var("CARGO_BIN_EXE_agentpm") {
            return PathBuf::from(path);
        }
        let exe = std::env::current_exe().expect("current test executable");
        let debug_dir = exe
            .parent()
            .and_then(|parent| {
                if parent.file_name().and_then(|name| name.to_str()) == Some("deps") {
                    parent.parent()
                } else {
                    Some(parent)
                }
            })
            .expect("debug target directory");
        let binary = debug_dir.join(format!("agentpm{}", std::env::consts::EXE_SUFFIX));
        assert!(
            binary.exists(),
            "expected built agentpm binary at {}",
            binary.display()
        );
        binary
    }

    fn available_command(candidates: &[&str]) -> Option<String> {
        candidates.iter().find_map(|candidate| {
            Command::new(candidate)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .ok()
                .filter(|status| status.success())
                .map(|_| (*candidate).to_string())
        })
    }

    fn write_lock(root: &Path, content: String) {
        std::fs::write(root.join("agent.lock"), content).unwrap();
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

    fn write_tool(root: &Path, package: &str, version: &str, manifest: String, script: &str) {
        let dir = tool_dir(root, package, version);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("agent.json"), manifest).unwrap();
        std::fs::write(dir.join("script.py"), script).unwrap();
    }

    fn tool_dir(root: &Path, package: &str, version: &str) -> PathBuf {
        let (namespace, name) = split_package(package);
        root.join(".agentpm")
            .join("tools")
            .join(namespace)
            .join(name)
            .join(version)
    }

    fn split_package(package: &str) -> (String, String) {
        let mut parts = package.trim_start_matches('@').splitn(2, '/');
        (
            parts.next().unwrap().to_string(),
            parts.next().unwrap().to_string(),
        )
    }

    fn schema_tool_manifest(name: &str, command: &str) -> String {
        format!(
            r#"{{
  "kind": "tool",
  "name": "{name}",
  "version": "0.1.0",
  "description": "Harness ToolRuntime fixture.",
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

    fn python_echo_script() -> &'static str {
        r#"import json, sys
payload = json.load(sys.stdin)
print(json.dumps({"message": payload["message"]}))
"#
    }
}
