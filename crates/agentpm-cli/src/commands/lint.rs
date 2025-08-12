use crate::prelude::*;
use crate::util::{discover_manifest_files, load_json, load_schema_value};
use anyhow::anyhow;
use jsonschema::{Draft, JSONSchema};
use serde::Serialize;
use serde_json::Value;
use std::{fs, path::PathBuf};

#[derive(Args, Debug, Default)]
pub struct LintArgs {
    /// Files/dirs/globs to lint. Defaults to ./agent.json
    #[arg(value_name = "PATHS")]
    paths: Vec<String>,

    /// Override schema URL or path
    #[arg(long, value_name = "URL|PATH")]
    schema: Option<String>,

    /// Treat warnings as errors
    #[arg(long)]
    strict: bool,

    /// Output format: pretty | json | ndjson
    #[arg(long, default_value = "pretty")]
    format: String,

    /// Attempt automatic fixes (non-invasive)
    #[arg(long)]
    fix: bool,
}

#[derive(Serialize)]
struct LintIssue {
    file: String,
    level: &'static str, // "error" | "warning"
    message: String,
    instance_path: String,
    schema_path: String,
}

#[derive(Serialize)]
struct LintFileReport {
    file: String,
    ok: bool,
    issues: Vec<LintIssue>,
}

impl LintArgs {
    pub async fn run(self) -> Result<()> {
        // Resolve schema
        let schema_source = self.schema.unwrap_or_else(|| {
            let local_path = PathBuf::from("schemas/agentpm.manifest.schema.json");
            if local_path.exists() {
                local_path.to_string_lossy().into_owned()
            } else {
                "https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json".to_string()
            }
        });
        let schema_value = load_schema_value(&schema_source)?;
        let schema_static: &'static serde_json::Value = Box::leak(Box::new(schema_value));
        let compiled = JSONSchema::options()
            .with_draft(Draft::Draft202012)
            .compile(schema_static)?;

        // Discover manifest files
        let files = discover_manifest_files(&self.paths)?;
        if files.is_empty() {
            println!("No agent.json found. (Looked at ./agent.json or provided paths)");
            return Ok(());
        }

        // Validate
        let mut reports: Vec<LintFileReport> = Vec::new();

        for file in files {
            let (mut value, _) = match load_json(&file) {
                Ok(v) => v,
                Err(e) => {
                    reports.push(LintFileReport {
                        file: file.to_string_lossy().to_string(),
                        ok: false,
                        issues: vec![LintIssue {
                            file: file.to_string_lossy().to_string(),
                            level: "error",
                            message: format!("Failed to parse JSON: {e}"),
                            instance_path: "".into(),
                            schema_path: "".into(),
                        }],
                    });
                    continue;
                }
            };

            let mut issues: Vec<LintIssue> = Vec::new();

            // JSON Schema validation
            if let Err(errors) = compiled.validate(&value) {
                for e in errors {
                    issues.push(LintIssue {
                        file: file.to_string_lossy().to_string(),
                        level: "error",
                        message: e.to_string(),
                        instance_path: e.instance_path.to_string(),
                        schema_path: e.schema_path.to_string(),
                    })
                }
            }

            // Semantic warnings (examples)
            // - recommend $schema present
            if value.get("$schema").is_none() {
                issues.push(LintIssue {
                    file: file.to_string_lossy().to_string(),
                    level: "warning",
                    message: "Missing $schema; editors may lack IntelliSense.".into(),
                    instance_path: "".into(),
                    schema_path: "".into(),
                });

                // --fix: add $schema pointing at the same schema we used
                if self.fix
                    && let Some(obj) = value.as_object_mut()
                {
                    obj.insert("$schema".into(), Value::String(schema_source.clone()));
                    // Write back
                    let pretty = serde_json::to_string_pretty(&value)?;
                    fs::write(&file, pretty + "\n").with_context(|| {
                        format!("Failed to write fixed file {}", file.display())
                    })?;
                }
            }

            // - warn if description is empty
            if let Some(Value::String(desc)) = value.get("description")
                && desc.trim().is_empty()
            {
                issues.push(LintIssue {
                    file: file.to_string_lossy().to_string(),
                    level: "warning",
                    message: "`description` should not be empty".into(),
                    instance_path: "/description".into(),
                    schema_path: "".into(),
                });
            }

            let has_error = issues.iter().any(|i| i.level == "error");
            let has_warning = issues.iter().any(|i| i.level == "warning");

            let ok = if self.strict {
                // strict: any warning or error fails
                !has_error && !has_warning
            } else {
                // non-strict: errors fail, warnings allowed
                !has_error
            };
            reports.push(LintFileReport {
                file: file.to_string_lossy().to_string(),
                ok,
                issues,
            });
        }

        // Output
        match self.format.as_str() {
            "json" => {
                println!("{}", serde_json::to_string_pretty(&reports)?);
            }
            "ndjson" => {
                for report in &reports {
                    println!("{}", serde_json::to_string(&report)?);
                }
            }
            _ => {
                // pretty
                for r in &reports {
                    if r.ok {
                        println!("✓ {}", r.file);
                    } else {
                        println!("✗ {}", r.file);
                    }
                    for i in &r.issues {
                        let badge = match i.level {
                            "error" => "ERROR",
                            _ => "WARN ",
                        };
                        println!("  [{badge}] {}", i.message);
                        if !i.instance_path.is_empty() {
                            println!("        at instance {}", i.instance_path);
                        }
                        if !i.schema_path.is_empty() {
                            println!("        vs schema  {}", i.schema_path);
                        }
                    }
                }
            }
        }

        // Exit code
        let should_fail = reports.iter().any(|r| !r.ok);
        if should_fail {
            return Err(anyhow!("Lint failed"));
        }
        Ok(())
    }
}
