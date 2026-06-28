use crate::assets::{
    AGENT_JSON_TPL, SKILL_AGENT_JSON_TPL, SKILL_INIT_MD_TPL, TEMPLATE_GENERATED_README_MD_TPL,
    TEMPLATE_PACKAGE_AGENT_JSON_TPL, TOOL_AGENT_JSON_TPL,
};
use crate::io::fs::write_atomic;
use crate::prelude::*;
use std::path::PathBuf;

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum InitKind {
    Tool,
    Agent,
    Template,
    Skill,
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// What to scaffold: a single-tool package, a composed agent, a workflow template, or a skill
    #[arg(long, value_enum, default_value = "tool")]
    kind: InitKind,

    /// Name for the package being initialized
    #[arg(long, default_value = "my-tool")]
    name: String,

    /// Description for the package being initialized
    #[arg(long, default_value = "Starter AgentPM project")]
    description: String,

    /// Output directory (defaults to current dir)
    #[arg(long)]
    out_dir: Option<PathBuf>,
}

impl InitArgs {
    pub async fn run(self, _base_url: String) -> Result<()> {
        let out = self.out_dir.unwrap_or(std::env::current_dir()?);
        match self.kind {
            InitKind::Tool => {
                let rendered = render(
                    TOOL_AGENT_JSON_TPL,
                    &[
                        ("TOOL_NAME", &self.name),
                        ("TOOL_DESCRIPTION", &self.description),
                    ],
                );
                let path = out.join("agent.json");
                write_atomic(&path, &rendered)?;
                println!("Created {}", path.display());
            }
            InitKind::Agent => {
                let rendered = render(
                    AGENT_JSON_TPL,
                    &[
                        ("AGENT_NAME", &self.name),
                        ("AGENT_DESCRIPTION", &self.description),
                    ],
                );
                let path = out.join("agent.json");
                write_atomic(&path, &rendered)?;
                println!("Created {}", path.display());
            }
            InitKind::Template => {
                let display_name = humanize_name(&self.name);
                let rendered = render(
                    TEMPLATE_PACKAGE_AGENT_JSON_TPL,
                    &[
                        ("TEMPLATE_NAME", &self.name),
                        ("TEMPLATE_DESCRIPTION", &self.description),
                        ("TEMPLATE_DISPLAY_NAME", &display_name),
                        ("PROJECT_NAME_DEFAULT", &self.name),
                    ],
                );
                let path = out.join("agent.json");
                write_atomic(&path, &rendered)?;
                println!("Created {}", path.display());

                let scaffold_dir = out.join("template");
                std::fs::create_dir_all(&scaffold_dir)?;

                let generated_readme = render(
                    TEMPLATE_GENERATED_README_MD_TPL,
                    &[
                        ("PROJECT_DISPLAY_NAME", &display_name),
                        ("TEMPLATE_NAME", &self.name),
                    ],
                );

                let generated_readme_path = scaffold_dir.join("README.md");
                write_atomic(&generated_readme_path, &generated_readme)?;

                println!("Created {}", generated_readme_path.display());
            }
            InitKind::Skill => {
                let rendered = render(
                    SKILL_AGENT_JSON_TPL,
                    &[
                        ("SKILL_NAME", &self.name),
                        ("SKILL_DESCRIPTION", &self.description),
                    ],
                );
                let path = out.join("agent.json");
                write_atomic(&path, &rendered)?;
                println!("Created {}", path.display());

                let title = humanize_name(&self.name);
                let skill_markdown = render(
                    SKILL_INIT_MD_TPL,
                    &[
                        ("SKILL_NAME", &self.name),
                        ("SKILL_TITLE", &title),
                        ("SKILL_DESCRIPTION", &self.description),
                    ],
                );
                let skill_path = out.join("SKILL.md");
                write_atomic(&skill_path, &skill_markdown)?;
                println!("Created {}", skill_path.display());
            }
        }
        Ok(())
    }
}

/// Naive renderer: replaces {{KEY}} with value.
fn render(tpl: &str, vars: &[(&str, &str)]) -> String {
    let mut out = tpl.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{{{}}}}}", k), v);
    }
    out
}

fn humanize_name(name: &str) -> String {
    name.split('-')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => {
                    let mut out = first.to_uppercase().to_string();
                    out.push_str(chars.as_str());
                    out
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{load_manifest_value, validate_manifest_value};
    use serde_json::Value;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("agentpm-init-{label}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn schema_path() -> String {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../schemas/agentpm.manifest.schema.json")
            .to_string_lossy()
            .into_owned()
    }

    fn validate(path: &Path) {
        let schema_source = schema_path();
        let (mut value, _) = load_manifest_value(path).unwrap();
        let (ok, issues) =
            validate_manifest_value(&schema_source, &path.to_string_lossy(), &mut value, false)
                .unwrap();
        assert!(
            ok,
            "expected {} to validate, got {issues:#?}",
            path.display()
        );
    }

    #[tokio::test]
    async fn init_template_creates_valid_template_package_and_scaffold() {
        let out = temp_dir("template");
        let args = InitArgs {
            kind: InitKind::Template,
            name: "research-assistant-python".into(),
            description: "Python SDK starter for a local research assistant.".into(),
            out_dir: Some(out.clone()),
        };

        args.run("http://example.invalid".into()).await.unwrap();

        let package_manifest = out.join("agent.json");
        let scaffold_readme = out.join("template").join("README.md");

        assert!(package_manifest.exists());
        assert!(scaffold_readme.exists());
        assert!(!out.join("template").join("agent.json").exists());

        validate(&package_manifest);

        let package_json: Value =
            serde_json::from_str(&std::fs::read_to_string(&package_manifest).unwrap()).unwrap();
        assert_eq!(package_json["kind"], "template");
        assert_eq!(package_json["template"]["files_root"], "template");
        assert_eq!(
            package_json["template"]["variables"][0]["name"],
            "project_name"
        );

        let scaffold_readme_text = std::fs::read_to_string(&scaffold_readme).unwrap();
        assert!(
            scaffold_readme_text.contains("{ { project_name } }"),
            "expected scaffold README to demonstrate render-time project_name placeholder syntax"
        );

        let _ = std::fs::remove_dir_all(out);
    }

    #[tokio::test]
    async fn init_skill_creates_valid_manifest_and_skill_markdown() {
        let out = temp_dir("skill");
        let args = InitArgs {
            kind: InitKind::Skill,
            name: "incident-commander".into(),
            description: "Incident response coordination playbook.".into(),
            out_dir: Some(out.clone()),
        };

        args.run("http://example.invalid".into()).await.unwrap();

        let manifest_path = out.join("agent.json");
        let skill_path = out.join("SKILL.md");

        assert!(manifest_path.exists());
        assert!(skill_path.exists());
        assert!(!out.join("references").exists());
        assert!(!out.join("scripts").exists());

        validate(&manifest_path);

        let manifest_json: Value =
            serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
        assert_eq!(manifest_json["kind"], "skill");
        assert_eq!(manifest_json["skill"]["entrypoint"], "SKILL.md");
        assert!(manifest_json.get("tools").is_none());

        let skill_markdown = std::fs::read_to_string(&skill_path).unwrap();
        assert!(skill_markdown.contains("name: incident-commander"));
        assert!(skill_markdown.contains("description: Incident response coordination playbook."));
        assert!(skill_markdown.contains("# Incident Commander"));
        assert!(skill_markdown.contains("Incident response coordination playbook."));
        assert!(!skill_markdown.contains("{{SKILL_NAME}}"));

        let _ = std::fs::remove_dir_all(out);
    }

    #[tokio::test]
    async fn init_tool_agent_and_template_behavior_still_work() {
        for (kind, label, expected_kind) in [
            (InitKind::Tool, "tool", "tool"),
            (InitKind::Agent, "agent", "agent"),
            (InitKind::Template, "template", "template"),
        ] {
            let out = temp_dir(label);
            let args = InitArgs {
                kind,
                name: format!("demo-{label}"),
                description: format!("Starter {label}."),
                out_dir: Some(out.clone()),
            };

            args.run("http://example.invalid".into()).await.unwrap();
            let manifest_path = out.join("agent.json");
            assert!(manifest_path.exists());

            let manifest_json: Value =
                serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
            assert_eq!(manifest_json["kind"], expected_kind);
            if expected_kind == "tool" {
                assert!(manifest_json.get("files").is_some());
                assert!(manifest_json.get("entrypoint").is_some());
                assert!(manifest_json.get("inputs").is_some());
                assert!(manifest_json.get("outputs").is_some());
            }
            if expected_kind == "agent" || expected_kind == "template" {
                validate(&manifest_path);
            }
            if expected_kind == "template" {
                assert!(out.join("template").join("README.md").exists());
            }

            let _ = std::fs::remove_dir_all(out);
        }
    }
}
