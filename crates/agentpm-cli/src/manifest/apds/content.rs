use super::*;
use crate::commands::knowledge::{KnowledgeBuildMode, build_vector_mode};
use jsonschema::{SchemaResolver, SchemaResolverError};
use std::sync::Arc;

struct LocalOnly;
impl SchemaResolver for LocalOnly {
    fn resolve(
        &self,
        _root: &Value,
        _url: &reqwest::Url,
        reference: &str,
    ) -> std::result::Result<Arc<Value>, SchemaResolverError> {
        Err(anyhow!(
            "External schema resolution is disabled: {reference}"
        ))
    }
}

pub(super) fn knowledge_metadata(value: &Value, file: &str, result: &mut Evaluation) {
    result.checked(&["APDS-KNOW-001", "APDS-KNOW-003"]);
    let mut seen = HashSet::new();
    for (i, index) in array(value.pointer("/knowledge/indexes"))
        .iter()
        .enumerate()
    {
        if index["embedding_id"] != value["knowledge"]["embedding"]["id"] {
            result.error(
                file,
                &format!("/knowledge/indexes/{i}/embedding_id"),
                "APDS-KNOW-003",
                "Index must identify the declared embedding payload.",
            );
        }
        if !seen.insert(index["id"].as_str()) {
            result.error(
                file,
                &format!("/knowledge/indexes/{i}/id"),
                "APDS-KNOW-003",
                "Duplicate index identifier.",
            );
        }
    }
}

fn resolve(root: &Path, relative: &str, directory: bool) -> Result<PathBuf> {
    let safe = parse_safe_relative_path(relative)?;
    let boundary = root.canonicalize()?;
    let path = boundary.join(safe).canonicalize()?;
    if !path.starts_with(&boundary) {
        bail!("Path escapes package root");
    }
    if directory && !path.is_dir() || !directory && !path.is_file() {
        bail!("Path has the wrong file type");
    }
    Ok(path)
}
fn supplied(
    root: &Path,
    relative: &str,
    directory: bool,
    file: &str,
    pointer: &str,
    rule: &str,
    result: &mut Evaluation,
) -> Option<PathBuf> {
    match resolve(root, relative, directory) {
        Ok(path) => Some(path),
        Err(error) => {
            result.error(file,pointer,rule,format!("Required packaged content `{relative}` is unavailable or unsafe: {error}. Restore the content or correct its declaration."));
            None
        }
    }
}
fn count(value: &Value, pointer: &str, actual: u64, file: &str, result: &mut Evaluation) {
    if let Some(declared) = value.pointer(pointer).and_then(Value::as_u64)
        && declared != actual
    {
        result.error(
            file,
            pointer,
            "APDS-KNOW-002",
            format!("Declared count/size {declared} disagrees with supplied content ({actual})."),
        );
    }
}

pub(super) fn check(
    value: &Value,
    file: &str,
    root: Option<&Path>,
    result: &mut Evaluation,
) -> Result<()> {
    let kind = value["kind"].as_str().unwrap_or("");
    let rules: &[&str] = match kind {
        "knowledge" => &["APDS-KNOW-002"],
        "memory" => &["APDS-MEM-006", "APDS-MEM-012"],
        "skill" => &["APDS-SKILL-001"],
        "template" => &["APDS-TEMPLATE-001"],
        "tool" => &["APDS-ID-004"],
        _ => &[],
    };
    let Some(root) = root else {
        result.missing(rules);
        return Ok(());
    };
    if !root.is_dir() {
        bail!("Complete package root must be an existing directory");
    }
    result.checked(rules);
    match kind {
        "knowledge" => knowledge(value, file, root, result)?,
        "memory" => memory(value, file, root, result)?,
        "skill" => {
            if let Some(relative) = value.pointer("/skill/entrypoint").and_then(Value::as_str) {
                supplied(
                    root,
                    relative,
                    false,
                    file,
                    "/skill/entrypoint",
                    "APDS-SKILL-001",
                    result,
                );
            }
            for field in ["references", "scripts"] {
                for (i, path) in array(value.pointer(&format!("/skill/{field}")))
                    .iter()
                    .enumerate()
                {
                    if let Some(relative) = path.as_str() {
                        supplied(
                            root,
                            relative,
                            false,
                            file,
                            &format!("/skill/{field}/{i}"),
                            "APDS-SKILL-001",
                            result,
                        );
                    }
                }
            }
        }
        "template" => {
            if let Some(relative) = value
                .pointer("/template/files_root")
                .and_then(Value::as_str)
                && let Some(directory) = supplied(
                    root,
                    relative,
                    true,
                    file,
                    "/template/files_root",
                    "APDS-TEMPLATE-001",
                    result,
                )
            {
                inspect_tree(
                    root,
                    &directory,
                    file,
                    "/template/files_root",
                    "APDS-TEMPLATE-001",
                    result,
                )?;
            }
        }
        "tool" => {
            for (i, path) in array(value.get("files")).iter().enumerate() {
                if let Some(relative) = path.as_str() {
                    let is_directory = root.join(relative).is_dir();
                    if let Some(path) = supplied(
                        root,
                        relative,
                        is_directory,
                        file,
                        &format!("/files/{i}"),
                        "APDS-ID-004",
                        result,
                    ) && is_directory
                    {
                        inspect_tree(
                            root,
                            &path,
                            file,
                            &format!("/files/{i}"),
                            "APDS-ID-004",
                            result,
                        )?;
                    }
                }
            }
        }
        _ => {}
    }
    for pointer in ["/readme", "/license/file"] {
        if let Some(relative) = value.pointer(pointer).and_then(Value::as_str) {
            supplied(root, relative, false, file, pointer, "APDS-ID-004", result);
        }
    }
    Ok(())
}

fn inspect_tree(
    root: &Path,
    directory: &Path,
    file: &str,
    pointer: &str,
    rule: &str,
    result: &mut Evaluation,
) -> Result<()> {
    let boundary = root.canonicalize()?;
    for entry in walkdir::WalkDir::new(directory).follow_links(false) {
        match entry {
            Ok(entry) => {
                if !entry
                    .path()
                    .canonicalize()
                    .is_ok_and(|p| p.starts_with(&boundary))
                {
                    result.error(
                        file,
                        pointer,
                        "APDS-ID-004",
                        "Content contains an unreadable path or an escaping link.",
                    );
                }
            }
            Err(_) => result.error(file, pointer, rule, "Declared content cannot be inspected."),
        }
    }
    Ok(())
}

fn knowledge(value: &Value, file: &str, root: &Path, result: &mut Evaluation) -> Result<()> {
    if value["knowledge"]["mode"] == "context" {
        let mut bytes = 0;
        let mut all = true;
        let documents = array(value.pointer("/knowledge/documents"));
        for (i, document) in documents.iter().enumerate() {
            if let Some(relative) = document["path"].as_str() {
                if let Some(path) = supplied(
                    root,
                    relative,
                    false,
                    file,
                    &format!("/knowledge/documents/{i}/path"),
                    "APDS-KNOW-002",
                    result,
                ) {
                    let content = fs::read(path)?;
                    bytes += content.len() as u64;
                    count(
                        value,
                        &format!("/knowledge/documents/{i}/bytes"),
                        content.len() as u64,
                        file,
                        result,
                    );
                    if let Some(hash) = document["sha256"].as_str() {
                        check_hash(
                            &content,
                            hash,
                            file,
                            &format!("/knowledge/documents/{i}/sha256"),
                            result,
                        );
                    }
                } else {
                    all = false;
                }
            }
        }
        if all {
            count(
                value,
                "/knowledge/context/document_count",
                documents.len() as u64,
                file,
                result,
            );
            count(value, "/knowledge/context/total_bytes", bytes, file, result);
        }
    } else if value["knowledge"]["mode"] == "vector" {
        let mut all = true;
        for pointer in [
            "/knowledge/corpus/chunks_path",
            "/knowledge/corpus/sources_path",
            "/knowledge/embedding/vectors_path",
        ] {
            if let Some(relative) = value.pointer(pointer).and_then(Value::as_str)
                && supplied(
                    root,
                    relative,
                    false,
                    file,
                    pointer,
                    "APDS-KNOW-002",
                    result,
                )
                .is_none()
            {
                all = false;
            }
        }
        for (i, index) in array(value.pointer("/knowledge/indexes"))
            .iter()
            .enumerate()
        {
            if let Some(relative) = index["path"].as_str() {
                let directory = root.join(relative).is_dir();
                if supplied(
                    root,
                    relative,
                    directory,
                    file,
                    &format!("/knowledge/indexes/{i}/path"),
                    "APDS-KNOW-002",
                    result,
                )
                .is_none()
                {
                    all = false;
                }
            }
        }
        // APDS does not mandate an encoding. Reuse AgentPM's existing read-only
        // decoder for the concrete float32 representation that it understands.
        let known = value
            .pointer("/knowledge/embedding/vectors_path")
            .and_then(Value::as_str)
            .is_some_and(|s| s.ends_with(".f32"));
        if all && known {
            if let Ok(manifest) = parse_knowledge_manifest(value) {
                match build_vector_mode(root, &manifest, KnowledgeBuildMode::Check) {
                    Ok(built) => {
                        for (path, actual) in [
                            ("/knowledge/corpus/chunk_count", built.chunk_count),
                            ("/knowledge/corpus/source_count", built.source_count),
                            ("/knowledge/embedding/vector_count", built.vector_count),
                        ] {
                            count(value, path, actual, file, result);
                        }
                        if let Some(hash) = value
                            .pointer("/knowledge/embedding/vectors_hash")
                            .and_then(Value::as_str)
                            && hash.starts_with("sha256:")
                            && hash != built.vectors_hash
                        {
                            result.error(
                                file,
                                "/knowledge/embedding/vectors_hash",
                                "APDS-KNOW-002",
                                "Vector hash disagrees with supplied bytes.",
                            );
                        }
                        for (i, index) in array(value.pointer("/knowledge/indexes"))
                            .iter()
                            .enumerate()
                        {
                            let Some(relative) = index["path"].as_str() else {
                                continue;
                            };
                            let Some(path) = supplied(
                                root,
                                relative,
                                root.join(relative).is_dir(),
                                file,
                                &format!("/knowledge/indexes/{i}/path"),
                                "APDS-KNOW-002",
                                result,
                            ) else {
                                continue;
                            };
                            let metadata_path = if path.is_dir() {
                                path.join("metadata.json")
                            } else {
                                path.clone()
                            };
                            // Resolve the file again to enforce the boundary through links.
                            let metadata_relative = if path.is_dir() {
                                Path::new(relative).join("metadata.json")
                            } else {
                                PathBuf::from(relative)
                            };
                            let Some(safe) = supplied(
                                root,
                                &metadata_relative.to_string_lossy(),
                                false,
                                file,
                                &format!("/knowledge/indexes/{i}/path"),
                                "APDS-KNOW-002",
                                result,
                            ) else {
                                continue;
                            };
                            let metadata: Value = match serde_json::from_slice(&fs::read(safe)?) {
                                Ok(v) => v,
                                Err(_) => {
                                    result.error(
                                        file,
                                        &format!("/knowledge/indexes/{i}/path"),
                                        "APDS-KNOW-002",
                                        "Index metadata is not JSON.",
                                    );
                                    continue;
                                }
                            };
                            if metadata["type"] != "agentpm-local"
                                || metadata["format_version"] != 1
                            {
                                result.status = Status::Unsupported;
                                result.missing(&["APDS-KNOW-002"]);
                                result.notes.push(format!(
                                    "Unsupported index representation at {}.",
                                    metadata_path.display()
                                ));
                                continue;
                            }
                            if let Some(hash) = value
                                .pointer("/knowledge/corpus/content_hash")
                                .and_then(Value::as_str)
                                && hash.starts_with("sha256:")
                                && hash != built.corpus_hash
                            {
                                result.error(file, "/knowledge/corpus/content_hash", "APDS-KNOW-002", "Corpus hash disagrees with the supplied agentpm-local representation.");
                            }
                            for (key, expected) in [
                                ("embedding_id", Value::String(built.embedding_id.clone())),
                                ("metric", value["knowledge"]["embedding"]["metric"].clone()),
                                (
                                    "normalized",
                                    value["knowledge"]["embedding"]["normalized"].clone(),
                                ),
                                (
                                    "chunks_path",
                                    value["knowledge"]["corpus"]["chunks_path"].clone(),
                                ),
                                (
                                    "sources_path",
                                    value["knowledge"]["corpus"]["sources_path"].clone(),
                                ),
                                (
                                    "vectors_path",
                                    value["knowledge"]["embedding"]["vectors_path"].clone(),
                                ),
                                ("dimensions", Value::from(built.dimensions)),
                                ("chunk_count", Value::from(built.chunk_count)),
                                ("source_count", Value::from(built.source_count)),
                                ("vector_count", Value::from(built.vector_count)),
                                (
                                    "source_chunks_hash",
                                    Value::String(built.chunks_hash.clone()),
                                ),
                                (
                                    "source_sources_hash",
                                    Value::String(built.sources_hash.clone()),
                                ),
                                (
                                    "source_vectors_hash",
                                    Value::String(built.vectors_hash.clone()),
                                ),
                                (
                                    "source_corpus_hash",
                                    Value::String(built.corpus_hash.clone()),
                                ),
                            ] {
                                if metadata[key] != expected {
                                    result.error(
                                        file,
                                        &format!("/knowledge/indexes/{i}/path"),
                                        "APDS-KNOW-002",
                                        format!(
                                            "Index metadata `{key}` disagrees with its payload."
                                        ),
                                    );
                                }
                            }
                        }
                    }
                    Err(error) => result.error(
                        file,
                        "/knowledge/embedding/vectors_path",
                        "APDS-KNOW-002",
                        format!("Cannot verify supplied vector corpus: {error}"),
                    ),
                }
            }
        } else if all {
            result.status = Status::Unsupported;
            result.missing(&["APDS-KNOW-002"]);
            result.notes.push("Vector representation is not supported by this validator; metadata validity does not verify its payload.".into());
        }
    }
    if let Some(relative) = value
        .pointer("/knowledge/provenance/sources_manifest_path")
        .and_then(Value::as_str)
    {
        supplied(
            root,
            relative,
            false,
            file,
            "/knowledge/provenance/sources_manifest_path",
            "APDS-KNOW-002",
            result,
        );
    }
    Ok(())
}
fn check_hash(bytes: &[u8], declared: &str, file: &str, path: &str, result: &mut Evaluation) {
    use sha2::{Digest, Sha256};
    let actual = format!("{:x}", Sha256::digest(bytes));
    if declared.strip_prefix("sha256:").unwrap_or(declared) != actual {
        result.error(
            file,
            path,
            "APDS-KNOW-002",
            "Declared SHA-256 disagrees with supplied content.",
        );
    }
}

fn memory(value: &Value, file: &str, root: &Path, result: &mut Evaluation) -> Result<()> {
    for (key, record) in value
        .pointer("/memory/record_types")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        let Some(relative) = record["schema"].as_str() else {
            continue;
        };
        let Some(path) = supplied(
            root,
            relative,
            false,
            file,
            &format!("/memory/record_types/{key}/schema"),
            "APDS-MEM-006",
            result,
        ) else {
            continue;
        };
        let label = path.to_string_lossy();
        let source: Value = match serde_json::from_slice(&fs::read(&path)?) {
            Ok(v) => v,
            Err(error) => {
                result.error(
                    &label,
                    "",
                    "APDS-MEM-006",
                    format!("Record schema is not JSON: {error}"),
                );
                continue;
            }
        };
        let (non_persistable, composed) =
            source_schema(&source, &label, "", false, &source, result);
        if non_persistable && composed {
            result.status = Status::Unsupported;
            result.missing(&["APDS-MEM-006"]);
            result.notes.push(format!("Durable-projection compatibility for composed governance schemas is not evaluated: {label}."));
        }
        // External references are rejected before compilation. The resolver also
        // denies external retrieval for references rebased by nested $id values.
        let mut options = JSONSchema::options();
        options
            .with_draft(Draft::Draft202012)
            .with_resolver(LocalOnly);
        if let Err(error) = options.compile(&source) {
            result.error(
                &label,
                &error.instance_path.to_string(),
                "APDS-MEM-006",
                format!("Invalid Draft 2020-12 record schema: {error}"),
            );
        }
    }
    Ok(())
}
fn source_schema(
    value: &Value,
    file: &str,
    path: &str,
    complex: bool,
    document: &Value,
    result: &mut Evaluation,
) -> (bool, bool) {
    let Some(object) = value.as_object() else {
        return (false, false);
    };
    let mut flags = (
        object.get("x-agentpm-persist") == Some(&Value::Bool(false)),
        [
            "allOf",
            "anyOf",
            "oneOf",
            "if",
            "then",
            "else",
            "not",
            "dependentSchemas",
        ]
        .iter()
        .any(|k| object.contains_key(*k)),
    );
    if object
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|r| r.strip_prefix('#'))
        .and_then(|r| document.pointer(r))
        .is_some_and(|target| target.get("x-agentpm-persist") == Some(&Value::Bool(false)))
    {
        flags.1 = true;
    }
    for (key, annotation) in object {
        if key.starts_with("x-agentpm-") {
            let valid = match key.as_str() {
                "x-agentpm-data-class" => matches!(
                    annotation.as_str(),
                    Some(
                        "public"
                            | "internal"
                            | "personal"
                            | "authentication"
                            | "financial"
                            | "health"
                            | "legal"
                            | "operational"
                            | "other"
                    )
                ),
                "x-agentpm-sensitivity" => matches!(
                    annotation.as_str(),
                    Some("low" | "moderate" | "high" | "critical")
                ),
                "x-agentpm-persist" | "x-agentpm-shareable" => annotation.is_boolean(),
                _ => false,
            };
            if !valid {
                result.error(
                    file,
                    &json_pointer_child(path, key),
                    "APDS-MEM-012",
                    "Unknown governance annotation or invalid annotation value.",
                );
            }
        }
    }
    if let Some(reference) = object.get("$ref").and_then(Value::as_str)
        && !reference.starts_with('#')
    {
        result.error(
            file,
            &json_pointer_child(path, "$ref"),
            "APDS-MEM-006",
            "Only in-document record-schema references are allowed.",
        );
    }
    for (i, required) in array(object.get("required")).iter().enumerate() {
        if !complex
            && let Some(key) = required.as_str()
            && object
                .get("properties")
                .and_then(|p| p.get(key))
                .and_then(|p| p.get("x-agentpm-persist"))
                == Some(&Value::Bool(false))
        {
            result.error(
                file,
                &format!("{path}/required/{i}"),
                "APDS-MEM-006",
                "A directly required property cannot be non-persistable.",
            );
        }
    }
    // Traverse schema locations, not examples/defaults or ordinary property names.
    for keyword in [
        "properties",
        "patternProperties",
        "$defs",
        "definitions",
        "dependentSchemas",
    ] {
        for (key, child) in object
            .get(keyword)
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            let child_flags = source_schema(
                child,
                file,
                &json_pointer_child(&json_pointer_child(path, keyword), key),
                complex || keyword == "dependentSchemas",
                document,
                result,
            );
            flags.0 |= child_flags.0;
            flags.1 |= child_flags.1;
        }
    }
    for keyword in [
        "items",
        "additionalProperties",
        "unevaluatedProperties",
        "unevaluatedItems",
        "contains",
        "propertyNames",
        "not",
        "if",
        "then",
        "else",
    ] {
        if let Some(child) = object.get(keyword) {
            let child_flags = source_schema(
                child,
                file,
                &json_pointer_child(path, keyword),
                complex || matches!(keyword, "not" | "if" | "then" | "else"),
                document,
                result,
            );
            flags.0 |= child_flags.0;
            flags.1 |= child_flags.1;
        }
    }
    for keyword in ["prefixItems", "allOf", "anyOf", "oneOf"] {
        for (i, child) in array(object.get(keyword)).iter().enumerate() {
            let child_flags = source_schema(
                child,
                file,
                &format!("{path}/{keyword}/{i}"),
                complex || keyword != "prefixItems",
                document,
                result,
            );
            flags.0 |= child_flags.0;
            flags.1 |= child_flags.1;
        }
    }
    flags
}
