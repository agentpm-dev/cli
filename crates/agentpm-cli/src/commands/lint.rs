use crate::lint_output::render_lint_report;
use crate::manifest::apds::{self, Context, Evaluation, Level, ResolvedGraph, Status};
use crate::manifest::{
    LintFileReport, LintIssue, discover_manifest_files, load_manifest_value, resolve_schema_source,
    validate_manifest_value, write_manifest_pretty,
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

    /// Evaluate the pinned APDS contract at an explicit scope
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
        // Resolve schema
        let overridden = self.schema.is_some();
        let schema_source = resolve_schema_source(self.schema);
        // Reject arbitrary remote inputs even when the manifest declares APDS
        // and would otherwise be checked solely against the bundled contract.
        if overridden {
            crate::manifest::load_schema_value(&schema_source)?;
        }

        // Discover manifests via shared helper
        let mut files = discover_manifest_files(&self.paths)?;
        if self.apds.is_some() {
            for input in &self.paths {
                let path = PathBuf::from(input);
                if path.is_file() && !files.contains(&path) {
                    files.push(path);
                }
            }
            files.sort();
            files.dedup();
        }
        if files.is_empty() {
            if self.apds.is_some() {
                return Err(anyhow!(
                    "APDS validation requires an existing manifest file"
                ));
            }
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
                            rule_id: None,
                            human_message: None,
                            details: Vec::new(),
                        }],
                    });
                    continue;
                }
            };

            let (schema_ok, mut issues) = if let Some(level) = self.apds {
                if let Some(root) = &package_root
                    && !file.canonicalize()?.starts_with(root)
                {
                    return Err(anyhow!(
                        "Manifest must be inside the supplied complete package root"
                    ));
                }
                let evaluation = apds::evaluate(
                    &value,
                    &file.to_string_lossy(),
                    level,
                    Context {
                        graph: graph.as_ref(),
                        package_root: package_root.as_deref(),
                    },
                )?;
                let ok = evaluation.status == Status::Conformant;
                let issues = evaluation.issues.clone();
                evaluations.insert(file.to_string_lossy().into_owned(), evaluation);
                (ok, issues)
            } else {
                validate_manifest_value(
                    &schema_source,
                    &file.to_string_lossy(),
                    &mut value,
                    self.fix,
                )?
            };
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
        let output: Vec<_> = reports
            .iter()
            .map(|report| Report {
                lint: report,
                apds: evaluations.get(&report.file),
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
                    if let Some(evaluation) = evaluations.get(&r.file) {
                        println!(
                            "  APDS {} scope: {}; Runner behavior not evaluated.",
                            serde_json::to_value(evaluation.level)?
                                .as_str()
                                .unwrap_or(""),
                            serde_json::to_value(evaluation.status)?
                                .as_str()
                                .unwrap_or("")
                        );
                        if !evaluation.unevaluated.is_empty() {
                            println!(
                                "  Unevaluated: {}",
                                evaluation
                                    .unevaluated
                                    .iter()
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            );
                        }
                        for note in &evaluation.notes {
                            println!("  {note}");
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
