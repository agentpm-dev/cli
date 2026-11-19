use crate::lint_output::render_lint_report;
use crate::manifest::apds::{self, Context, Evaluation, Level, ResolvedGraph, Status};
use crate::manifest::{
    LintFileReport, LintIssue, discover_manifest_files, load_manifest_value, resolve_schema_source,
    write_manifest_pretty,
};
use crate::prelude::*;
use anyhow::anyhow;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(serde::Serialize)]
struct Report<'a> {
    #[serde(flatten)]
    lint: &'a LintFileReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    apds: Option<&'a Evaluation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compatibility: Option<&'static str>,
}

#[derive(Args, Debug, Default)]
pub struct LintArgs {
    /// Files/dirs/globs to lint. Defaults to ./agent.json
    #[arg(value_name = "PATHS")]
    paths: Vec<String>,

    /// Local schema for non-authoritative development lint (not APDS conformance)
    #[arg(long, value_name = "PATH")]
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

    /// Read-only compatibility check for undeclared historical manifests
    #[arg(long, conflicts_with_all = ["apds", "fix", "schema"])]
    legacy: bool,

    /// APDS validation scope (defaults to standalone)
    #[arg(long, value_enum, conflicts_with_all = ["schema", "fix"])]
    apds: Option<Level>,

    /// Local selected-dependency graph JSON for --apds resolved
    #[arg(long, value_name = "FILE", requires = "apds")]
    graph: Option<PathBuf>,

    /// Complete supplied package directory for --apds packaged
    #[arg(long, value_name = "DIR", requires = "apds")]
    package_root: Option<PathBuf>,
}

impl LintArgs {
    pub async fn run(self) -> Result<()> {
        if self.graph.is_some() && self.apds != Some(Level::Resolved) {
            return Err(anyhow!("--graph requires --apds resolved"));
        }
        if self.package_root.is_some() && self.apds != Some(Level::Packaged) {
            return Err(anyhow!("--package-root requires --apds packaged"));
        }
        let graph = self.graph.as_deref().map(ResolvedGraph::read).transpose()?;
        let package_root = self
            .package_root
            .as_deref()
            .map(std::fs::canonicalize)
            .transpose()?;
        let mut evaluations = BTreeMap::new();
        let mut legacy_files = std::collections::BTreeSet::new();
        // Resolve schema
        let overridden = self.schema.is_some();
        let schema_source = resolve_schema_source(self.schema);
        // Reject arbitrary remote inputs even when the manifest declares APDS
        // and would otherwise be checked solely against the bundled contract.
        if overridden {
            crate::manifest::load_schema_value(&schema_source)?;
        }

        // Explicit filenames (including *.agent.json) must be evaluated, and a
        // missing input must never become a successful empty validation run.
        let inputs = if self.paths.is_empty() {
            vec!["agent.json".into()]
        } else {
            self.paths.clone()
        };
        let mut files = Vec::new();
        for input in &inputs {
            let path = PathBuf::from(input);
            let found = discover_manifest_files(std::slice::from_ref(input))?;
            if path.is_file() {
                files.push(path);
            } else if found.is_empty() {
                files.push(if path.is_dir() {
                    path.join("agent.json")
                } else {
                    path
                });
            } else {
                files.extend(found);
            }
        }
        files.sort();
        files.dedup();

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
                            rule_id: None,
                            human_message: None,
                            details: Vec::new(),
                        }],
                    });
                    continue;
                }
            };

            let original = value.clone();
            let inferred = self.legacy && value.is_object() && value.get("standard").is_none();
            let mut evaluated_value = value.clone();
            if inferred {
                // This temporary selector is never written or presented as an
                // explicit declaration or APDS-conformant historical release.
                evaluated_value["standard"] = serde_json::json!({"id":"agentpm","version":"1.0.0"});
            }
            if let Some(root) = &package_root
                && !file.canonicalize()?.starts_with(root)
            {
                return Err(anyhow!(
                    "Manifest must be inside the supplied complete package root"
                ));
            }
            let evaluation = if let Some(level) = self.apds {
                apds::evaluate(
                    &value,
                    &file.to_string_lossy(),
                    level,
                    Context {
                        graph: graph.as_ref(),
                        package_root: package_root.as_deref(),
                    },
                )?
            } else {
                let result = crate::manifest::authoring::evaluate(
                    &mut evaluated_value,
                    &file.to_string_lossy(),
                    self.fix,
                )?;
                if !inferred {
                    value = evaluated_value;
                }
                result
            };
            let schema_ok = evaluation.status == Status::Conformant;
            let mut issues = evaluation.issues.clone();
            if inferred {
                legacy_files.insert(file.to_string_lossy().into_owned());
            } else {
                evaluations.insert(file.to_string_lossy().into_owned(), evaluation);
            }
            if overridden {
                issues.push(LintIssue {
                    file: file.to_string_lossy().into_owned(),
                    level: "warning",
                    message: "Schema override: non-authoritative development validation; this result does not establish APDS conformance. Declared APDS manifests still use the bundled contract.".into(),
                    instance_path: "".into(),
                    schema_path: "".into(),
                    rule_id: Some("schema.override".into()),
                    human_message: None,
                    details: Vec::new(),
                });
            }

            let has_error = issues.iter().any(|i| i.level == "error");
            let has_warning = issues.iter().any(|i| i.level == "warning");
            let ok = if self.strict {
                schema_ok && !has_warning
            } else {
                schema_ok && !has_error
            };

            // If we auto-fixed ($schema), write back once here
            if self.fix && schema_ok && value != original {
                // Preserve original formatting and key order when no repair is needed.
                write_manifest_pretty(&file, &value)?;
            }

            reports.push(LintFileReport {
                file: file.to_string_lossy().to_string(),
                ok,
                issues,
            });
        }

        // Output
        let output: Vec<_> = reports
            .iter()
            .map(|report| Report {
                lint: report,
                apds: evaluations.get(&report.file),
                compatibility: legacy_files
                    .contains(&report.file)
                    .then_some("legacy-unverified"),
            })
            .collect();
        match self.format.as_str() {
            "json" => {
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            "ndjson" => {
                for report in &output {
                    println!("{}", serde_json::to_string(&report)?);
                }
            }
            _ => {
                // pretty
                for r in &reports {
                    print!("{}", render_lint_report(r));
                    if legacy_files.contains(&r.file) {
                        println!(
                            "  Legacy compatibility only (inferred agentpm / 1.0.0); APDS conformance and Runner behavior not evaluated. Source unchanged."
                        );
                    }
                    if let Some(evaluation) = evaluations.get(&r.file) {
                        print!("{}", crate::lint_output::render_apds_summary(evaluation)?);
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
  "standard": {"id":"agentpm","version":"1.0.0"},
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
            ..Default::default()
        }
        .run()
        .await;

        assert!(result.is_ok(), "expected lint to pass, got: {result:?}");

        let _ = std::fs::remove_dir_all(out);
    }

    #[tokio::test]
    async fn lint_profile_warning_fails_only_in_strict_mode() {
        let out = temp_dir("profile-strict");
        let manifest_path = out.join("agent.json");
        std::fs::write(
            &manifest_path,
            r#"{
  "standard": {"id":"agentpm","version":"1.0.0"},
  "kind": "profile",
  "name": "customer-success-advocate",
  "version": "1.0.0",
  "description": "Support behavior profile.",
  "profile": {
    "identity": {
      "role": "Support agent"
    },
    "objectives": ["Help the user reach a clear next step."],
    "communication": {
      "tone": ["warm"],
      "verbosity": "concise"
    }
  }
}
"#,
        )
        .unwrap();

        let non_strict = LintArgs {
            paths: vec![manifest_path.to_string_lossy().to_string()],
            schema: None,
            strict: false,
            format: "json".into(),
            fix: false,
            ..Default::default()
        }
        .run()
        .await;
        assert!(
            non_strict.is_ok(),
            "expected non-strict lint to allow warning, got: {non_strict:?}"
        );

        let strict = LintArgs {
            paths: vec![manifest_path.to_string_lossy().to_string()],
            schema: None,
            strict: true,
            format: "json".into(),
            fix: false,
            ..Default::default()
        }
        .run()
        .await;
        assert!(strict.is_err(), "expected strict lint to fail on warning");

        let _ = std::fs::remove_dir_all(out);
    }

    #[tokio::test]
    async fn lint_profile_formats_work_across_pretty_json_and_ndjson() {
        let out = temp_dir("profile-formats");
        let manifest_path = out.join("agent.json");
        std::fs::write(
            &manifest_path,
            r#"{
  "standard": {"id":"agentpm","version":"1.0.0"},
  "kind": "profile",
  "name": "customer-success-advocate",
  "version": "1.0.0",
  "description": "Support behavior profile.",
  "profile": {
    "identity": {
      "role": "Support agent"
    },
    "objectives": ["Help the user reach a clear next step."],
    "communication": {
      "tone": ["warm"],
      "verbosity": "concise"
    }
  }
}
"#,
        )
        .unwrap();

        for format in ["pretty", "json", "ndjson"] {
            let non_strict = LintArgs {
                paths: vec![manifest_path.to_string_lossy().to_string()],
                schema: None,
                strict: false,
                format: format.into(),
                fix: false,
                ..Default::default()
            }
            .run()
            .await;
            assert!(
                non_strict.is_ok(),
                "expected non-strict lint to allow warning in format {format}, got: {non_strict:?}"
            );

            let strict = LintArgs {
                paths: vec![manifest_path.to_string_lossy().to_string()],
                schema: None,
                strict: true,
                format: format.into(),
                fix: false,
                ..Default::default()
            }
            .run()
            .await;
            assert!(
                strict.is_err(),
                "expected strict lint to fail on warning in format {format}"
            );
        }

        let _ = std::fs::remove_dir_all(out);
    }

    #[tokio::test]
    async fn lint_loop_formats_work_across_pretty_json_and_ndjson() {
        let out = temp_dir("loop-formats");
        let manifest_path = out.join("agent.json");
        std::fs::write(
            &manifest_path,
            r#"{
  "standard": {"id":"agentpm","version":"1.0.0"},
  "kind": "loop",
  "name": "incident-response-loop",
  "version": "1.0.0",
  "description": "Support loop.",
  "loop": {
    "entry_phase": "triage",
    "phases": [
      {
        "id": "triage",
        "objective": "Assess the incident."
      }
    ],
    "transitions": [
      {
        "from": "triage",
        "on": "complete",
        "to": "$end"
      }
    ]
  }
}
"#,
        )
        .unwrap();

        for format in ["pretty", "json", "ndjson"] {
            let non_strict = LintArgs {
                paths: vec![manifest_path.to_string_lossy().to_string()],
                schema: None,
                strict: false,
                format: format.into(),
                fix: false,
                ..Default::default()
            }
            .run()
            .await;
            assert!(
                non_strict.is_ok(),
                "expected non-strict lint to allow warning in format {format}, got: {non_strict:?}"
            );

            let strict = LintArgs {
                paths: vec![manifest_path.to_string_lossy().to_string()],
                schema: None,
                strict: true,
                format: format.into(),
                fix: false,
                ..Default::default()
            }
            .run()
            .await;
            assert!(
                strict.is_err(),
                "expected strict lint to fail on warning in format {format}"
            );
        }

        let _ = std::fs::remove_dir_all(out);
    }
}
