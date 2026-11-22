use super::*;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResolvedGraph {
    pub complete: bool,
    artifacts: Vec<Artifact>,
    edges: Vec<Edge>,
    /// Caller-owned, read-only interpretation views (never persisted to artifacts).
    #[serde(skip)]
    documents: BTreeMap<PathBuf, Value>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    id: String,
    identity: String,
    kind: String,
    version: String,
    manifest: PathBuf,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Edge {
    from: String,
    field: String,
    to: String,
}

impl ResolvedGraph {
    pub(crate) fn with_documents(mut self, documents: BTreeMap<PathBuf, Value>) -> Self {
        self.documents = documents;
        self
    }
    pub(crate) fn read(path: &Path) -> Result<Self> {
        let mut graph: Self = serde_json::from_slice(&fs::read(path)?)?;
        let base = path.parent().unwrap_or_else(|| Path::new("."));
        for node in &mut graph.artifacts {
            if node.manifest.is_relative() {
                node.manifest = base.join(&node.manifest);
            }
        }
        Ok(graph)
    }
}

fn reference(value: &Value) -> Option<(String, Option<String>)> {
    if let Some(text) = value.as_str() {
        if let Some((name, version)) = text.rsplit_once('@')
            && !name.is_empty()
        {
            return Some((name.into(), Some(version.into())));
        }
        Some((text.into(), None))
    } else {
        Some((
            value["name"].as_str()?.into(),
            value["version"].as_str().map(str::to_owned),
        ))
    }
}
fn declarations(value: &Value) -> Vec<(String, &'static str, &Value)> {
    let mut refs = Vec::new();
    let kind = value["kind"].as_str().unwrap_or("");
    let prefix = if kind == "template" {
        "/template/dependencies/"
    } else {
        "/"
    };
    for (group, target) in [
        ("tools", "tool"),
        ("skills", "skill"),
        ("knowledge", "knowledge"),
        ("memory", "memory"),
        ("profiles", "profile"),
        ("agents", "agent"),
    ] {
        if !(kind == "agent" && group != "agents"
            || kind == "skill" && group == "tools"
            || kind == "template")
        {
            continue;
        }
        let pointer = format!("{prefix}{group}");
        for (i, item) in array(value.pointer(&pointer)).iter().enumerate() {
            refs.push((format!("{pointer}/{i}"), target, item));
        }
    }
    if kind == "agent"
        && let Some(loop_ref) = value.get("loop")
    {
        refs.push(("/loop".into(), "loop", loop_ref));
    }
    refs
}

pub(super) fn local_dependencies(value: &Value, file: &str, result: &mut Evaluation) {
    let mut seen = BTreeSet::new();
    for (path, _, item) in declarations(value) {
        if let Some((identity, _)) = reference(item) {
            let collection = path.rsplit_once('/').map(|x| x.0).unwrap_or("");
            if !seen.insert((collection.to_owned(), identity)) {
                result.error(
                    file,
                    &path,
                    "APDS-GRAPH-003",
                    "Duplicate identity in one dependency collection.",
                );
            }
        }
    }
    result.checked(&["APDS-GRAPH-003"]);
}
fn version_matches(selector: Option<&str>, version: &str) -> bool {
    let Some(selector) = selector else {
        return true;
    };
    if let Ok(exact) = semver::Version::parse(selector) {
        return semver::Version::parse(version).is_ok_and(|v| v == exact);
    }
    let Ok(selected) = semver::Version::parse(version) else {
        return false;
    };
    let components: Vec<_> = selector.split('.').collect();
    components.len() <= 2
        && components.first().and_then(|v| v.parse::<u64>().ok()) == Some(selected.major)
        && (components.len() == 1 || components[1].parse::<u64>().ok() == Some(selected.minor))
}

pub(super) fn check(
    value: &Value,
    file: &str,
    graph: Option<&ResolvedGraph>,
    result: &mut Evaluation,
) -> Result<()> {
    let Some(graph) = graph else {
        if !declarations(value).is_empty() {
            result.missing(&["APDS-GRAPH-002"]);
        }
        if value.pointer("/bindings/phases").is_some() && value.get("loop").is_some() {
            result.missing(&["APDS-BIND-002"]);
        }
        if !array(value.pointer("/memory")).is_empty() && value.get("bindings").is_some() {
            result.missing(&["APDS-BIND-001"]);
        }
        return Ok(());
    };
    result.checked(&["APDS-GRAPH-002"]);
    let mut nodes = BTreeMap::new();
    let mut loaded = BTreeMap::new();
    for node in &graph.artifacts {
        if node.id == "root" || nodes.insert(node.id.as_str(), node).is_some() {
            bail!("Resolved graph artifact IDs must be unique and cannot be root");
        }
        if let Some(data) = graph.documents.get(&node.manifest) {
            loaded.insert(node.id.as_str(), data.clone());
        } else if let Ok(bytes) = fs::read(&node.manifest)
            && let Ok(data) = serde_json::from_slice::<Value>(&bytes)
        {
            loaded.insert(node.id.as_str(), data);
        }
    }
    let mut selections = BTreeMap::new();
    for edge in &graph.edges {
        if !nodes.contains_key(edge.to.as_str())
            || (edge.from != "root" && !nodes.contains_key(edge.from.as_str()))
        {
            bail!("Resolved graph edge references an unknown artifact");
        }
        if selections
            .insert((edge.from.as_str(), edge.field.as_str()), edge.to.as_str())
            .is_some()
        {
            bail!("Resolved graph selects more than one target for an edge");
        }
    }
    let mut pending = vec!["root"];
    let mut visited = BTreeSet::new();
    let mut unsupported_nodes = BTreeSet::new();
    while let Some(owner) = pending.pop() {
        if !visited.insert(owner) {
            continue;
        }
        if owner != "root" && !loaded.contains_key(owner) {
            result.missing(&["APDS-GRAPH-002"]);
            let node = nodes[owner];
            let label = node.manifest.to_string_lossy();
            if let Ok(bytes) = fs::read(&node.manifest) {
                if let Err(error) = serde_json::from_slice::<Value>(&bytes) {
                    result.error(
                        &label,
                        "",
                        "APDS-GRAPH-001",
                        format!("Dependency manifest is not JSON: {error}"),
                    );
                }
            } else {
                result.notes.push(format!(
                    "Dependency inspection context is unavailable: {label}."
                ));
            }
            continue;
        }
        let (data, label) = if owner == "root" {
            (value, file.to_owned())
        } else {
            (
                &loaded[owner],
                nodes[owner].manifest.to_string_lossy().into_owned(),
            )
        };
        if owner != "root" {
            let node = nodes[owner];
            if data["kind"] != node.kind
                || data["version"] != node.version
                || data["name"].as_str() != node.identity.rsplit('/').next()
            {
                result.error(
                    &label,
                    "/name",
                    "APDS-GRAPH-002",
                    "Supplied graph metadata disagrees with the embedded artifact identity.",
                );
            }
            let evaluation = evaluate(data, &label, Level::Standalone, Context::default())?;
            if evaluation.status == Status::Unsupported {
                result.status = Status::Unsupported;
                result
                    .notes
                    .push(format!("Unsupported dependency standard in {label}."));
                unsupported_nodes.insert(owner);
                continue;
            }
            result.issues.extend(evaluation.issues);
            result.evaluated_rules.extend(evaluation.evaluated_rules);
        }
        for (path, kind, declared) in declarations(data) {
            let Some(target) = selections.get(&(owner, path.as_str())).copied() else {
                if graph.complete {
                    result.error(
                        &label,
                        &path,
                        "APDS-GRAPH-002",
                        "Complete resolved graph omits a declared dependency edge.",
                    );
                } else {
                    result.missing(&["APDS-GRAPH-002"]);
                }
                continue;
            };
            let node = nodes[target];
            if let Some((identity, selector)) = reference(declared)
                && (identity != node.identity
                    || kind != node.kind
                    || !version_matches(selector.as_deref(), &node.version))
            {
                result.error(
                    &label,
                    &path,
                    "APDS-GRAPH-002",
                    "Selected dependency has incompatible kind, identity or version.",
                );
            }
            pending.push(target);
        }
    }
    // An extra selection for an undeclared field must never silently invent a dependency.
    for edge in &graph.edges {
        if !visited.contains(edge.from.as_str())
            || unsupported_nodes.contains(edge.from.as_str())
            || edge.from != "root" && !loaded.contains_key(edge.from.as_str())
        {
            continue;
        }
        let data = if edge.from == "root" {
            value
        } else {
            &loaded[edge.from.as_str()]
        };
        if !declarations(data)
            .iter()
            .any(|(path, _, _)| path == &edge.field)
        {
            let label = if edge.from == "root" {
                file.to_owned()
            } else {
                nodes[edge.from.as_str()]
                    .manifest
                    .to_string_lossy()
                    .into_owned()
            };
            result.error(
                &label,
                &edge.field,
                "APDS-GRAPH-002",
                "Graph edge is not a declared dependency.",
            );
        }
    }
    for &owner in &visited {
        if unsupported_nodes.contains(owner) || owner != "root" && !loaded.contains_key(owner) {
            continue;
        }
        let (value, label) = if owner == "root" {
            (value, file.to_owned())
        } else {
            (
                &loaded[owner],
                nodes[owner].manifest.to_string_lossy().into_owned(),
            )
        };
        let file = label.as_str();
        if value["kind"] != "agent" {
            continue;
        }
        if let Some(phases) = value.pointer("/bindings/phases").and_then(Value::as_object)
            && value.get("loop").is_some()
        {
            if let Some(target) = selections.get(&(owner, "/loop")) {
                if unsupported_nodes.contains(target) || !loaded.contains_key(target) {
                    result.missing(&["APDS-BIND-002"]);
                } else {
                    let loop_value = &loaded[target];
                    for phase in phases.keys() {
                        if !array(loop_value.pointer("/loop/phases"))
                            .iter()
                            .any(|p| p["id"].as_str() == Some(phase))
                        {
                            result.error(
                                file,
                                &json_pointer_child("/bindings/phases", phase),
                                "APDS-BIND-002",
                                "Binding phase is absent from the selected Loop.",
                            );
                        }
                    }
                    result.checked(&["APDS-BIND-002"]);
                }
            } else {
                result.missing(&["APDS-BIND-002"]);
            }
        }
        if let Some(bindings) = value.get("bindings") {
            let scopes = bindings
                .get("global")
                .into_iter()
                .map(|v| ("/bindings/global".to_owned(), v))
                .chain(
                    bindings
                        .get("phases")
                        .and_then(Value::as_object)
                        .into_iter()
                        .flatten()
                        .map(|(k, v)| (json_pointer_child("/bindings/phases", k), v)),
                );
            for (scope_path, scope) in scopes {
                for (i, binding) in array(scope.get("memory")).iter().enumerate() {
                    let identity = binding["package"].as_str().unwrap_or("");
                    let selected = declarations(value)
                        .into_iter()
                        .find(|(_, kind, item)| {
                            *kind == "memory"
                                && reference(item).is_some_and(|(name, _)| name == identity)
                        })
                        .and_then(|(path, _, _)| selections.get(&(owner, path.as_str())).copied());
                    if selected.is_some_and(|id| {
                        unsupported_nodes.contains(id) || !loaded.contains_key(id)
                    }) {
                        result.missing(&["APDS-BIND-001"]);
                        continue;
                    }
                    let memory = selected.and_then(|id| loaded.get(id));
                    if let Some(memory) = memory {
                        for (selector, collection) in
                            [("spaces", "spaces"), ("operations", "operations")]
                        {
                            for (j, key) in array(binding.get(selector)).iter().enumerate() {
                                if key.as_str().is_some_and(|key| {
                                    memory["memory"][collection].get(key).is_none()
                                }) {
                                    result.error(
                                        file,
                                        &format!("{scope_path}/memory/{i}/{selector}/{j}"),
                                        "APDS-BIND-001",
                                        "Selector is absent from the selected Memory Blueprint.",
                                    );
                                }
                            }
                        }
                        result.checked(&["APDS-BIND-001"]);
                    } else if !graph.complete {
                        result.missing(&["APDS-BIND-001"]);
                    }
                }
            }
        }
    }
    Ok(())
}
