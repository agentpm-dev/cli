use super::effective_phase::{
    active_memory_space, memory_read_mode_label, memory_read_mode_matches,
    memory_write_operation_label,
};
use super::*;

pub(super) fn validate_semantic_action(
    action: &SemanticAction,
    phase: &EffectivePhase,
) -> Result<(), String> {
    match action {
        SemanticAction::AgentPmTool { tool, arguments } => {
            let Some(tool_snapshot) = phase
                .active_tools
                .iter()
                .find(|candidate| candidate.name == *tool)
            else {
                return Err(format!(
                    "Tool `{tool}` is not available in the current EffectivePhase."
                ));
            };
            validate_json_schema_value(&tool_snapshot.input_schema, arguments)
                .map_err(|err| format!("Tool `{tool}` arguments are invalid: {err}"))
        }
        SemanticAction::SkillResourceRead { skill, resource } => {
            let Some(skill_snapshot) = phase
                .active_skills
                .iter()
                .find(|candidate| candidate.name == *skill)
            else {
                return Err(format!(
                    "Skill `{skill}` is not available in the current EffectivePhase."
                ));
            };
            if skill_snapshot
                .resources
                .iter()
                .any(|candidate| candidate.id == *resource)
            {
                Ok(())
            } else {
                Err(format!(
                    "Skill `{skill}` resource `{resource}` is not active in this phase."
                ))
            }
        }
        SemanticAction::KnowledgeRequest {
            package,
            mode,
            document,
            query,
            top_k,
            ..
        } => {
            let Some(knowledge) = phase
                .active_knowledge
                .iter()
                .find(|candidate| candidate.name == *package)
            else {
                return Err(format!(
                    "Knowledge package `{package}` is not available in the current EffectivePhase."
                ));
            };
            if top_k == &Some(0) {
                return Err("Knowledge request top_k must be greater than 0.".into());
            }
            let request_mode = mode.as_ref().cloned().unwrap_or_else(|| {
                if document.is_some() {
                    crate::harness_runtime::KnowledgeRequestMode::ContextDocument
                } else {
                    crate::harness_runtime::KnowledgeRequestMode::VectorQuery
                }
            });
            match request_mode {
                crate::harness_runtime::KnowledgeRequestMode::ContextDocument => {
                    let Some(document) = document else {
                        return Err(
                            "Context Knowledge request must include a document path.".into()
                        );
                    };
                    if query.is_some() {
                        return Err(
                            "Context Knowledge request must not include a vector query.".into()
                        );
                    }
                    if knowledge.mode != "context" {
                        return Err(format!(
                            "Knowledge package `{package}` does not support context-document requests."
                        ));
                    }
                    if !knowledge
                        .documents
                        .iter()
                        .any(|candidate| candidate.path == *document)
                    {
                        return Err(format!(
                            "Document `{document}` is not declared by Knowledge package `{package}`."
                        ));
                    }
                }
                crate::harness_runtime::KnowledgeRequestMode::VectorQuery => {
                    let Some(query) = query else {
                        return Err("Vector Knowledge request must include a query.".into());
                    };
                    if document.is_some() {
                        return Err(
                            "Vector Knowledge request must not include a context document.".into(),
                        );
                    }
                    if query.trim().is_empty() {
                        return Err("Vector Knowledge query must not be empty.".into());
                    }
                    if knowledge.mode != "vector" {
                        return Err(format!(
                            "Knowledge package `{package}` does not support vector-query requests."
                        ));
                    }
                }
            }
            Ok(())
        }
        SemanticAction::MemoryRead {
            package,
            space,
            mode,
            record_id,
            record_type,
            filter,
            query,
            limit,
        } => {
            let Some(memory) = active_memory_space(phase, package, space) else {
                return Err(format!(
                    "Memory space `{}` for package `{package}` is not available in the current EffectivePhase.",
                    space
                ));
            };
            if !memory
                .retrieval_modes
                .iter()
                .any(|candidate| memory_read_mode_matches(candidate, *mode))
            {
                return Err(format!(
                    "Memory space `{}` does not declare retrieval mode `{}`.",
                    space,
                    memory_read_mode_label(*mode)
                ));
            }
            if let Some(record_type) = record_type
                && !memory
                    .record_types
                    .iter()
                    .any(|candidate| candidate.name == *record_type)
            {
                return Err(format!(
                    "Memory record type `{record_type}` is not declared for space `{space}`."
                ));
            }
            if limit == &Some(0) {
                return Err("Memory read limit must be greater than 0.".into());
            }
            match mode {
                MemoryReadMode::Key => {
                    if record_id.is_none() && !matches!(memory.model, MemorySpaceModel::Document) {
                        return Err(
                            "Memory key read requires a record_id for non-document spaces.".into(),
                        );
                    }
                    if query.is_some() || !filter.is_empty() {
                        return Err(
                            "Memory key read must not include query or filter arguments.".into(),
                        );
                    }
                }
                MemoryReadMode::Filter => {
                    if query.is_some() {
                        return Err("Memory filter read must not include a query.".into());
                    }
                    validate_memory_filter_paths(memory, filter)?;
                }
                MemoryReadMode::Chronological => {
                    if record_id.is_some() || query.is_some() || !filter.is_empty() {
                        return Err(
                            "Memory chronological read must not include record_id, query, or filter arguments."
                                .into(),
                        );
                    }
                }
                MemoryReadMode::FullText => {
                    let Some(query) = query else {
                        return Err("Memory full_text read requires a query.".into());
                    };
                    if query.trim().is_empty() {
                        return Err("Memory full_text query must not be empty.".into());
                    }
                    if record_id.is_some() {
                        return Err("Memory full_text read must not include record_id.".into());
                    }
                }
                MemoryReadMode::Semantic => {
                    return Err("Memory semantic read is deferred to Milestone 14d.".into());
                }
            }
            Ok(())
        }
        SemanticAction::MemoryWrite {
            package,
            space,
            operation,
            record_type,
            record_id,
            content,
        } => {
            let Some(memory) = active_memory_space(phase, package, space) else {
                return Err(format!(
                    "Memory space `{}` for package `{package}` is not available in the current EffectivePhase.",
                    space
                ));
            };
            let Some(record_type_schema) = memory
                .record_types
                .iter()
                .find(|candidate| candidate.name == *record_type)
            else {
                return Err(format!(
                    "Memory record type `{record_type}` is not declared for space `{space}`."
                ));
            };
            if memory.append_only
                && matches!(
                    operation,
                    MemoryWriteOperation::Upsert
                        | MemoryWriteOperation::Update
                        | MemoryWriteOperation::Delete
                        | MemoryWriteOperation::Archive
                )
            {
                return Err(format!(
                    "Memory space `{space}` is append-only and only permits create."
                ));
            }
            match operation {
                MemoryWriteOperation::Create | MemoryWriteOperation::Upsert => {
                    if record_id.is_some() {
                        return Err(
                            "Memory create/upsert cannot assign an authoritative record_id.".into(),
                        );
                    }
                    validate_memory_write_content(record_type_schema, content)?;
                }
                MemoryWriteOperation::Update => {
                    if record_id.is_none() {
                        return Err("Memory update requires an existing record_id.".into());
                    }
                    validate_memory_write_content(record_type_schema, content)?;
                }
                MemoryWriteOperation::Delete | MemoryWriteOperation::Archive => {
                    if record_id.is_none() {
                        return Err(format!(
                            "Memory {} requires an existing record_id.",
                            memory_write_operation_label(*operation)
                        ));
                    }
                    if content.is_some() {
                        return Err(format!(
                            "Memory {} must not include content.",
                            memory_write_operation_label(*operation)
                        ));
                    }
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(super) fn validate_memory_write_content(
    record_type: &crate::harness_runtime::model::MemoryRecordTypeRuntimeSnapshot,
    content: &Option<Value>,
) -> Result<(), String> {
    let Some(content) = content else {
        return Err("Memory create/update requires record content.".into());
    };
    validate_json_schema_value(&record_type.content_schema, content).map_err(|err| {
        format!(
            "Memory content for record type `{}` is invalid: {err}",
            record_type.name
        )
    })
}

pub(super) fn validate_memory_filter_paths(
    memory: &MemorySpaceRuntimeSnapshot,
    filter: &BTreeMap<String, Value>,
) -> Result<(), String> {
    for path in filter.keys() {
        let segments = memory_filter_path_segments(path)?;
        let known = memory.record_types.iter().any(|record_type| {
            memory_schema_path_exists(
                &record_type.content_schema,
                &record_type.content_schema,
                &segments,
                0,
            )
        });
        if !known {
            return Err(format!(
                "Memory filter path `{path}` is not declared by any content contract for space `{}`.",
                memory.space
            ));
        }
    }
    Ok(())
}

pub(super) fn memory_filter_path_segments(path: &str) -> Result<Vec<&str>, String> {
    if path.is_empty() || path.split('.').any(str::is_empty) {
        return Err(format!("Memory filter path `{path}` is invalid."));
    }
    Ok(path.split('.').collect())
}

pub(super) fn memory_schema_path_exists(
    schema: &Value,
    root_schema: &Value,
    segments: &[&str],
    depth: usize,
) -> bool {
    if segments.is_empty() {
        return true;
    }
    if depth > 64 {
        return false;
    }
    let resolved_schema = resolve_schema_ref_for_memory_path(schema, root_schema).unwrap_or(schema);
    let Some(object) = resolved_schema.as_object() else {
        return false;
    };
    for keyword in ["allOf", "anyOf", "oneOf"] {
        if object
            .get(keyword)
            .and_then(Value::as_array)
            .is_some_and(|schemas| {
                schemas.iter().any(|schema| {
                    memory_schema_path_exists(schema, root_schema, segments, depth + 1)
                })
            })
        {
            return true;
        }
    }
    if let Some(items) = object.get("items")
        && memory_schema_path_exists(items, root_schema, segments, depth + 1)
    {
        return true;
    }
    object
        .get("properties")
        .and_then(Value::as_object)
        .and_then(|properties| properties.get(segments[0]))
        .is_some_and(|child| {
            memory_schema_path_exists(child, root_schema, &segments[1..], depth + 1)
        })
}

pub(super) fn resolve_schema_ref_for_memory_path<'a>(
    schema: &'a Value,
    root_schema: &'a Value,
) -> Option<&'a Value> {
    let reference = schema.as_object()?.get("$ref")?.as_str()?;
    let pointer = reference.strip_prefix('#')?;
    root_schema.pointer(pointer)
}

pub(super) fn validate_json_schema_value(schema: &Value, value: &Value) -> Result<(), String> {
    let schema = schema_for_standalone_compile(schema);
    let compiled = JSONSchema::options()
        .with_draft(Draft::Draft202012)
        .compile(&schema)
        .map_err(|err| format!("schema is invalid: {err}"))?;
    compiled.validate(value).map_err(|errors| {
        errors
            .map(|error| format!("{} at instance {}", error, error.instance_path))
            .collect::<Vec<_>>()
            .join("; ")
    })
}

pub(super) fn schema_for_standalone_compile(schema: &Value) -> Value {
    let mut schema = schema.clone();
    if let Some(object) = schema.as_object_mut()
        && let Some(Value::String(id)) = object.get("$id")
        && !id.contains(':')
    {
        object.remove("$id");
    }
    schema
}
