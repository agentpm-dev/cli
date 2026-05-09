//! Internal adapter-facing seam for ecosystem integrations such as MCP.
//!
//! This module intentionally stays crate-private. It gives future adapters a
//! stable internal way to inspect locked installed tools and invoke them
//! through the shared runner without turning that seam into a public plugin API.
//! Public third-party plugins and WASM-based adapter loading are future work.

use crate::manifest::read_lock_or_default;
use crate::runner::{
    EnvVarDecl, EnvironmentDecl, RunOptions, RunResult, RunnerManifest, RuntimeDecl, ToolSelector,
    ToolSpec, resolve_installed_tool, run_installed_tool,
};
use anyhow::{Context, Result};
use semver::Version;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct AdapterToolDescriptor {
    pub package_ref: String,
    pub resolved_version: String,
    pub manifest_name: String,
    pub manifest_version: String,
    pub description: Option<String>,
    pub input_schema: Value,
    pub output_schema: Value,
    pub environment_requirements: HashMap<String, AdapterEnvRequirement>,
    pub runtime: Option<RuntimeDecl>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterEnvRequirement {
    pub required: bool,
    pub default: Option<String>,
}

#[allow(dead_code)]
pub fn list_locked_tool_descriptors(project_dir: &Path) -> Result<Vec<AdapterToolDescriptor>> {
    let lock = read_lock_or_default(project_dir)?;
    lock.dependencies
        .iter()
        .map(|(package_ref, dependency)| {
            let version = Version::parse(&dependency.version).with_context(|| {
                format!("locked version for {} is not valid semver", package_ref)
            })?;
            let spec = ToolSpec {
                package: package_ref.clone(),
                selector: ToolSelector::Exact(version),
            };
            let resolved = resolve_installed_tool(project_dir, &spec)?;
            Ok(descriptor_from_manifest(
                package_ref,
                &dependency.version,
                &resolved.manifest,
            ))
        })
        .collect()
}

#[allow(dead_code)]
pub fn invoke_descriptor(
    project_dir: &Path,
    descriptor: &AdapterToolDescriptor,
    input: &Value,
    options: &RunOptions,
) -> Result<RunResult> {
    // Synchronous — call through `tokio::task::spawn_blocking` from async contexts
    // such as the future MCP server handler.
    let spec = ToolSpec {
        package: descriptor.package_ref.clone(),
        selector: ToolSelector::Exact(Version::parse(&descriptor.resolved_version).with_context(
            || {
                format!(
                    "descriptor version for {} is not valid semver",
                    descriptor.package_ref
                )
            },
        )?),
    };
    run_installed_tool(project_dir, &spec, input, options)
}

fn descriptor_from_manifest(
    package_ref: &str,
    resolved_version: &str,
    manifest: &RunnerManifest,
) -> AdapterToolDescriptor {
    AdapterToolDescriptor {
        package_ref: package_ref.to_string(),
        resolved_version: resolved_version.to_string(),
        manifest_name: manifest.name.clone(),
        manifest_version: manifest.version.clone(),
        description: manifest.description.clone(),
        input_schema: manifest.inputs.clone(),
        output_schema: manifest.outputs.clone(),
        environment_requirements: adapter_env_requirements(manifest.environment.as_ref()),
        runtime: manifest.runtime.clone(),
    }
}

fn adapter_env_requirements(
    environment: Option<&EnvironmentDecl>,
) -> HashMap<String, AdapterEnvRequirement> {
    environment
        .map(|decl| {
            decl.vars
                .iter()
                .map(|(key, value)| (key.clone(), adapter_env_requirement(value)))
                .collect()
        })
        .unwrap_or_default()
}

fn adapter_env_requirement(value: &EnvVarDecl) -> AdapterEnvRequirement {
    AdapterEnvRequirement {
        required: value.required,
        default: value.default.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn returns_empty_descriptors_when_lockfile_is_missing() {
        let root = TestProject::new();

        let descriptors = list_locked_tool_descriptors(root.path()).unwrap();

        assert!(descriptors.is_empty());
    }

    #[test]
    fn generates_locked_tool_descriptors() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/adapter-echo", "0.1.0"));
        root.write_tool(
            "@zack/adapter-echo",
            "0.1.0",
            rich_tool_manifest("python3", "0.1.0"),
            python_versioned_echo_script("0.1.0"),
        );

        let descriptors = list_locked_tool_descriptors(root.path()).unwrap();
        assert_eq!(descriptors.len(), 1);

        let descriptor = &descriptors[0];
        assert_eq!(descriptor.package_ref, "@zack/adapter-echo");
        assert_eq!(descriptor.resolved_version, "0.1.0");
        assert_eq!(descriptor.manifest_name, "adapter-echo");
        assert_eq!(descriptor.manifest_version, "0.1.0");
        assert_eq!(
            descriptor.description.as_deref(),
            Some("Adapter echo tool for tests")
        );
        assert_eq!(descriptor.input_schema["type"], "object");
        assert_eq!(descriptor.output_schema["type"], "object");
        assert!(
            descriptor
                .environment_requirements
                .get("API_TOKEN")
                .unwrap()
                .required
        );
        assert_eq!(
            descriptor
                .environment_requirements
                .get("REGION")
                .unwrap()
                .default
                .as_deref(),
            Some("us-west-2")
        );
        assert_eq!(
            descriptor.runtime.as_ref().map(|r| r.runtime_type.as_str()),
            Some("python")
        );
    }

    #[test]
    fn invokes_descriptor_through_shared_runner_path() {
        let python = available_command(&["python3", "python"]).expect("python required for tests");
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/adapter-echo", "0.1.0"));
        root.write_tool(
            "@zack/adapter-echo",
            "0.1.0",
            rich_tool_manifest("python3", "0.1.0"),
            python_versioned_echo_script("0.1.0"),
        );
        root.write_tool(
            "@zack/adapter-echo",
            "0.2.0",
            rich_tool_manifest("python3", "0.2.0"),
            python_versioned_echo_script("0.2.0"),
        );

        let descriptor = list_locked_tool_descriptors(root.path()).unwrap().remove(0);
        let mut options = RunOptions::default();
        options
            .env_overrides
            .insert("AGENTPM_PYTHON".to_string(), python);
        options
            .env_overrides
            .insert("API_TOKEN".to_string(), "token-123".to_string());

        let result = invoke_descriptor(
            root.path(),
            &descriptor,
            &serde_json::json!({"message":"hi"}),
            &options,
        )
        .unwrap();

        assert_eq!(result.resolved.version, Version::parse("0.1.0").unwrap());
        assert_eq!(result.output["toolVersion"], "0.1.0");
        assert_eq!(result.output["apiToken"], "token-123");
        assert_eq!(result.output["region"], "us-west-2");
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

    fn rich_tool_manifest(command: &str, version: &str) -> String {
        format!(
            r#"{{
  "kind": "tool",
  "name": "adapter-echo",
  "version": "{version}",
  "description": "Adapter echo tool for tests",
  "entrypoint": {{
    "command": "{command}",
    "args": ["script.py"],
    "cwd": ".",
    "timeout_ms": 5000,
    "env": {{}}
  }},
  "runtime": {{
    "type": "python",
    "version": ">=3.10"
  }},
  "environment": {{
    "vars": {{
      "API_TOKEN": {{
        "required": true
      }},
      "REGION": {{
        "default": "us-west-2"
      }}
    }}
  }},
  "inputs": {{
    "type": "object",
    "properties": {{
      "message": {{
        "type": "string"
      }}
    }},
    "required": ["message"]
  }},
  "outputs": {{
    "type": "object",
    "properties": {{
      "toolVersion": {{
        "type": "string"
      }},
      "input": {{
        "type": "object"
      }}
    }},
    "required": ["toolVersion", "input"]
  }}
}}"#
        )
    }

    fn python_versioned_echo_script(version: &str) -> String {
        format!(
            r#"import json
import os
import sys

payload = json.load(sys.stdin)
json.dump(
    {{
        "toolVersion": "{version}",
        "input": payload,
        "apiToken": os.environ.get("API_TOKEN", ""),
        "region": os.environ.get("REGION", "")
    }},
    sys.stdout,
)
"#
        )
    }

    struct TestProject {
        root: PathBuf,
    }

    impl TestProject {
        fn new() -> Self {
            let mut root = std::env::temp_dir();
            let unique = format!(
                "agentpm-adapter-test-{}-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            );
            root.push(unique);
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn path(&self) -> &Path {
            &self.root
        }

        fn write_lock(&self, lock: String) {
            fs::write(self.root.join("agent.lock"), lock).unwrap();
        }

        fn write_tool(&self, package: &str, version: &str, manifest: String, script: String) {
            let dir = self.tool_dir(package, version);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("agent.json"), manifest).unwrap();
            fs::write(dir.join("script.py"), script).unwrap();
        }

        fn tool_dir(&self, package: &str, version: &str) -> PathBuf {
            let trimmed = package.trim_start_matches('@');
            let (namespace, name) = trimmed.split_once('/').unwrap();
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
