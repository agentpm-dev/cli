//! New local authoring uses the pinned evaluator. Historical readers remain separate.
use super::apds::{self, Context, Evaluation, Level, Status};
use super::{DEFAULT_MANIFEST_SCHEMA_URL, LintFileReport, LintIssue, load_manifest_value};
use anyhow::{Result, bail};
use serde_json::Value;
use std::path::Path;

const DECLARATION: &str = r#"{"id":"agentpm","version":"1.0.0"}"#;

pub(crate) fn evaluate(value: &mut Value, file: &str, fix: bool) -> Result<Evaluation> {
    let mut evaluation = apds::evaluate(value, file, Level::Standalone, Context::default())?;
    if evaluation.status == Status::Unsupported {
        let path = if value.pointer("/standard/id").and_then(Value::as_str) != Some("agentpm") {
            "/standard/id"
        } else {
            "/standard/version"
        };
        evaluation.issues.push(LintIssue {
            file: file.into(), level: "error",
            message: format!("Unsupported declared standard. This CLI supports {DECLARATION}; use a CLI supporting the declared contract or review the declaration in this local manifest."),
            instance_path: path.into(), schema_path: String::new(),
            rule_id: Some("APDS-ID-001".into()), human_message: None, details: Vec::new(),
        });
    } else if value.get("standard").is_none() {
        for issue in &mut evaluation.issues {
            if issue.instance_path == "/standard" {
                issue.message.push_str(&format!(" Review this local manifest, add `\"standard\":{DECLARATION}`, then run `agentpm lint` again. Existing published artifacts must not be rewritten."));
                issue.human_message = None;
            }
        }
    }
    // Editor advice is AgentPM policy, not another normative rule or validator.
    if value.get("$schema").is_none() {
        evaluation.issues.push(LintIssue {
            file: file.into(),
            level: "warning",
            message: "Missing $schema; editors may lack IntelliSense.".into(),
            instance_path: String::new(),
            schema_path: String::new(),
            rule_id: None,
            human_message: None,
            details: Vec::new(),
        });
        if fix
            && evaluation.status == Status::Conformant
            && let Some(object) = value.as_object_mut()
        {
            object.insert(
                "$schema".into(),
                Value::String(DEFAULT_MANIFEST_SCHEMA_URL.into()),
            );
        }
    }
    Ok(evaluation)
}

pub(crate) fn require_path(path: &Path) -> Result<()> {
    let (mut value, _) = load_manifest_value(path)?;
    let result = evaluate(&mut value, &path.to_string_lossy(), false)?;
    if result.status != Status::Conformant {
        let report = LintFileReport {
            file: path.to_string_lossy().into_owned(),
            ok: false,
            issues: result.issues,
        };
        bail!(
            "Manifest validation failed:\n{}",
            crate::lint_output::render_lint_report(&report)
        );
    }
    Ok(())
}

pub(crate) fn require_local_inspect_path(path: &Path) -> Result<()> {
    let canonical = path.canonicalize()?;
    let installed = canonical.ancestors().any(|parent| {
        matches!(
            parent.file_name().and_then(|name| name.to_str()),
            Some("knowledge" | "memory")
        ) && parent
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            == Some(".agentpm")
    });
    // Inspecting an installed immutable release must not demand that its bytes
    // be upgraded, including when selected through an absolute path or symlink.
    if !installed {
        require_path(path)?;
    }
    Ok(())
}
