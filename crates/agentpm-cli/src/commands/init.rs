use crate::assets::{
    AGENT_JSON_TPL, KNOWLEDGE_CONTEXT_AGENT_JSON_TPL, KNOWLEDGE_CONTEXT_README_MD_TPL,
    KNOWLEDGE_VECTOR_AGENT_JSON_TPL, KNOWLEDGE_VECTOR_README_MD_TPL, LOOP_AGENT_JSON_TPL,
    LOOP_README_MD_TPL, MEMORY_AGENT_JSON_TPL, MEMORY_README_MD_TPL, MEMORY_SCHEMA_JSON_TPL,
    PROFILE_AGENT_JSON_TPL, PROFILE_README_MD_TPL, SKILL_AGENT_JSON_TPL, SKILL_INIT_MD_TPL,
    TEMPLATE_GENERATED_README_MD_TPL, TEMPLATE_PACKAGE_AGENT_JSON_TPL, TOOL_AGENT_JSON_TPL,
    TOOL_MAIN_PY_TPL,
};
use crate::io::fs::{ensure_dirs, write_atomic};
use crate::prelude::*;
use anyhow::bail;
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum InitKind {
    Tool,
    Agent,
    Template,
    Skill,
    Knowledge,
    Memory,
    Profile,
    Loop,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum KnowledgeInitMode {
    Context,
    Vector,
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Package kind (defaults to an Agent Package)
    #[arg(long, value_enum, default_value = "agent")]
    kind: InitKind,

    /// Knowledge mode when --kind knowledge: direct context documents or prepared vector corpus
    #[arg(long, value_enum, default_value = "context")]
    mode: KnowledgeInitMode,

    /// Local package name; defaults to my-agent, or my-<kind> for an explicit Component/Template
    #[arg(
        long,
        default_value = "my-agent",
        default_value_if("kind", "tool", "my-tool"),
        default_value_if("kind", "skill", "my-skill"),
        default_value_if("kind", "knowledge", "my-knowledge"),
        default_value_if("kind", "memory", "my-memory"),
        default_value_if("kind", "profile", "my-profile"),
        default_value_if("kind", "loop", "my-loop"),
        default_value_if("kind", "template", "my-template")
    )]
    name: String,

    /// Package purpose; the default describes the selected scaffold
    #[arg(
        long,
        default_value = "A reusable Agent Package defining an agent's capabilities and behavior.",
        default_value_if("kind", "tool", "Echo one JSON input as JSON output."),
        default_value_if("kind", "skill", "Instructions for completing a repeatable task."),
        default_value_if("kind", "knowledge", "Reference material for an agent system."),
        default_value_if("kind", "memory", "A durable memory contract for an agent system."),
        default_value_if("kind", "profile", "Behavioral guidance for an agent system."),
        default_value_if("kind", "loop", "An assessment, execution and review workflow."),
        default_value_if("kind", "template", "A starter project for an agent system.")
    )]
    description: String,

    /// Output directory (defaults to current dir)
    #[arg(long)]
    out_dir: Option<PathBuf>,
}

impl InitArgs {
    pub async fn run(self, _base_url: String) -> Result<()> {
        if !matches!(self.kind, InitKind::Knowledge)
            && !matches!(self.mode, KnowledgeInitMode::Context)
        {
            bail!("`--mode` is only applicable when `--kind knowledge` is used");
        }

        let out = self.out_dir.unwrap_or(std::env::current_dir()?);
        preflight_output(&out, &self.kind, &self.mode)?;
        match self.kind {
            InitKind::Tool => {
                let source_path = out.join("main.py");
                let create_source = !source_path.exists();
                if !create_source && !source_path.is_file() {
                    bail!("{} already exists and is not a file", source_path.display());
                }
                let rendered = render_manifest(
                    TOOL_AGENT_JSON_TPL,
                    &[
                        ("TOOL_NAME", &self.name),
                        ("TOOL_DESCRIPTION", &self.description),
                    ],
                )?;
                let path = out.join("agent.json");
                write_atomic(&path, &rendered)?;
                println!("Created {}", path.display());
                if create_source {
                    write_atomic(&source_path, TOOL_MAIN_PY_TPL)?;
                    println!("Created {}", source_path.display());
                } else {
                    println!("Kept existing {}", source_path.display());
                }
            }
            InitKind::Agent => {
                let rendered = render_manifest(
                    AGENT_JSON_TPL,
                    &[
                        ("AGENT_NAME", &self.name),
                        ("AGENT_DESCRIPTION", &self.description),
                    ],
                )?;
                let path = out.join("agent.json");
                write_atomic(&path, &rendered)?;
                println!("Created {}", path.display());
                println!("Agent Package created. Add Components as needed, then run agentpm lint.");
                println!(
                    "To run it with agentpm harness, first add a Loop dependency and configure a model."
                );
            }
            InitKind::Template => {
                let display_name = humanize_name(&self.name);
                let rendered = render_manifest(
                    TEMPLATE_PACKAGE_AGENT_JSON_TPL,
                    &[
                        ("TEMPLATE_NAME", &self.name),
                        ("TEMPLATE_DESCRIPTION", &self.description),
                        ("TEMPLATE_DISPLAY_NAME", &display_name),
                        ("PROJECT_NAME_DEFAULT", &self.name),
                    ],
                )?;
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
                let rendered = render_manifest(
                    SKILL_AGENT_JSON_TPL,
                    &[
                        ("SKILL_NAME", &self.name),
                        ("SKILL_DESCRIPTION", &self.description),
                    ],
                )?;
                let path = out.join("agent.json");
                write_atomic(&path, &rendered)?;
                println!("Created {}", path.display());

                let title = humanize_name(&self.name);
                // A JSON string is also a quoted YAML scalar, including escaped
                // newlines and quotes in authored descriptions.
                let description = serde_json::to_string(&self.description)?;
                let skill_markdown = render(
                    SKILL_INIT_MD_TPL,
                    &[
                        ("SKILL_NAME", &self.name),
                        ("SKILL_TITLE", &title),
                        ("SKILL_DESCRIPTION", &description),
                    ],
                );
                let skill_path = out.join("SKILL.md");
                write_atomic(&skill_path, &skill_markdown)?;
                println!("Created {}", skill_path.display());
            }
            InitKind::Knowledge => {
                let rendered = match self.mode {
                    KnowledgeInitMode::Context => render_manifest(
                        KNOWLEDGE_CONTEXT_AGENT_JSON_TPL,
                        &[
                            ("KNOWLEDGE_NAME", &self.name),
                            ("KNOWLEDGE_DESCRIPTION", &self.description),
                        ],
                    )?,
                    KnowledgeInitMode::Vector => render_manifest(
                        KNOWLEDGE_VECTOR_AGENT_JSON_TPL,
                        &[
                            ("KNOWLEDGE_NAME", &self.name),
                            ("KNOWLEDGE_DESCRIPTION", &self.description),
                        ],
                    )?,
                };
                let path = out.join("agent.json");
                write_atomic(&path, &rendered)?;
                println!("Created {}", path.display());

                let title = humanize_name(&self.name);
                let readme = match self.mode {
                    KnowledgeInitMode::Context => render(
                        KNOWLEDGE_CONTEXT_README_MD_TPL,
                        &[("KNOWLEDGE_TITLE", &title)],
                    ),
                    KnowledgeInitMode::Vector => render(
                        KNOWLEDGE_VECTOR_README_MD_TPL,
                        &[("KNOWLEDGE_TITLE", &title)],
                    ),
                };
                let readme_path = out.join("README.md");
                write_atomic(&readme_path, &readme)?;
                println!("Created {}", readme_path.display());

                match self.mode {
                    KnowledgeInitMode::Context => {
                        let context_doc = format!(
                            "# {title}\n\nThis is a starter context document for the `{}` Knowledge package.\n",
                            self.name
                        );
                        let context_path = out.join("knowledge").join("docs").join("context.md");
                        write_atomic(&context_path, &context_doc)?;
                        println!("Created {}", context_path.display());
                    }
                    KnowledgeInitMode::Vector => {
                        let chunks_path = out.join("knowledge").join("chunks.jsonl");
                        let sources_path = out.join("knowledge").join("sources.jsonl");
                        let embeddings_dir = out.join("knowledge").join("embeddings");
                        write_atomic(&chunks_path, "")?;
                        write_atomic(&sources_path, "")?;
                        ensure_dirs(std::slice::from_ref(&embeddings_dir))?;
                        println!("Created {}", chunks_path.display());
                        println!("Created {}", sources_path.display());
                        println!("Created {}", embeddings_dir.display());
                    }
                }
            }
            InitKind::Memory => {
                let rendered = render_manifest(
                    MEMORY_AGENT_JSON_TPL,
                    &[
                        ("MEMORY_NAME", &self.name),
                        ("MEMORY_DESCRIPTION", &self.description),
                    ],
                )?;
                let path = out.join("agent.json");
                write_atomic(&path, &rendered)?;
                println!("Created {}", path.display());

                let title = humanize_name(&self.name);
                let readme = render(MEMORY_README_MD_TPL, &[("MEMORY_TITLE", &title)]);
                let readme_path = out.join("README.md");
                write_atomic(&readme_path, &readme)?;
                println!("Created {}", readme_path.display());

                let schema_path = out.join("schemas").join("user-preference.schema.json");
                write_atomic(&schema_path, MEMORY_SCHEMA_JSON_TPL)?;
                println!("Created {}", schema_path.display());
            }
            InitKind::Profile => {
                let rendered = render_manifest(
                    PROFILE_AGENT_JSON_TPL,
                    &[
                        ("PROFILE_NAME", &self.name),
                        ("PROFILE_DESCRIPTION", &self.description),
                    ],
                )?;
                let path = out.join("agent.json");
                write_atomic(&path, &rendered)?;
                println!("Created {}", path.display());

                let title = humanize_name(&self.name);
                let readme = render(PROFILE_README_MD_TPL, &[("PROFILE_TITLE", &title)]);
                let readme_path = out.join("README.md");
                write_atomic(&readme_path, &readme)?;
                println!("Created {}", readme_path.display());
            }
            InitKind::Loop => {
                let rendered = render_manifest(
                    LOOP_AGENT_JSON_TPL,
                    &[
                        ("LOOP_NAME", &self.name),
                        ("LOOP_DESCRIPTION", &self.description),
                    ],
                )?;
                let path = out.join("agent.json");
                write_atomic(&path, &rendered)?;
                println!("Created {}", path.display());

                let title = humanize_name(&self.name);
                let readme = render(LOOP_README_MD_TPL, &[("LOOP_TITLE", &title)]);
                let readme_path = out.join("README.md");
                write_atomic(&readme_path, &readme)?;
                println!("Created {}", readme_path.display());
            }
        }
        Ok(())
    }
}

/// Render text resources. JSON manifests use render_manifest instead.
fn render(tpl: &str, vars: &[(&str, &str)]) -> String {
    let mut out = tpl.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{{{}}}}}", k), v);
    }
    out
}

fn render_manifest(tpl: &str, vars: &[(&str, &str)]) -> Result<String> {
    fn replace(value: &mut Value, vars: &[(&str, &str)]) {
        match value {
            Value::String(text) => {
                if let Some((_, replacement)) = vars
                    .iter()
                    .find(|(key, _)| *text == format!("{{{{{key}}}}}"))
                {
                    *text = replacement.to_string();
                }
            }
            Value::Array(items) => items.iter_mut().for_each(|item| replace(item, vars)),
            Value::Object(fields) => fields.values_mut().for_each(|item| replace(item, vars)),
            _ => {}
        }
    }
    let mut value: Value = serde_json::from_str(tpl)?;
    replace(&mut value, vars);
    validate_scaffold_manifest(&value)?;
    // Keep author-facing identity fields near the top without enabling JSON
    // preserve_order globally (historical signature serialization relies on it
    // remaining disabled).
    #[derive(Serialize)]
    struct Scaffold<'a> {
        #[serde(rename = "$schema")]
        schema: &'a Value,
        standard: &'a Value,
        kind: &'a Value,
        name: &'a Value,
        version: &'a Value,
        description: &'a Value,
        #[serde(flatten)]
        content: std::collections::BTreeMap<&'a str, &'a Value>,
    }
    let common = [
        "$schema",
        "standard",
        "kind",
        "name",
        "version",
        "description",
    ];
    let scaffold = Scaffold {
        schema: &value["$schema"],
        standard: &value["standard"],
        kind: &value["kind"],
        name: &value["name"],
        version: &value["version"],
        description: &value["description"],
        content: value
            .as_object()
            .unwrap()
            .iter()
            .filter(|(key, _)| !common.contains(&key.as_str()))
            .map(|(key, value)| (key.as_str(), value))
            .collect(),
    };
    Ok(serde_json::to_string_pretty(&scaffold)? + "\n")
}

pub(super) fn validate_scaffold_manifest(value: &Value) -> Result<()> {
    use crate::manifest::apds::{Context, Level, Status, evaluate};
    let result = evaluate(value, "agent.json", Level::Standalone, Context::default())?;
    if result.status != Status::Conformant {
        if let Some(issue) = result.issues.first() {
            bail!(
                "Cannot create package: {} at {} ({})",
                issue.message,
                issue.instance_path,
                issue.rule_id.as_deref().unwrap_or("APDS")
            );
        }
        bail!("Cannot create package: unsupported scaffold standard");
    }
    Ok(())
}

/// Check the complete output set before any file is written. Existing Tool
/// source is intentionally retained; other generated files are never replaced.
fn preflight_output(out: &Path, kind: &InitKind, mode: &KnowledgeInitMode) -> Result<()> {
    let mut files = vec!["agent.json"];
    match kind {
        InitKind::Agent => {}
        InitKind::Tool => files.push("main.py"),
        InitKind::Skill => files.push("SKILL.md"),
        InitKind::Template => files.push("template/README.md"),
        InitKind::Knowledge => {
            files.push("README.md");
            match mode {
                KnowledgeInitMode::Context => files.push("knowledge/docs/context.md"),
                KnowledgeInitMode::Vector => {
                    files.extend(["knowledge/chunks.jsonl", "knowledge/sources.jsonl"]);
                }
            }
        }
        InitKind::Memory => files.extend(["README.md", "schemas/user-preference.schema.json"]),
        InitKind::Profile | InitKind::Loop => files.push("README.md"),
    }
    fn directories(path: &Path, out: &Path) -> Result<()> {
        for parent in path.ancestors().filter(|p| !p.as_os_str().is_empty()) {
            match std::fs::symlink_metadata(parent) {
                Ok(metadata) => {
                    if !parent.is_dir()
                        || metadata.file_type().is_symlink()
                            && parent != out
                            && parent.starts_with(out)
                    {
                        bail!("{} is not a safe scaffold directory", parent.display());
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
    for relative in files {
        let path = out.join(relative);
        directories(path.parent().unwrap_or(out), out)?;
        let keep_source = matches!(kind, InitKind::Tool) && relative == "main.py";
        match std::fs::symlink_metadata(&path) {
            Ok(_) if keep_source && path.is_file() => continue,
            Ok(_) => bail!(
                "{} already exists; init does not overwrite existing files",
                path.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        // The existing atomic writer uses this staging name. Protect preexisting
        // staging paths too, including broken links, rather than truncating them.
        let staging = path.with_extension("tmp");
        match std::fs::symlink_metadata(&staging) {
            Ok(_) => bail!(
                "{} already exists; init will not replace it",
                staging.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    if matches!(kind, InitKind::Knowledge) && matches!(mode, KnowledgeInitMode::Vector) {
        directories(&out.join("knowledge/embeddings"), out)?;
    }
    Ok(())
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
    use crate::commands::memory::{MemoryBuildMode, execute_memory_build};
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
            mode: KnowledgeInitMode::Context,
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
            mode: KnowledgeInitMode::Context,
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
        assert!(
            skill_markdown.contains("description: \"Incident response coordination playbook.\"")
        );
        assert!(skill_markdown.contains("# Incident Commander"));
        assert!(skill_markdown.contains("Incident response coordination playbook."));
        assert!(!skill_markdown.contains("{{SKILL_NAME}}"));

        let _ = std::fs::remove_dir_all(out);
    }

    #[tokio::test]
    async fn init_knowledge_context_creates_valid_manifest_and_layout() {
        let out = temp_dir("knowledge-context");
        let args = InitArgs {
            kind: InitKind::Knowledge,
            mode: KnowledgeInitMode::Context,
            name: "engineering-playbook".into(),
            description: "Engineering playbook intended for direct context loading.".into(),
            out_dir: Some(out.clone()),
        };

        args.run("http://example.invalid".into()).await.unwrap();

        let manifest_path = out.join("agent.json");
        let readme_path = out.join("README.md");
        let context_doc_path = out.join("knowledge").join("docs").join("context.md");

        assert!(manifest_path.exists());
        assert!(readme_path.exists());
        assert!(context_doc_path.exists());
        assert!(!out.join("knowledge").join("chunks.jsonl").exists());
        assert!(
            !out.join("knowledge")
                .join("embeddings")
                .join("default.f32")
                .exists()
        );
        assert!(
            !out.join("knowledge")
                .join("indexes")
                .join("default")
                .exists()
        );

        validate(&manifest_path);

        let manifest_json: Value =
            serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
        assert_eq!(manifest_json["kind"], "knowledge");
        assert_eq!(manifest_json["knowledge"]["mode"], "context");
        assert_eq!(
            manifest_json["knowledge"]["documents"][0]["path"],
            "knowledge/docs/context.md"
        );

        let readme_text = std::fs::read_to_string(&readme_path).unwrap();
        assert!(readme_text.contains("# Engineering Playbook"));

        let context_doc_text = std::fs::read_to_string(&context_doc_path).unwrap();
        assert!(context_doc_text.contains("# Engineering Playbook"));

        let _ = std::fs::remove_dir_all(out);
    }

    #[tokio::test]
    async fn init_knowledge_vector_creates_valid_manifest_and_layout() {
        let out = temp_dir("knowledge-vector");
        let args = InitArgs {
            kind: InitKind::Knowledge,
            mode: KnowledgeInitMode::Vector,
            name: "python-docs".into(),
            description: "Prepared retrieval corpus for Python documentation.".into(),
            out_dir: Some(out.clone()),
        };

        args.run("http://example.invalid".into()).await.unwrap();

        let manifest_path = out.join("agent.json");
        let readme_path = out.join("README.md");
        let chunks_path = out.join("knowledge").join("chunks.jsonl");
        let sources_path = out.join("knowledge").join("sources.jsonl");
        let embeddings_dir = out.join("knowledge").join("embeddings");
        let vectors_path = out.join("knowledge").join("embeddings").join("default.f32");

        assert!(manifest_path.exists());
        assert!(readme_path.exists());
        assert!(chunks_path.exists());
        assert!(sources_path.exists());
        assert!(embeddings_dir.exists());
        assert!(!vectors_path.exists());
        assert!(
            !out.join("knowledge")
                .join("indexes")
                .join("default")
                .exists()
        );

        validate(&manifest_path);

        let manifest_json: Value =
            serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
        assert_eq!(manifest_json["kind"], "knowledge");
        assert_eq!(manifest_json["knowledge"]["mode"], "vector");
        assert_eq!(
            manifest_json["knowledge"]["corpus"]["chunks_path"],
            "knowledge/chunks.jsonl"
        );
        assert_eq!(
            manifest_json["knowledge"]["embedding"]["vectors_path"],
            "knowledge/embeddings/default.f32"
        );

        let _ = std::fs::remove_dir_all(out);
    }

    #[tokio::test]
    async fn init_memory_creates_valid_named_manifest_readme_and_schema_only() {
        let out = temp_dir("memory-named");
        let args = InitArgs {
            kind: InitKind::Memory,
            mode: KnowledgeInitMode::Context,
            name: "conversation-continuity".into(),
            description: "Describe the durable memory contract this blueprint provides.".into(),
            out_dir: Some(out.clone()),
        };

        args.run("http://example.invalid".into()).await.unwrap();

        let manifest_path = out.join("agent.json");
        let readme_path = out.join("README.md");
        let schema_path = out.join("schemas").join("user-preference.schema.json");

        assert!(manifest_path.exists());
        assert!(readme_path.exists());
        assert!(schema_path.exists());
        assert!(!out.join("memory").exists());

        validate(&manifest_path);

        let manifest_json: Value =
            serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
        assert_eq!(manifest_json["kind"], "memory");
        assert_eq!(manifest_json["name"], "conversation-continuity");
        assert_eq!(manifest_json["readme"], "README.md");
        assert_eq!(
            manifest_json["memory"]["record_types"]["user_preference"]["schema"],
            "schemas/user-preference.schema.json"
        );
        assert_eq!(
            manifest_json["memory"]["spaces"]["profile"]["record_types"][0],
            "user_preference"
        );

        let readme_text = std::fs::read_to_string(&readme_path).unwrap();
        assert!(readme_text.contains("# Conversation Continuity"));
        assert!(readme_text.contains("agentpm memory build"));
        assert!(readme_text.contains("agentpm memory inspect ."));
        assert!(readme_text.contains("does not provide a live memory store"));

        let schema_json: Value =
            serde_json::from_str(&std::fs::read_to_string(&schema_path).unwrap()).unwrap();
        assert_eq!(
            schema_json["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        assert_eq!(
            schema_json["properties"]["favorite_color"]["x-agentpm-data-class"],
            "personal"
        );
        assert_eq!(
            schema_json["properties"]["favorite_color"]["x-agentpm-persist"],
            true
        );

        let _ = std::fs::remove_dir_all(out);
    }

    #[tokio::test]
    async fn init_memory_default_name_still_creates_buildable_starter() {
        let out = temp_dir("memory-default");
        let args = InitArgs {
            kind: InitKind::Memory,
            mode: KnowledgeInitMode::Context,
            name: "my-tool".into(),
            description: "Starter AgentPM project".into(),
            out_dir: Some(out.clone()),
        };

        args.run("http://example.invalid".into()).await.unwrap();

        let manifest_path = out.join("agent.json");
        let schema_path = out.join("schemas").join("user-preference.schema.json");
        assert!(manifest_path.exists());
        assert!(schema_path.exists());
        assert!(!out.join("memory").exists());

        validate(&manifest_path);
        execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap();

        assert!(out.join("memory").join("build.json").exists());
        assert!(
            out.join("memory")
                .join("contracts")
                .join("index.json")
                .exists()
        );
        assert!(
            out.join("memory")
                .join("contracts")
                .join("profile.user_preference.schema.json")
                .exists()
        );

        let _ = std::fs::remove_dir_all(out);
    }

    #[tokio::test]
    async fn init_profile_creates_valid_manifest_and_readme_only() {
        let out = temp_dir("profile");
        let args = InitArgs {
            kind: InitKind::Profile,
            mode: KnowledgeInitMode::Context,
            name: "customer-success-advocate".into(),
            description: "Warm, professional customer support behavior.".into(),
            out_dir: Some(out.clone()),
        };

        args.run("http://example.invalid".into()).await.unwrap();

        let manifest_path = out.join("agent.json");
        let readme_path = out.join("README.md");

        assert!(manifest_path.exists());
        assert!(readme_path.exists());
        assert!(!out.join("profile").exists());
        assert!(!out.join("profiles").exists());
        assert!(!out.join("instructions").exists());
        assert!(!out.join("schemas").exists());
        assert!(!out.join("scripts").exists());
        assert!(!out.join("references").exists());

        validate(&manifest_path);

        let manifest_json: Value =
            serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
        assert_eq!(manifest_json["kind"], "profile");
        assert_eq!(manifest_json["name"], "customer-success-advocate");
        assert_eq!(manifest_json["readme"], "README.md");
        assert!(manifest_json.get("display_name").is_none());
        assert!(manifest_json.get("tools").is_none());
        assert!(manifest_json.get("skills").is_none());
        assert!(manifest_json.get("knowledge").is_none());
        assert!(manifest_json.get("memory").is_none());
        assert!(manifest_json.get("profiles").is_none());
        assert_eq!(
            manifest_json["profile"]["identity"]["role"],
            "Support assistant"
        );
        assert_eq!(
            manifest_json["profile"]["objectives"][0],
            "Help the user reach a clear next step."
        );
        assert_eq!(
            manifest_json["profile"]["communication"]["verbosity"],
            "concise"
        );

        let readme_text = std::fs::read_to_string(&readme_path).unwrap();
        assert!(readme_text.contains("# Customer Success Advocate"));
        assert!(readme_text.contains("generated by `agentpm init --kind profile`"));
        assert!(readme_text.contains("Instruction Profiles vs Skills"));
        assert!(readme_text.contains("author intent; they are not runtime enforcement guarantees"));
        assert!(readme_text.contains("is not part of the structured behavioral contract"));
        assert!(!readme_text.contains("{{PROFILE_"));

        let _ = std::fs::remove_dir_all(out);
    }

    #[tokio::test]
    async fn init_loop_creates_valid_manifest_and_readme_only() {
        let out = temp_dir("loop");
        let args = InitArgs {
            kind: InitKind::Loop,
            mode: KnowledgeInitMode::Context,
            name: "incident-response-loop".into(),
            description: "Bounded review and execution loop.".into(),
            out_dir: Some(out.clone()),
        };

        args.run("http://example.invalid".into()).await.unwrap();

        let manifest_path = out.join("agent.json");
        let readme_path = out.join("README.md");

        assert!(manifest_path.exists());
        assert!(readme_path.exists());
        assert!(!out.join("loop").exists());
        assert!(!out.join("loops").exists());
        assert!(!out.join("schemas").exists());
        assert!(!out.join("scripts").exists());
        assert!(!out.join("references").exists());

        validate(&manifest_path);

        let manifest_json: Value =
            serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
        assert_eq!(
            manifest_json["$schema"],
            crate::manifest::DEFAULT_MANIFEST_SCHEMA_URL
        );
        assert_eq!(manifest_json["kind"], "loop");
        assert_eq!(manifest_json["name"], "incident-response-loop");
        assert_eq!(manifest_json["readme"], "README.md");
        assert_eq!(manifest_json["loop"]["entry_phase"], "assess");
        assert_eq!(manifest_json["loop"]["phases"].as_array().unwrap().len(), 3);
        assert_eq!(
            manifest_json["loop"]["transitions"]
                .as_array()
                .unwrap()
                .len(),
            5
        );
        assert_eq!(manifest_json["loop"]["checkpoints"][0]["type"], "approval");
        assert_eq!(
            manifest_json["loop"]["checkpoints"][0]["before_phase"],
            "review"
        );
        assert_eq!(manifest_json["loop"]["transitions"][2]["to"], "review");
        assert_eq!(manifest_json["loop"]["transitions"][3]["from"], "review");
        assert_eq!(manifest_json["loop"]["transitions"][3]["to"], "execute");
        assert_eq!(manifest_json["loop"]["transitions"][4]["to"], "$end");

        let readme_text = std::fs::read_to_string(&readme_path).unwrap();
        assert!(readme_text.contains("# Incident Response Loop"));
        assert!(readme_text.contains("generated by `agentpm init --kind loop`"));
        assert!(readme_text.contains("portable control-flow contract"));
        assert!(readme_text.contains("starter review loop"));
        assert!(readme_text.contains("does not execute a runtime loop by itself"));
        assert!(!readme_text.contains("{{LOOP_"));

        let _ = std::fs::remove_dir_all(out);
    }

    #[tokio::test]
    async fn init_tool_agent_template_and_skill_behavior_still_work() {
        for (kind, label, expected_kind) in [
            (InitKind::Tool, "tool", "tool"),
            (InitKind::Agent, "agent", "agent"),
            (InitKind::Template, "template", "template"),
            (InitKind::Skill, "skill", "skill"),
            (InitKind::Memory, "memory", "memory"),
            (InitKind::Profile, "profile", "profile"),
            (InitKind::Loop, "loop", "loop"),
        ] {
            let out = temp_dir(label);
            let args = InitArgs {
                kind,
                mode: KnowledgeInitMode::Context,
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
            if expected_kind == "agent" || expected_kind == "template" || expected_kind == "skill" {
                validate(&manifest_path);
            }
            if expected_kind == "template" {
                assert!(out.join("template").join("README.md").exists());
            }
            if expected_kind == "skill" {
                assert!(out.join("SKILL.md").exists());
            }
            if expected_kind == "memory" {
                validate(&manifest_path);
                assert!(out.join("README.md").exists());
                assert!(
                    out.join("schemas")
                        .join("user-preference.schema.json")
                        .exists()
                );
                assert!(!out.join("memory").exists());
            }
            if expected_kind == "profile" {
                validate(&manifest_path);
                assert!(out.join("README.md").exists());
                assert!(!out.join("profile").exists());
                assert!(!out.join("profiles").exists());
                assert!(!out.join("instructions").exists());
                assert!(!out.join("schemas").exists());
            }
            if expected_kind == "loop" {
                validate(&manifest_path);
                assert!(out.join("README.md").exists());
                assert!(!out.join("loop").exists());
                assert!(!out.join("loops").exists());
                assert!(!out.join("schemas").exists());
            }

            let _ = std::fs::remove_dir_all(out);
        }
    }

    #[tokio::test]
    async fn init_rejects_mode_for_non_knowledge_kinds() {
        let out = temp_dir("mode-invalid");
        let args = InitArgs {
            kind: InitKind::Tool,
            mode: KnowledgeInitMode::Vector,
            name: "demo-tool".into(),
            description: "Starter tool.".into(),
            out_dir: Some(out.clone()),
        };

        let err = args.run("http://example.invalid".into()).await.unwrap_err();
        assert_eq!(
            err.to_string(),
            "`--mode` is only applicable when `--kind knowledge` is used"
        );
        assert!(!out.join("agent.json").exists());

        let _ = std::fs::remove_dir_all(out);
    }

    #[tokio::test]
    async fn init_profile_rejects_mode_for_non_knowledge_kind() {
        let out = temp_dir("profile-mode-invalid");
        let args = InitArgs {
            kind: InitKind::Profile,
            mode: KnowledgeInitMode::Vector,
            name: "support-profile".into(),
            description: "Starter profile.".into(),
            out_dir: Some(out.clone()),
        };

        let err = args.run("http://example.invalid".into()).await.unwrap_err();
        assert_eq!(
            err.to_string(),
            "`--mode` is only applicable when `--kind knowledge` is used"
        );
        assert!(!out.join("agent.json").exists());

        let _ = std::fs::remove_dir_all(out);
    }

    #[tokio::test]
    async fn init_loop_rejects_mode_for_non_knowledge_kind() {
        let out = temp_dir("loop-mode-invalid");
        let args = InitArgs {
            kind: InitKind::Loop,
            mode: KnowledgeInitMode::Vector,
            name: "incident-loop".into(),
            description: "Starter loop.".into(),
            out_dir: Some(out.clone()),
        };

        let err = args.run("http://example.invalid".into()).await.unwrap_err();
        assert_eq!(
            err.to_string(),
            "`--mode` is only applicable when `--kind knowledge` is used"
        );
        assert!(!out.join("agent.json").exists());

        let _ = std::fs::remove_dir_all(out);
    }
}
