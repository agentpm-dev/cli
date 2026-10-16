use crate::manifest::{LintFileReport, LintIssue};
use anyhow::{Result, anyhow};
use jsonschema::{Draft, JSONSchema, ValidationError, error::ValidationErrorKind};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::fmt::Write;

const MAX_MESSAGE_CHARS: usize = 320;
const MAX_UNION_DEPTH: usize = 12;

pub(crate) fn schema_issues(schema: &Value, file: &str, value: &Value) -> Result<Vec<LintIssue>> {
    collect_schema_issues(schema, file, value, "", "", "", 0)
}

fn collect_schema_issues(
    schema: &Value,
    file: &str,
    value: &Value,
    instance_prefix: &str,
    schema_from: &str,
    schema_to: &str,
    depth: usize,
) -> Result<Vec<LintIssue>> {
    let compiled = JSONSchema::options()
        .with_draft(Draft::Draft202012)
        .compile(schema)
        .map_err(|error| anyhow!("compiling manifest schema: {error}"))?;
    let mut issues = Vec::new();
    if let Err(errors) = compiled.validate(value) {
        for error in errors {
            if depth > 0
                && matches!(
                    error.kind,
                    ValidationErrorKind::InvalidReference { .. }
                        | ValidationErrorKind::Resolver { .. }
                        | ValidationErrorKind::Schema
                )
            {
                return Err(anyhow!("could not expand schema diagnostic: {error}"));
            }
            let local_schema_path = error.schema_path.to_string();
            let schema_path = format!(
                "{schema_to}{}",
                local_schema_path
                    .strip_prefix(schema_from)
                    .unwrap_or(&local_schema_path)
            );
            let instance_path = format!("{instance_prefix}{}", error.instance_path);
            let mut issue = LintIssue {
                file: file.into(),
                level: "error",
                message: error.to_string(),
                instance_path,
                schema_path,
                rule_id: None,
                human_message: Some(schema_message(&error)),
                details: Vec::new(),
            };
            if depth > 0
                && let ValidationErrorKind::Required { property } = &error.kind
                && let Some(property) = property.as_str()
            {
                issue.instance_path.push('/');
                issue
                    .instance_path
                    .push_str(&property.replace('~', "~0").replace('/', "~1"));
            }
            if matches!(
                error.kind,
                ValidationErrorKind::AnyOf | ValidationErrorKind::OneOfNotValid
            ) && depth < MAX_UNION_DEPTH
            {
                issue.details = union_details(schema, &error, &mut issue, depth)?;
            }
            // Root kind dispatch is knowable. Return the actual branch errors in machine
            // output too; nested union elaboration remains a human-only presentation detail.
            if depth == 0 && local_schema_path == "/oneOf" && !issue.details.is_empty() {
                issues.extend(issue.details);
            } else {
                issues.push(issue);
            }
        }
    }
    Ok(issues)
}

fn resolve<'a>(root: &'a Value, mut node: &'a Value) -> (&'a Value, Option<&'a str>) {
    let mut definition = None;
    for _ in 0..32 {
        let Some(reference) = node.get("$ref").and_then(Value::as_str) else {
            break;
        };
        let Some(pointer) = reference.strip_prefix('#') else {
            break;
        };
        let Some(target) = root.pointer(pointer) else {
            break;
        };
        definition = pointer.strip_prefix("/$defs/");
        node = target;
    }
    (node, definition)
}

fn schema_node<'a>(root: &'a Value, path: &str) -> Option<(&'a Value, Option<&'a str>)> {
    let mut node = root;
    let mut definition = None;
    for token in path.strip_prefix('/')?.split('/') {
        let (resolved, name) = resolve(root, node);
        node = resolved;
        definition = name.or(definition);
        let token = token.replace("~1", "/").replace("~0", "~");
        node = if let Some(array) = node.as_array() {
            array.get(token.parse::<usize>().ok()?)?
        } else {
            node.get(&token)?
        };
    }
    Some((node, definition))
}

fn union_details(
    schema: &Value,
    error: &ValidationError<'_>,
    issue: &mut LintIssue,
    depth: usize,
) -> Result<Vec<LintIssue>> {
    let Some((alternatives, definition)) = schema_node(schema, &error.schema_path.to_string())
    else {
        return Ok(Vec::new());
    };
    let Some(branches) = alternatives.as_array() else {
        return Ok(Vec::new());
    };
    if definition == Some("packageRef") {
        issue.human_message = Some("Expected a package reference: \"@ns/name@1.2.3\" or {\"name\":\"@ns/name\",\"version\":\"1.2.3\"} (version is optional).".into());
        return Ok(Vec::new());
    }
    if definition == Some("loopTransitionTarget") {
        let terminals: Vec<_> = branches
            .iter()
            .flat_map(|branch| {
                resolve(schema, branch)
                    .0
                    .get("enum")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
            })
            .collect();
        issue.human_message = Some(format!(
            "Expected a phase ID or a terminal target: {}.",
            terminals.join(", ")
        ));
        return Ok(Vec::new());
    }
    if error.instance_path.to_string() == "/entrypoint/command" {
        let interpreters: Vec<_> = branches
            .iter()
            .flat_map(|branch| {
                resolve(schema, branch)
                    .0
                    .get("enum")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
            })
            .collect();
        issue.human_message = Some(format!(
            "Expected a bare interpreter ({}) or an absolute interpreter path.",
            interpreters.join(", ")
        ));
        return Ok(Vec::new());
    }

    if definition == Some("agentMemoryBinding") {
        issue.human_message =
            Some("Specify at least one Memory binding selector: spaces or operations.".into());
        return Ok(Vec::new());
    }

    let resolved: Vec<_> = branches
        .iter()
        .map(|branch| resolve(schema, branch).0)
        .collect();
    // Only schema-authored const discriminators select branches, never authored labels.
    for discriminator in ["kind", "type", "action", "mode"] {
        let choices: Vec<Vec<&str>> = resolved
            .iter()
            .map(|branch| {
                branch
                    .get("properties")
                    .and_then(|p| p.get(discriminator))
                    .map(|property| {
                        if let Some(value) = property.get("const").and_then(Value::as_str) {
                            vec![value]
                        } else {
                            property
                                .get("enum")
                                .and_then(Value::as_array)
                                .into_iter()
                                .flatten()
                                .filter_map(Value::as_str)
                                .collect()
                        }
                    })
                    .unwrap_or_default()
            })
            .collect();
        let expected: Vec<_> = choices.iter().flatten().copied().collect();
        if choices.iter().all(|choice| !choice.is_empty())
            && let Some(actual) = error.instance.get(discriminator)
            && !expected.iter().any(|item| actual.as_str() == Some(item))
        {
            issue.instance_path.push('/');
            issue.instance_path.push_str(discriminator);
            issue.human_message = Some(format!(
                "Expected `{discriminator}` to be one of: {}.",
                expected.join(", ")
            ));
            return Ok(Vec::new());
        }
    }
    let candidates: Vec<_> = resolved
        .iter()
        .enumerate()
        .filter(|(_, branch)| {
            if let Some(kind) = branch.get("type").and_then(Value::as_str) {
                let matches = match kind {
                    "object" => error.instance.is_object(),
                    "array" => error.instance.is_array(),
                    "string" => error.instance.is_string(),
                    "boolean" => error.instance.is_boolean(),
                    "number" => error.instance.is_number(),
                    "integer" => error.instance.is_i64() || error.instance.is_u64(),
                    "null" => error.instance.is_null(),
                    _ => true,
                };
                if !matches {
                    return false;
                }
            }
            branch
                .get("properties")
                .and_then(Value::as_object)
                .is_none_or(|properties| {
                    properties.iter().all(|(key, property)| {
                        let Some(actual) = error.instance.get(key) else {
                            return true;
                        };
                        property
                            .get("const")
                            .is_none_or(|expected| expected == actual)
                            && property
                                .get("enum")
                                .and_then(Value::as_array)
                                .is_none_or(|values| values.contains(actual))
                    })
                })
        })
        .collect();
    let candidates = if candidates.len() > 1 {
        candidates
            .into_iter()
            .filter(|(_, branch)| shape_matches(schema, branch, &error.instance, 0))
            .collect()
    } else {
        candidates
    };
    if candidates.len() != 1 {
        return Ok(Vec::new());
    }
    let index = candidates[0].0;
    let wrapper = json!({ "$defs": schema.get("$defs").cloned().unwrap_or(json!({})), "allOf": [branches[index].clone()] });
    // Custom schemas may reference locations outside the copied definitions. A
    // presentation-only expansion must not discard the original validation error.
    let details = collect_schema_issues(
        &wrapper,
        &issue.file,
        &error.instance,
        &issue.instance_path,
        "/allOf/0",
        &format!("{}/{index}", issue.schema_path),
        depth + 1,
    )
    .unwrap_or_default();
    Ok(details)
}

fn shape_matches(schema: &Value, branch: &Value, value: &Value, depth: usize) -> bool {
    if depth >= MAX_UNION_DEPTH {
        return false;
    }
    let branch = resolve(schema, branch).0;
    if let Some(alternatives) = branch
        .get("oneOf")
        .or_else(|| branch.get("anyOf"))
        .and_then(Value::as_array)
    {
        return alternatives
            .iter()
            .any(|child| shape_matches(schema, child, value, depth + 1));
    }
    match branch.get("type").and_then(Value::as_str) {
        Some("string") if !value.is_string() => return false,
        Some("object") if !value.is_object() => return false,
        Some("array") if !value.is_array() => return false,
        _ => {}
    }
    if branch.get("additionalProperties") == Some(&Value::Bool(false))
        && let Some(object) = value.as_object()
        && let Some(properties) = branch.get("properties").and_then(Value::as_object)
    {
        return object.keys().all(|key| properties.contains_key(key));
    }
    true
}

fn schema_message(error: &ValidationError<'_>) -> String {
    match &error.kind {
        ValidationErrorKind::AnyOf | ValidationErrorKind::OneOfNotValid => {
            "Value does not match any allowed alternative.".into()
        }
        ValidationErrorKind::OneOfMultipleValid => {
            "Value matches more than one allowed alternative; exactly one is required.".into()
        }
        ValidationErrorKind::Required { property } => format!("Missing required field {property}."),
        ValidationErrorKind::AdditionalProperties { unexpected } => {
            format!("Unknown field(s): {}.", unexpected.join(", "))
        }
        ValidationErrorKind::Type { kind } => format!("Expected type {kind:?}."),
        ValidationErrorKind::Enum { options } => format!("Expected one of: {options}."),
        ValidationErrorKind::Constant { expected_value } => {
            format!("Expected constant {}.", scalar_summary(expected_value))
        }
        ValidationErrorKind::Pattern { pattern } => {
            format!("Value must match pattern {}.", bounded(pattern))
        }
        ValidationErrorKind::MinItems { limit } => format!("Expected at least {limit} item(s)."),
        ValidationErrorKind::MinLength { limit } => {
            format!("Expected at least {limit} character(s).")
        }
        ValidationErrorKind::MaxItems { limit } => format!("Expected at most {limit} item(s)."),
        ValidationErrorKind::MaxLength { limit } => {
            format!("Expected at most {limit} character(s).")
        }
        _ => format!(
            "Schema constraint `{}` was not satisfied.",
            error
                .schema_path
                .to_string()
                .rsplit('/')
                .next()
                .unwrap_or("value")
        ),
    }
}

fn scalar_summary(value: &Value) -> String {
    match value {
        Value::Object(_) => "object".into(),
        Value::Array(_) => "array".into(),
        _ => bounded(&value.to_string()),
    }
}

fn bounded(message: &str) -> String {
    let escaped = message.replace('\n', "\\n").replace('\r', "\\r");
    let mut chars = escaped.chars();
    let mut output: String = chars.by_ref().take(MAX_MESSAGE_CHARS).collect();
    if chars.next().is_some() {
        output.push_str("...");
    }
    output
}

fn at_or_below(path: &str, parent: &str) -> bool {
    path == parent
        || path
            .strip_prefix(parent)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn flatten<'a>(issue: &'a LintIssue, out: &mut Vec<&'a LintIssue>) {
    if issue.details.is_empty() {
        out.push(issue);
    } else {
        for detail in &issue.details {
            flatten(detail, out);
        }
    }
}

pub(crate) fn render_lint_report(report: &LintFileReport) -> String {
    let mut flat = Vec::new();
    for issue in &report.issues {
        flatten(issue, &mut flat);
    }
    let mut seen = HashSet::new();
    let mut selected: Vec<_> = flat
        .iter()
        .copied()
        .filter(|issue| {
            let union =
                issue.schema_path.ends_with("/oneOf") || issue.schema_path.ends_with("/anyOf");
            if union
                && flat.iter().any(|other| {
                    other.level == "error"
                        && at_or_below(&other.instance_path, &issue.instance_path)
                        && (other.schema_path.is_empty()
                            || (issue.human_message.as_deref()
                                == Some("Value does not match any allowed alternative.")
                                && other.human_message.as_deref()
                                    != Some("Value does not match any allowed alternative.")))
                })
            {
                return false;
            }
            seen.insert((
                issue.level,
                issue.instance_path.as_str(),
                issue.message.as_str(),
                issue.human_message.as_deref().unwrap_or(&issue.message),
                issue.rule_id.as_deref(),
            ))
        })
        .collect();
    selected.sort_by_key(|issue| (issue.level != "error", !issue.schema_path.is_empty()));
    let mut output = format!("{} {}\n", if report.ok { "✓" } else { "✗" }, report.file);
    for issue in selected {
        let badge = if issue.level == "error" {
            "ERROR"
        } else {
            "WARN "
        };
        let rule = issue
            .rule_id
            .as_ref()
            .map(|id| format!(" [{id}]"))
            .unwrap_or_default();
        writeln!(
            output,
            "  [{badge}]{rule} {}",
            bounded(issue.human_message.as_deref().unwrap_or(&issue.message))
        )
        .unwrap();
        writeln!(
            output,
            "        at instance {}",
            if issue.instance_path.is_empty() {
                "/"
            } else {
                &issue.instance_path
            }
        )
        .unwrap();
        if !issue.schema_path.is_empty() {
            writeln!(output, "        vs schema  {}", issue.schema_path).unwrap();
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{resolve_schema_source, validate_manifest_value};

    fn report(mut value: Value) -> LintFileReport {
        let (ok, issues) = validate_manifest_value(
            &resolve_schema_source(None),
            "fixture/agent.json",
            &mut value,
            false,
        )
        .unwrap();
        LintFileReport {
            file: "fixture/agent.json".into(),
            ok,
            issues,
        }
    }

    fn memory() -> Value {
        serde_json::from_str(include_str!(
            "../tests/fixtures/lint/invalid-memory-operation.json"
        ))
        .unwrap()
    }

    fn loop_manifest() -> Value {
        serde_json::from_str(include_str!("../tests/fixtures/lint/invalid-loop.json")).unwrap()
    }

    #[test]
    fn root_kind_dispatch_exposes_actual_required_field_in_all_formats() {
        let report = report(
            json!({"kind":"agent","name":"missing-tools","version":"0.1.0","description":"An agent."}),
        );
        assert!(!report.ok);
        assert!(report.issues.iter().any(
            |issue| issue.instance_path == "/tools" && issue.schema_path == "/oneOf/0/required"
        ));
        assert!(
            !report
                .issues
                .iter()
                .any(|issue| issue.schema_path == "/oneOf")
        );
        let text = render_lint_report(&report);
        assert!(
            text.contains("/tools") && text.contains("Missing required field"),
            "{text}"
        );
        assert!(!text.contains("\"description\":"), "{text}");
    }

    #[test]
    fn known_union_cliffs_have_actionable_bounded_messages_and_pointers() {
        let mut trigger = memory();
        trigger["memory"]["operations"]["refresh"] = json!({
            "type":"transform", "description":"Refresh", "trigger":{"type":"unknown"},
            "inputs":[{"space":"profile","record_type":"preference"}],
            "output":{"space":"profile","record_type":"preference"},
            "source_handling":"retain", "preserve_provenance":true
        });
        let mut policy = loop_manifest();
        policy["loop"]["transitions"][0]["to"] = json!("$end");
        policy["loop"]["error_policy"] = json!({"tool_failure":{"action":"unknown"}});
        let cases = vec![
            (
                json!({"kind":"agent","name":"bad-ref","version":"0.1.0","description":"Agent","tools":[{}]}),
                "/tools/0",
                "@ns/name@1.2.3",
            ),
            (
                trigger,
                "/memory/operations/refresh/trigger/type",
                "record_count",
            ),
            (
                memory(),
                "/memory/operations/refresh/type",
                "consolidate, transform, delete",
            ),
            (
                loop_manifest(),
                "/loop/transitions/0/to",
                "supported terminal",
            ),
            (policy, "/loop/error_policy/tool_failure/action", "retry"),
            (
                json!({"kind":"knowledge","name":"bad-knowledge","version":"0.1.0","description":"Knowledge","knowledge":{"mode":"unknown"}}),
                "/knowledge/mode",
                "context",
            ),
            (
                json!({"kind":"agent","name":"bad-binding","version":"0.1.0","description":"Agent","tools":[],"memory":["@ns/memory"],"bindings":{"global":{"memory":[{"package":"@ns/memory"}]}}}),
                "/bindings/global/memory/0",
                "spaces or operations",
            ),
            (
                json!({"kind":"tool","name":"bad-command","version":"0.1.0","description":"Tool","files":["main.py"],"entrypoint":{"command":"","args":["main.py"]},"inputs":{},"outputs":{}}),
                "/entrypoint/command",
                "absolute interpreter path",
            ),
        ];
        for (value, path, expected) in cases {
            let report = report(value);
            assert!(!report.ok, "expected invalid fixture for {path}");
            let text = render_lint_report(&report);
            assert!(
                text.contains(path) && text.contains(expected),
                "{path}: expected {expected}, got:\n{text}"
            );
            assert!(
                text.len() < 8000,
                "unbounded output for {path}: {}",
                text.len()
            );
            assert!(
                !text.contains("\"record_types\":"),
                "containing object leaked: {text}"
            );
        }
    }

    #[test]
    fn semantic_errors_precede_schema_fallback_and_duplicates_are_removed() {
        let report = report(loop_manifest());
        let text = render_lint_report(&report);
        let semantic = report
            .issues
            .iter()
            .find(|issue| issue.level == "error" && issue.schema_path.is_empty())
            .expect("Loop fixture must exercise a real semantic error");
        assert!(text.contains(&semantic.message), "{text}");
        let first = text.lines().find(|line| line.contains("[ERROR]")).unwrap();
        assert!(first.contains(&semantic.message), "{text}");
        let headings: Vec<_> = text
            .lines()
            .filter(|line| line.contains("[ERROR]"))
            .collect();
        let unique: HashSet<_> = headings.iter().copied().collect();
        assert_eq!(headings.len(), unique.len(), "duplicate errors: {text}");
    }

    #[test]
    fn sole_structural_error_is_not_suppressed_and_large_values_never_echo() {
        let report = report(
            json!({"kind":"agent","name":"invalid","version":"0.1.0","description":"x".repeat(20000),"tools":[{"not-a-ref":"sensitive fixture content".repeat(1000)}]}),
        );
        let text = render_lint_report(&report);
        assert!(
            text.contains("[ERROR]") && text.contains("/tools/0"),
            "{text}"
        );
        assert!(!text.contains("sensitive fixture content"));
        assert!(text.len() < 2000, "{text}");
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.message.contains("sensitive fixture content")),
            "machine diagnostics should remain independent of human truncation"
        );
    }

    #[test]
    fn custom_schema_expansion_preserves_errors_with_non_definition_references() {
        let schema = json!({
            "type": "object",
            "properties": {"expected": {"type": "string"}},
            "oneOf": [
                {"properties": {"kind": {"const": "tool"}, "name": {"$ref": "#/properties/expected"}}, "required": ["name"]},
                {"properties": {"kind": {"const": "agent"}}}
            ]
        });
        let issues = schema_issues(&schema, "agent.json", &json!({"kind":"tool", "name":42}))
            .expect("human-only expansion must not break canonical custom-schema validation");
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].schema_path, "/oneOf");
        let text = render_lint_report(&LintFileReport {
            file: "agent.json".into(),
            ok: false,
            issues,
        });
        assert!(text.contains("[ERROR]") && text.contains("oneOf"), "{text}");
    }

    #[test]
    fn machine_fields_stay_stable_and_optional_external_rule_id_renders_next_to_path() {
        let mut report = report(
            json!({"kind":"agent","name":"invalid","version":"0.1.0","description":"Agent"}),
        );
        let raw = serde_json::to_value(&report).unwrap();
        let keys: HashSet<_> = raw["issues"][0]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            HashSet::from(["file", "level", "message", "instance_path", "schema_path"])
        );
        report.issues[0].rule_id = Some("external.example".into());
        assert!(render_lint_report(&report).contains("[external.example]"));
        assert_eq!(
            serde_json::to_value(&report).unwrap()["issues"][0]["rule_id"],
            "external.example"
        );
    }

    #[test]
    fn duplicate_union_paths_collapse_but_distinct_constraints_survive() {
        let text = render_lint_report(&report(memory()));
        assert_eq!(
            text.matches("consolidate, transform, delete").count(),
            1,
            "{text}"
        );
        let schema =
            json!({"allOf":[{"type":"string","pattern":"^a"},{"type":"string","pattern":"z$"}]});
        let structural = LintFileReport {
            file: "fixture/agent.json".into(),
            ok: false,
            issues: schema_issues(&schema, "fixture/agent.json", &json!("x")).unwrap(),
        };
        let text = render_lint_report(&structural);
        assert_eq!(text.matches("[ERROR]").count(), 2, "{text}");
        assert!(text.contains("^a") && text.contains("z$"), "{text}");
        assert!(!at_or_below(
            "/memory/operations_extra",
            "/memory/operations"
        ));
    }

    #[test]
    fn bounds_handle_unicode_and_control_characters() {
        let text = bounded(&"é\n".repeat(1000));
        assert!(text.chars().count() <= MAX_MESSAGE_CHARS + 3);
        assert!(!text.contains('\n'));
        assert!(text.ends_with("..."));
    }
}
