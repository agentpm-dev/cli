use crate::manifest::{
    LintFileReport, LintIssue, discover_manifest_files, load_manifest_value, resolve_schema_source,
    validate_manifest_value, write_manifest_pretty,
};
use crate::prelude::*;
use anyhow::anyhow;

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

    /// Output format: pretty | JSON | ndjson
    #[arg(long, default_value = "pretty")]
    format: String,

    /// Attempt automatic fixes (non-invasive)
    #[arg(long)]
    fix: bool,
}

impl LintArgs {
    pub async fn run(self) -> Result<()> {
        // Resolve schema
        let schema_source = resolve_schema_source(self.schema);

        // Discover manifests via shared helper
        let files = discover_manifest_files(&self.paths)?;
        if files.is_empty() {
            println!("No agent.json found. (Looked at ./agent.json or provided paths)");
            return Ok(());
        }

        // Validate
        let mut reports: Vec<LintFileReport> = Vec::new();

        for file in files {
            let (mut value, _) = match load_manifest_value(&file) {
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

            let (schema_ok, issues) = validate_manifest_value(
                &schema_source,
                &file.to_string_lossy(),
                &mut value,
                self.fix,
            )?;

            let has_error = issues.iter().any(|i| i.level == "error");
            let has_warning = issues.iter().any(|i| i.level == "warning");
            let ok = if self.strict {
                schema_ok && !has_warning
            } else {
                schema_ok && !has_error
            };

            // If we auto-fixed ($schema), write back once here
            if self.fix && value.get("$schema").is_some() {
                // The value may or may not have changed; writing is inexpensive/safe.
                write_manifest_pretty(&file, &value)?;
            }

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("agentpm-lint-{label}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn lint_accepts_valid_knowledge_manifest_without_warnings() {
        let out = temp_dir("knowledge");
        let manifest_path = out.join("agent.json");
        std::fs::write(
            &manifest_path,
            r#"{
  "$schema": "https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json",
  "kind": "knowledge",
  "name": "engineering-playbook",
  "version": "0.1.0",
  "description": "Engineering playbook intended for direct context loading.",
  "knowledge": {
    "mode": "context",
    "documents": [
      {
        "path": "knowledge/docs/context.md"
      }
    ]
  }
}
"#,
        )
        .unwrap();

        let result = LintArgs {
            paths: vec![manifest_path.to_string_lossy().to_string()],
            schema: None,
            strict: true,
            format: "json".into(),
            fix: false,
        }
        .run()
        .await;

        assert!(result.is_ok(), "expected lint to pass, got: {result:?}");

        let _ = std::fs::remove_dir_all(out);
    }
}
