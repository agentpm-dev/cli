use crate::assets::{EXAMPLES_MD_TPL, RUN_SH_TPL, SKILL_MD_TPL, TOOL_CONTRACT_MD_TPL};
use crate::prelude::*;
use crate::runner::{ResolvedTool, parse_tool_spec, resolve_installed_tool};
use anyhow::{Context, bail};
use serde_json::Value;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

#[derive(Args, Debug, Clone)]
pub struct ExportArgs {
    /// Generate a starter skill scaffold from the installed tool
    #[arg(long, value_name = "PACKAGE_REF")]
    pub skill: String,

    /// Override the output directory (default: skills/<tool-name>)
    #[arg(long, value_name = "PATH")]
    pub output: Option<PathBuf>,

    /// Overwrite an existing output directory
    #[arg(long)]
    pub force: bool,
}

impl ExportArgs {
    pub async fn run(self, _base_url: String) -> Result<()> {
        let project_dir = std::env::current_dir().context("reading current directory")?;
        self.run_with_dir(&project_dir)
    }

    fn run_with_dir(self, project_dir: &Path) -> Result<()> {
        let spec = parse_tool_spec(&self.skill)?;
        let resolved = resolve_installed_tool(project_dir, &spec)?;
        let namespace_collisions = if self.output.is_none() {
            find_default_output_namespace_collisions(project_dir, &resolved)?
        } else {
            Vec::new()
        };
        let output_dir = self
            .output
            .clone()
            .unwrap_or_else(|| default_output_dir(project_dir, &resolved));

        if output_dir.exists() {
            if !self.force {
                bail!(
                    "output directory already exists: {} (pass --force to overwrite)",
                    output_dir.display()
                );
            }
            remove_existing_path(&output_dir)?;
        }

        write_skill_scaffold(&output_dir, &resolved)?;
        if !namespace_collisions.is_empty() {
            eprintln!(
                "Warning: the default output path {} is based only on the tool name. Also installed with this leaf name: {}. Use --output to avoid namespace collisions.",
                output_dir.display(),
                namespace_collisions.join(", ")
            );
        }
        eprintln!("Generated skill scaffold at {}", output_dir.display());
        Ok(())
    }
}

fn default_output_dir(project_dir: &Path, resolved: &ResolvedTool) -> PathBuf {
    project_dir
        .join("skills")
        .join(slugify_tool_name(&resolved.package))
}

fn find_default_output_namespace_collisions(
    project_dir: &Path,
    resolved: &ResolvedTool,
) -> Result<Vec<String>> {
    let package_ref = resolved.package.as_str();
    let Some((namespace, name)) = package_ref.trim_start_matches('@').split_once('/') else {
        return Ok(Vec::new());
    };

    let tools_root = project_dir.join(".agentpm").join("tools");
    if !tools_root.exists() {
        return Ok(Vec::new());
    }

    let mut collisions = Vec::new();
    for entry in
        fs::read_dir(&tools_root).with_context(|| format!("reading {}", tools_root.display()))?
    {
        let entry = entry.with_context(|| format!("reading {}", tools_root.display()))?;
        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }
        let other_namespace = entry.file_name().to_string_lossy().to_string();
        if other_namespace == namespace {
            continue;
        }
        if entry_path.join(name).exists() {
            collisions.push(format!("@{other_namespace}/{name}"));
        }
    }
    collisions.sort();
    Ok(collisions)
}

fn slugify_tool_name(package_ref: &str) -> String {
    package_ref
        .trim_start_matches('@')
        .split_once('/')
        .map(|(_, name)| name.to_string())
        .unwrap_or_else(|| package_ref.trim_start_matches('@').to_string())
}

fn remove_existing_path(path: &Path) -> Result<()> {
    let metadata = fs::metadata(path).with_context(|| format!("reading {}", path.display()))?;
    if metadata.is_dir() {
        fs::remove_dir_all(path).with_context(|| format!("removing {}", path.display()))?;
    } else {
        fs::remove_file(path).with_context(|| format!("removing {}", path.display()))?;
    }
    Ok(())
}

fn write_skill_scaffold(output_dir: &Path, resolved: &ResolvedTool) -> Result<()> {
    let references_dir = output_dir.join("references");
    let scripts_dir = output_dir.join("scripts");
    fs::create_dir_all(&references_dir)
        .with_context(|| format!("creating {}", references_dir.display()))?;
    fs::create_dir_all(&scripts_dir)
        .with_context(|| format!("creating {}", scripts_dir.display()))?;

    let skill_md = render_skill_md(resolved);
    let contract_md = render_tool_contract_md(resolved)?;
    let examples_md = render_examples_md(resolved)?;
    let run_sh = render_run_script(resolved);

    fs::write(output_dir.join("SKILL.md"), skill_md)
        .with_context(|| format!("writing {}", output_dir.join("SKILL.md").display()))?;
    fs::write(references_dir.join("tool-contract.md"), contract_md).with_context(|| {
        format!(
            "writing {}",
            references_dir.join("tool-contract.md").display()
        )
    })?;
    fs::write(references_dir.join("examples.md"), examples_md)
        .with_context(|| format!("writing {}", references_dir.join("examples.md").display()))?;
    fs::write(scripts_dir.join("run.sh"), run_sh)
        .with_context(|| format!("writing {}", scripts_dir.join("run.sh").display()))?;
    mark_run_script_executable(&scripts_dir.join("run.sh"))?;
    Ok(())
}

fn mark_run_script_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(path)
            .with_context(|| format!("reading {}", path.display()))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)
            .with_context(|| format!("updating permissions for {}", path.display()))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

fn render_skill_md(resolved: &ResolvedTool) -> String {
    let package_ref = &resolved.package;
    let tool_name = &resolved.manifest.name;
    let title = title_case(tool_name);
    let resolved_version = resolved.version.to_string();
    let skill_name = slugify_tool_name(package_ref);
    let skill_description = format!(
        "Use this skill when you want to run the {} tool through AgentPM from a skill-capable client while keeping execution delegated to agentpm run.",
        package_ref
    );
    let description = resolved
        .manifest
        .description
        .as_deref()
        .unwrap_or("No description provided in the tool manifest.");
    render_template(
        SKILL_MD_TPL,
        &[
            ("SKILL_NAME", skill_name.as_str()),
            ("SKILL_DESCRIPTION", skill_description.as_str()),
            ("TITLE", title.as_str()),
            ("PACKAGE_REF", package_ref.as_str()),
            ("RESOLVED_VERSION", resolved_version.as_str()),
            ("DESCRIPTION", description),
        ],
    )
}

fn render_tool_contract_md(resolved: &ResolvedTool) -> Result<String> {
    let runtime = match &resolved.manifest.runtime {
        Some(runtime) => match &runtime.version {
            Some(version) => format!("{} ({})", runtime.runtime_type, version),
            None => runtime.runtime_type.clone(),
        },
        None => "Not declared".to_string(),
    };
    let entrypoint_args = serde_json::to_string_pretty(&resolved.manifest.entrypoint.args)
        .context("formatting entrypoint args as JSON")?;

    let environment = render_environment_requirements(resolved);
    let input_schema = format_optional_schema(&resolved.manifest.inputs)
        .context("formatting input schema as JSON")?;
    let output_schema = format_optional_schema(&resolved.manifest.outputs)
        .context("formatting output schema as JSON")?;

    let resolved_version = resolved.version.to_string();
    let timeout_ms = resolved.manifest.entrypoint.timeout_ms.to_string();
    Ok(render_template(
        TOOL_CONTRACT_MD_TPL,
        &[
            ("PACKAGE_REF", resolved.package.as_str()),
            ("RESOLVED_VERSION", resolved_version.as_str()),
            ("MANIFEST_NAME", resolved.manifest.name.as_str()),
            ("MANIFEST_VERSION", resolved.manifest.version.as_str()),
            (
                "DESCRIPTION",
                resolved
                    .manifest
                    .description
                    .as_deref()
                    .unwrap_or("No description provided in the tool manifest."),
            ),
            ("RUNTIME", runtime.as_str()),
            (
                "ENTRYPOINT_COMMAND",
                resolved.manifest.entrypoint.command.as_str(),
            ),
            ("ENTRYPOINT_ARGS", entrypoint_args.as_str()),
            ("ENTRYPOINT_CWD", resolved.manifest.entrypoint.cwd.as_str()),
            ("TIMEOUT_MS", timeout_ms.as_str()),
            ("ENVIRONMENT", environment.as_str()),
            ("INPUT_SCHEMA", input_schema.as_str()),
            ("OUTPUT_SCHEMA", output_schema.as_str()),
        ],
    ))
}

fn render_environment_requirements(resolved: &ResolvedTool) -> String {
    let Some(environment) = &resolved.manifest.environment else {
        return "No environment requirements declared.".to_string();
    };
    if environment.vars.is_empty() {
        return "No environment requirements declared.".to_string();
    }

    let mut lines = Vec::new();
    for (name, rule) in &environment.vars {
        let mut parts = Vec::new();
        if rule.required {
            parts.push("required".to_string());
        } else {
            parts.push("optional".to_string());
        }
        if let Some(default) = &rule.default {
            parts.push(format!("default: `{default}`"));
        }
        lines.push(format!("- `{name}` — {}", parts.join(", ")));
    }
    lines.join("\n")
}

fn render_examples_md(resolved: &ResolvedTool) -> Result<String> {
    let package_ref = &resolved.package;
    let pretty_example = example_input_from_schema(&resolved.manifest.inputs);
    let inline_input =
        serde_json::to_string(&pretty_example).context("formatting inline example JSON")?;
    let pretty_input =
        serde_json::to_string_pretty(&pretty_example).context("formatting example payload")?;

    Ok(render_template(
        EXAMPLES_MD_TPL,
        &[
            ("PACKAGE_REF", package_ref.as_str()),
            ("INLINE_INPUT", inline_input.as_str()),
            ("PRETTY_INPUT", pretty_input.as_str()),
        ],
    ))
}

fn render_run_script(resolved: &ResolvedTool) -> String {
    render_template(RUN_SH_TPL, &[("PACKAGE_REF", resolved.package.as_str())])
}

fn example_input_from_schema(schema: &Value) -> Value {
    if let Some(example) = schema.get("example") {
        return example.clone();
    }
    if let Some(default) = schema.get("default") {
        return default.clone();
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("object") => {
            let mut map = serde_json::Map::new();
            if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
                for (name, property) in properties {
                    map.insert(name.clone(), example_input_from_schema(property));
                }
            }
            Value::Object(map)
        }
        Some("array") => Value::Array(Vec::new()),
        Some("string") => Value::String("TODO".to_string()),
        Some("integer") => Value::from(0),
        Some("number") => Value::from(0),
        Some("boolean") => Value::Bool(true),
        _ => Value::Object(Default::default()),
    }
}

fn format_optional_schema(schema: &Value) -> Result<String> {
    if schema.is_null() || matches!(schema, Value::Object(map) if map.is_empty()) {
        Ok("Not declared.".to_string())
    } else {
        serde_json::to_string_pretty(schema).context("formatting schema as JSON")
    }
}

fn title_case(raw: &str) -> String {
    raw.split(['-', '_', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_template(tpl: &str, vars: &[(&str, &str)]) -> String {
    let mut out = tpl.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{{{}}}}}", k), v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn generates_expected_file_structure() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            rich_tool_manifest("python3", "0.1.0"),
            python_echo_script(),
        );

        let args = ExportArgs {
            skill: "@zack/echo-json".to_string(),
            output: None,
            force: false,
        };

        args.run_with_dir(root.path()).unwrap();

        let skill_dir = root.path().join("skills").join("echo-json");
        assert!(skill_dir.join("SKILL.md").exists());
        assert!(skill_dir.join("references/tool-contract.md").exists());
        assert!(skill_dir.join("references/examples.md").exists());
        assert!(skill_dir.join("scripts/run.sh").exists());

        #[cfg(unix)]
        {
            let mode = fs::metadata(skill_dir.join("scripts/run.sh"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111);
        }
    }

    #[test]
    fn refuses_to_overwrite_existing_output_without_force() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            rich_tool_manifest("python3", "0.1.0"),
            python_echo_script(),
        );

        let args = ExportArgs {
            skill: "@zack/echo-json".to_string(),
            output: None,
            force: false,
        };
        args.clone().run_with_dir(root.path()).unwrap();

        let err = args.run_with_dir(root.path()).unwrap_err();
        assert!(
            format!("{err:#}").contains("output directory already exists"),
            "{err:#}"
        );
    }

    #[test]
    fn overwrites_existing_output_with_force() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            rich_tool_manifest("python3", "0.1.0"),
            python_echo_script(),
        );

        let first = ExportArgs {
            skill: "@zack/echo-json".to_string(),
            output: None,
            force: false,
        };
        first.run_with_dir(root.path()).unwrap();

        let skill_dir = root.path().join("skills").join("echo-json");
        fs::write(skill_dir.join("SKILL.md"), "stale content").unwrap();

        let second = ExportArgs {
            skill: "@zack/echo-json".to_string(),
            output: None,
            force: true,
        };
        second.run_with_dir(root.path()).unwrap();

        let skill_md = fs::read_to_string(skill_dir.join("SKILL.md")).unwrap();
        assert!(skill_md.starts_with("---\nname: echo-json\n"));
        assert!(skill_md.contains("When to use this skill"));
        assert!(!skill_md.contains("stale content"));
    }

    #[test]
    fn writes_manifest_derived_content() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            rich_tool_manifest("python3", "0.1.0"),
            python_echo_script(),
        );

        let args = ExportArgs {
            skill: "@zack/echo-json".to_string(),
            output: None,
            force: false,
        };
        args.run_with_dir(root.path()).unwrap();

        let skill_dir = root.path().join("skills").join("echo-json");
        let skill_md = fs::read_to_string(skill_dir.join("SKILL.md")).unwrap();
        let contract_md =
            fs::read_to_string(skill_dir.join("references/tool-contract.md")).unwrap();
        let examples_md = fs::read_to_string(skill_dir.join("references/examples.md")).unwrap();

        assert!(skill_md.starts_with("---\nname: echo-json\n"));
        assert!(skill_md.contains("description: Use this skill when you want to run the @zack/echo-json tool through AgentPM from a skill-capable client while keeping execution delegated to agentpm run."));
        assert!(skill_md.contains("When to use this skill"));
        assert!(skill_md.contains("agentpm run @zack/echo-json"));
        assert!(skill_md.contains("TODO: Add the specific workflow cues"));
        assert!(contract_md.contains("Echo tool for skill export tests"));
        assert!(contract_md.contains("\"message\""));
        assert!(contract_md.contains("`API_TOKEN` — required"));
        assert!(examples_md.contains("./scripts/run.sh"));
    }

    #[test]
    fn supports_output_override() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            rich_tool_manifest("python3", "0.1.0"),
            python_echo_script(),
        );

        let custom = root.path().join("custom-skill-output");
        let args = ExportArgs {
            skill: "@zack/echo-json".to_string(),
            output: Some(custom.clone()),
            force: false,
        };
        args.run_with_dir(root.path()).unwrap();

        assert!(custom.join("SKILL.md").exists());
        assert!(custom.join("references/tool-contract.md").exists());
    }

    #[test]
    fn detects_default_output_namespace_collisions() {
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            rich_tool_manifest("python3", "0.1.0"),
            python_echo_script(),
        );
        root.write_tool(
            "@acme/echo-json",
            "0.2.0",
            rich_tool_manifest("python3", "0.2.0"),
            python_echo_script(),
        );

        let spec = parse_tool_spec("@zack/echo-json").expect("parse tool spec");
        let resolved = resolve_installed_tool(root.path(), &spec).expect("resolve installed tool");

        let collisions = find_default_output_namespace_collisions(root.path(), &resolved)
            .expect("detect namespace collisions");

        assert_eq!(collisions, vec!["@acme/echo-json"]);
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
  "name": "echo-json",
  "version": "{version}",
  "description": "Echo tool for skill export tests",
  "entrypoint": {{
    "command": "{command}",
    "args": ["script.py"],
    "cwd": ".",
    "timeout_ms": 5000,
    "env": {{
      "MODE": "skill"
    }}
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
      "upper": {{
        "type": "string"
      }}
    }},
    "required": ["upper"]
  }}
}}"#
        )
    }

    fn python_echo_script() -> String {
        r#"import json
import sys

payload = json.load(sys.stdin)
json.dump({"upper": payload.get("message", "").upper()}, sys.stdout)
"#
        .to_string()
    }

    struct TestProject {
        root: PathBuf,
    }

    impl TestProject {
        fn new() -> Self {
            let mut root = std::env::temp_dir();
            let unique = format!(
                "agentpm-export-test-{}-{}",
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
