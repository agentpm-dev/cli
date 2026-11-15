//! Offline APDS evaluation. Context is supplied by the caller; no resolver or Runner runs here.
use super::*;
use clap::ValueEnum;
use std::collections::BTreeSet;

mod content;
mod graph;

pub(crate) use graph::ResolvedGraph;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Level {
    Standalone,
    Resolved,
    Packaged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Status {
    Conformant,
    Nonconformant,
    Incomplete,
    Unsupported,
}

#[derive(Debug, Serialize)]
pub(crate) struct Evaluation {
    pub standard: Value,
    pub level: Level,
    pub status: Status,
    pub evaluated_rules: BTreeSet<String>,
    pub unevaluated: BTreeSet<String>,
    pub supported_standard: Value,
    pub runner_evaluated: bool,
    #[serde(skip)]
    pub issues: Vec<LintIssue>,
    pub notes: Vec<String>,
}

#[derive(Default)]
pub(crate) struct Context<'a> {
    pub graph: Option<&'a ResolvedGraph>,
    /// None means unavailable content, not a supplied empty package.
    pub package_root: Option<&'a Path>,
}

pub(crate) fn evaluate(
    value: &Value,
    file: &str,
    level: Level,
    context: Context<'_>,
) -> Result<Evaluation> {
    let mut result = Evaluation {
        standard: value.get("standard").cloned().unwrap_or(Value::Null),
        level,
        status: Status::Conformant,
        evaluated_rules: BTreeSet::new(),
        unevaluated: BTreeSet::new(),
        supported_standard: serde_json::json!({"id":"agentpm","version":"1.0.0"}),
        runner_evaluated: false,
        issues: Vec::new(),
        notes: Vec::new(),
    };
    if let Some(selector) = value.get("standard")
        && selector
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty())
        && selector
            .get("version")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty())
        && (selector["id"] != "agentpm" || selector["version"] != "1.0.0")
    {
        result.status = Status::Unsupported;
        result
            .notes
            .push("Unsupported declared standard; no substitute contract was applied.".into());
        return Ok(result);
    }
    intrinsic(value, file, &mut result)?;
    match level {
        Level::Standalone => {}
        Level::Resolved => graph::check(value, file, context.graph, &mut result)?,
        Level::Packaged => content::check(value, file, context.package_root, &mut result)?,
    }
    result.finish();
    Ok(result)
}

impl Evaluation {
    fn checked(&mut self, rules: &[&str]) {
        self.evaluated_rules
            .extend(rules.iter().map(|r| r.to_string()));
    }
    fn missing(&mut self, rules: &[&str]) {
        self.unevaluated.extend(rules.iter().map(|r| r.to_string()));
    }
    fn error(&mut self, file: &str, path: &str, rule: &str, message: impl Into<String>) {
        self.evaluated_rules.insert(rule.into());
        self.issues.push(LintIssue {
            file: file.into(),
            level: "error",
            message: message.into(),
            instance_path: path.into(),
            schema_path: String::new(),
            rule_id: Some(rule.into()),
            human_message: None,
            details: Vec::new(),
        });
    }
    fn finish(&mut self) {
        self.issues.sort_by(|a, b| {
            (&a.file, &a.instance_path, &a.rule_id, &a.message).cmp(&(
                &b.file,
                &b.instance_path,
                &b.rule_id,
                &b.message,
            ))
        });
        self.issues.dedup_by(|a, b| {
            a.file == b.file && a.instance_path == b.instance_path && a.rule_id == b.rule_id
        });
        self.status = if self.issues.iter().any(|i| i.level == "error") {
            Status::Nonconformant
        } else if self.status == Status::Unsupported {
            Status::Unsupported
        } else if !self.unevaluated.is_empty() {
            Status::Incomplete
        } else {
            Status::Conformant
        };
    }
}

fn rule_for(value: &Value, path: &str) -> &'static str {
    if path.starts_with("/standard") {
        "APDS-ID-001"
    } else if ["/name", "/description", "/version"]
        .iter()
        .any(|p| path.starts_with(p))
    {
        "APDS-ID-002"
    } else if path.starts_with("/bindings/mcp") {
        "APDS-BIND-005"
    } else if path.starts_with("/bindings") {
        "APDS-BIND-001"
    } else if path.starts_with("/loop/checkpoints") {
        "APDS-LOOP-005"
    } else if path.starts_with("/loop/error_policy") {
        "APDS-LOOP-006"
    } else if path.starts_with("/loop/transitions") {
        "APDS-LOOP-002"
    } else if path.starts_with("/loop") && value["kind"] == "loop" {
        "APDS-LOOP-001"
    } else if path.starts_with("/profile") && value["kind"] == "profile" {
        "APDS-PROFILE-001"
    } else if path.starts_with("/skill") {
        "APDS-SKILL-001"
    } else if path.starts_with("/template/dependencies") {
        "APDS-ID-003"
    } else if path.starts_with("/template") {
        "APDS-TEMPLATE-001"
    } else if path.starts_with("/memory") && value["kind"] == "memory" {
        if path.contains("/retention/") || path.ends_with("/trigger/every") {
            "APDS-MEM-004"
        } else if path.ends_with("/constraints/append_only") {
            "APDS-MEM-005"
        } else if path.contains("/retrieval/") {
            "APDS-MEM-003"
        } else if path.contains("/operations/")
            && (path.ends_with("/source_handling") || path.ends_with("/output_mode"))
        {
            "APDS-MEM-010"
        } else if path.contains("/operations/")
            && (path.ends_with("/inputs") || path.ends_with("/type"))
        {
            "APDS-MEM-007"
        } else {
            "APDS-MEM-001"
        }
    } else if path.starts_with("/knowledge") && value["kind"] == "knowledge" {
        "APDS-KNOW-001"
    } else if ["/entrypoint", "/runtime", "/files", "/inputs", "/outputs"]
        .iter()
        .any(|p| path.starts_with(p))
        && value["kind"] == "tool"
    {
        "APDS-TOOL-002"
    } else if [
        "/tools",
        "/skills",
        "/knowledge",
        "/memory",
        "/profiles",
        "/loop",
    ]
    .iter()
    .any(|p| path.starts_with(p))
    {
        "APDS-ID-003"
    } else {
        "APDS-GRAPH-001"
    }
}

fn intrinsic(value: &Value, file: &str, result: &mut Evaluation) -> Result<()> {
    let schema: Value = serde_json::from_str(EMBEDDED_MANIFEST_SCHEMA_JSON)?;
    result.checked(&[
        "APDS-ID-001",
        "APDS-ID-002",
        "APDS-ID-003",
        "APDS-GRAPH-001",
    ]);
    // Reuse the existing kind/union diagnostic substrate. Preserve schema paths.
    fn add_schema(mut issue: LintIssue, value: &Value, result: &mut Evaluation) {
        if !issue.details.is_empty() {
            for detail in issue.details {
                add_schema(detail, value, result);
            }
            return;
        }
        let path = &issue.instance_path;
        let rule = if issue.schema_path.ends_with("/pattern")
            && (path.starts_with("/skill/entrypoint")
                || path.starts_with("/template/files_root")
                || path.starts_with("/skill/references/")
                || path.starts_with("/skill/scripts/")
                || path.starts_with("/knowledge/documents/") && path.ends_with("/path")
                || path.starts_with("/memory/record_types/") && path.ends_with("/schema"))
        {
            "APDS-ID-004"
        } else {
            rule_for(value, path)
        };
        issue.rule_id = Some(rule.into());
        result.evaluated_rules.insert(rule.into());
        result.issues.push(issue);
    }
    for issue in crate::lint_output::schema_issues(&schema, file, value)? {
        if issue.schema_path == "/required" || issue.schema_path == "/additionalProperties" {
            continue;
        }
        add_schema(issue, value, result);
    }
    for field in schema["required"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if value.get(field).is_none() {
            let path = format!("/{field}");
            result.error(
                file,
                &path,
                rule_for(value, &path),
                format!("Required field `{field}` is missing."),
            );
        }
    }
    if let Some(object) = value.as_object() {
        for key in object
            .keys()
            .filter(|key| schema["properties"].get(*key).is_none())
        {
            result.error(
                file,
                &json_pointer_child("", key),
                "APDS-GRAPH-001",
                format!("Unknown manifest field `{key}`."),
            );
        }
    }
    // Typed local semantic checks already used by AgentPM. No filesystem context is
    // passed here; source files are evaluated only in the packaged-content scope.
    let mut local = Vec::new();
    match value["kind"].as_str() {
        Some("memory") => {
            if let Ok(m) = parse_memory_manifest(value) {
                local.extend(validate_memory_manifest_semantics(file, value, &m, None));
                result.checked(&[
                    "APDS-MEM-001",
                    "APDS-MEM-003",
                    "APDS-MEM-004",
                    "APDS-MEM-005",
                    "APDS-MEM-007",
                    "APDS-MEM-008",
                    "APDS-MEM-010",
                ]);
            }
        }
        Some("profile") => {
            if let Ok(m) = parse_profile_manifest(value) {
                local.extend(validate_profile_manifest_semantics(file, &m));
                result.checked(&["APDS-PROFILE-001"]);
            }
        }
        Some("loop") => {
            if let Ok(m) = parse_loop_manifest(value) {
                local.extend(validate_loop_manifest_semantics(file, &m));
                result.checked(&[
                    "APDS-LOOP-001",
                    "APDS-LOOP-002",
                    "APDS-LOOP-005",
                    "APDS-LOOP-006",
                ]);
            }
        }
        Some("agent") => {
            if let Ok(m) = serde_json::from_value::<AgentManifest>(value.clone()) {
                local.extend(validate_agent_bindings_semantics(file, &m));
                result.checked(&["APDS-BIND-001", "APDS-BIND-002", "APDS-BIND-005"]);
            }
        }
        _ => {}
    }
    for mut issue in local {
        // Existing authoring-policy checks remain in ordinary lint. Only portable
        // normative Profile uniqueness and Loop relationships belong in APDS.
        if value["kind"] == "profile"
            && !(issue.instance_path.starts_with("/profile/constraints/")
                && issue.instance_path.ends_with("/id"))
        {
            continue;
        }
        if value["kind"] == "loop" && issue.instance_path == "/loop/archetype" {
            continue;
        }
        if value["kind"] == "memory"
            && (issue.instance_path.ends_with("/inputs")
                || issue.instance_path.ends_with("/output"))
            && (issue.message.contains("references unknown space")
                || issue.message.contains("references record type"))
        {
            continue;
        }
        let mut rule = rule_for(value, &issue.instance_path);
        if value["kind"] == "loop" && issue.message.contains("unreachable") {
            rule = "APDS-LOOP-002";
        }
        if issue.instance_path == "/bindings/phases" {
            rule = "APDS-BIND-002";
        }
        if issue.message.contains("capacity") && issue.instance_path.contains("/trigger/") {
            rule = "APDS-MEM-008";
        }
        if issue.message.contains("retain_until_expiration") {
            if let Some((prefix, _)) = issue.instance_path.split_once("/inputs/") {
                issue.instance_path = format!("{prefix}/source_handling");
            }
            rule = "APDS-MEM-010";
        }
        if issue.message.contains("same space and record_type") {
            issue.instance_path =
                issue.instance_path.trim_end_matches("/output").to_string() + "/output_mode";
            rule = "APDS-MEM-010";
        }
        issue.rule_id = Some(rule.into());
        result.evaluated_rules.insert(rule.into());
        result.issues.push(issue);
    }
    if value["kind"] == "memory" {
        for (key, operation) in value
            .pointer("/memory/operations")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            let prefix = json_pointer_child("/memory/operations", key);
            let refs = array(operation.get("inputs"))
                .iter()
                .enumerate()
                .map(|(i, v)| (format!("{prefix}/inputs/{i}"), v))
                .chain(
                    operation
                        .get("output")
                        .into_iter()
                        .map(|v| (format!("{prefix}/output"), v)),
                );
            for (path, reference) in refs {
                if let Some(space) = reference["space"].as_str() {
                    if let Some(declared) = value["memory"]["spaces"].get(space) {
                        if let Some(record) = reference["record_type"].as_str()
                            && !array(declared.get("record_types"))
                                .iter()
                                .any(|v| v.as_str() == Some(record))
                        {
                            result.error(
                                file,
                                &format!("{path}/record_type"),
                                "APDS-MEM-001",
                                "Record type is not permitted by the selected space.",
                            );
                        }
                    } else {
                        result.error(
                            file,
                            &format!("{path}/space"),
                            "APDS-MEM-001",
                            "Operation references an undeclared space.",
                        );
                    }
                }
            }
        }
    }
    graph::local_dependencies(value, file, result);
    if value["kind"] == "tool" {
        result.checked(&["APDS-TOOL-002"]);
        if value.pointer("/runtime/type").and_then(Value::as_str) == Some("python")
            && let Some(dependencies) = value.pointer("/runtime/dependencies")
            && let Ok(requirements) = serde_json::from_value::<Vec<String>>(dependencies.clone())
            && let Err(error) = agentpm_sdk::python::normalize_requirements(&requirements)
        {
            result.error(
                file,
                "/runtime/dependencies",
                "APDS-TOOL-002",
                format!("Invalid Python distribution requirements: {error:#}"),
            );
        }
        if let (Some(runtime), Some(command)) = (
            value.pointer("/runtime/type").and_then(Value::as_str),
            value.pointer("/entrypoint/command").and_then(Value::as_str),
        ) && !is_interpreter_match(
            &canonical_interpreter(runtime),
            &canonical_interpreter(command),
        ) {
            result.error(
                file,
                "/runtime/type",
                "APDS-TOOL-002",
                "Runtime type must agree with the entrypoint interpreter.",
            );
        }
    }
    if value["kind"] == "template" {
        result.checked(&["APDS-TEMPLATE-001"]);
        let mut seen = HashSet::new();
        for (i, v) in array(value.pointer("/template/variables"))
            .iter()
            .enumerate()
        {
            if let Some(name) = v["name"].as_str()
                && !seen.insert(name)
            {
                result.error(
                    file,
                    &format!("/template/variables/{i}/name"),
                    "APDS-TEMPLATE-001",
                    "Duplicate Template variable name.",
                );
            }
        }
    }
    if value["kind"] == "loop" {
        let mut seen = HashSet::new();
        for (i, t) in array(value.pointer("/loop/transitions")).iter().enumerate() {
            let key = (t["from"].clone(), t["on"].clone());
            if !seen.insert((key.0.to_string(), key.1.to_string())) {
                result.error(
                    file,
                    &format!("/loop/transitions/{i}"),
                    "APDS-LOOP-002",
                    "Duplicate phase/outcome transition.",
                );
            }
        }
    }
    if value["kind"] == "knowledge" {
        content::knowledge_metadata(value, file, result);
    }
    Ok(())
}

fn array(value: Option<&Value>) -> &[Value] {
    value
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}
