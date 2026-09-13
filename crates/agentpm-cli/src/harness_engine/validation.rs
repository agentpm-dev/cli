use super::effective_phase::{
    active_memory_space, memory_read_mode_label, memory_read_mode_matches,
    memory_write_operation_label,
};
use super::*;
use crate::manifest::{MemoryRetrievalMode, MemorySpaceModel};

pub(super) fn validate_semantic_action(
    action: &SemanticAction,
    phase: &EffectivePhase,
) -> Result<(), String> {
    match action {
        SemanticAction::PersistenceReviewComplete => {
            Err("persistence_review_complete is only valid during Memory write review.".into())
        }
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
                return Err(memory_record_type_mismatch_feedback(
                    phase,
                    "memory_read",
                    package,
                    space,
                    record_type,
                ));
            }
            if limit == &Some(0) {
                return Err("Memory read limit must be greater than 0.".into());
            }
            match mode {
                MemoryReadMode::Key => {
                    if record_id.is_none() && !matches!(memory.model, MemorySpaceModel::Document) {
                        return Err(memory_read_argument_feedback(
                            phase,
                            package,
                            space,
                            "Memory key read requires a record_id for non-document spaces.",
                        ));
                    }
                    if query.is_some() || !filter.is_empty() {
                        return Err(memory_read_argument_feedback(
                            phase,
                            package,
                            space,
                            "Memory key read must not include query or filter arguments.",
                        ));
                    }
                }
                MemoryReadMode::Filter => {
                    if record_id.is_some() {
                        return Err(memory_read_argument_feedback(
                            phase,
                            package,
                            space,
                            "Memory filter read must not include record_id.",
                        ));
                    }
                    if query.is_some() {
                        return Err(memory_read_argument_feedback(
                            phase,
                            package,
                            space,
                            "Memory filter read must not include a query.",
                        ));
                    }
                    validate_memory_filter_paths(memory, filter)?;
                }
                MemoryReadMode::Chronological => {
                    if record_id.is_some() || query.is_some() || !filter.is_empty() {
                        return Err(memory_read_argument_feedback(
                            phase,
                            package,
                            space,
                            "Memory chronological read must not include record_id, query, or filter arguments.",
                        ));
                    }
                }
                MemoryReadMode::FullText => {
                    let Some(query) = query else {
                        return Err(memory_read_argument_feedback(
                            phase,
                            package,
                            space,
                            "Memory full_text read requires a query.",
                        ));
                    };
                    if query.trim().is_empty() {
                        return Err(memory_read_argument_feedback(
                            phase,
                            package,
                            space,
                            "Memory full_text query must not be empty.",
                        ));
                    }
                    if record_id.is_some() {
                        return Err(memory_read_argument_feedback(
                            phase,
                            package,
                            space,
                            "Memory full_text read must not include record_id.",
                        ));
                    }
                    if !filter.is_empty() {
                        return Err(memory_read_argument_feedback(
                            phase,
                            package,
                            space,
                            "Memory full_text read must not include filter.",
                        ));
                    }
                }
                MemoryReadMode::Semantic => {
                    let Some(query) = query else {
                        return Err(memory_read_argument_feedback(
                            phase,
                            package,
                            space,
                            "Memory semantic read requires a query.",
                        ));
                    };
                    if query.trim().is_empty() {
                        return Err(memory_read_argument_feedback(
                            phase,
                            package,
                            space,
                            "Memory semantic query must not be empty.",
                        ));
                    }
                    if record_id.is_some() {
                        return Err(memory_read_argument_feedback(
                            phase,
                            package,
                            space,
                            "Memory semantic read must not include record_id.",
                        ));
                    }
                    validate_memory_filter_paths(memory, filter)?;
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
                return Err(memory_record_type_mismatch_feedback(
                    phase,
                    "memory_write",
                    package,
                    space,
                    record_type,
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

fn memory_read_argument_feedback(
    phase: &EffectivePhase,
    package: &str,
    space: &str,
    message: &str,
) -> String {
    let Some(memory) = active_memory_space(phase, package, space) else {
        return message.into();
    };
    let mut shapes = Vec::new();
    for mode in &memory.retrieval_modes {
        match mode {
            MemoryRetrievalMode::Key if matches!(memory.model, MemorySpaceModel::Document) => {
                shapes.push("key document read: omit record_id, query, and filter")
            }
            MemoryRetrievalMode::Key => {
                shapes.push("key record read: provide record_id and omit query/filter")
            }
            MemoryRetrievalMode::Chronological => shapes
                .push("chronological read: omit record_id, query, and filter; limit is optional"),
            MemoryRetrievalMode::Filter => {
                shapes.push("filter read: provide filter and omit record_id/query")
            }
            MemoryRetrievalMode::FullText => {
                shapes.push("full_text read: provide non-empty query and omit record_id/filter")
            }
            MemoryRetrievalMode::Semantic => shapes.push(
                "semantic read: provide non-empty query, optionally filter, and omit record_id",
            ),
        }
    }
    if shapes.is_empty() {
        return message.into();
    }
    format!(
        "{message} Authorized Memory read shapes for `{package}/{space}`: {}.",
        shapes.join("; ")
    )
}

fn memory_record_type_mismatch_feedback(
    phase: &EffectivePhase,
    action_kind: &str,
    package: &str,
    selected_space: &str,
    record_type: &str,
) -> String {
    let mut message = format!(
        "Memory record type `{record_type}` is not declared for selected Memory space `{selected_space}` in package `{package}`."
    );
    let mut alternatives = phase
        .active_memory
        .iter()
        .filter(|memory| {
            memory.package == package
                && memory.space != selected_space
                && memory
                    .record_types
                    .iter()
                    .any(|candidate| candidate.name == record_type)
        })
        .filter_map(|memory| {
            let identity = format!("{}/{}", memory.package, memory.space);
            phase
                .capability_catalog
                .iter()
                .any(|descriptor| {
                    descriptor.action_kind == action_kind && descriptor.identity == identity
                })
                .then_some(identity)
        })
        .collect::<Vec<_>>();
    alternatives.sort();
    alternatives.dedup();
    if !alternatives.is_empty() {
        message.push_str(&format!(
            " Authorized alternative Memory {} action(s) for record type `{record_type}`: {}.",
            if action_kind == "memory_read" {
                "read"
            } else {
                "write"
            },
            alternatives
                .iter()
                .map(|identity| format!("`{identity}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    message
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness_runtime::MemoryRecordTypeRuntimeSnapshot;
    use serde_json::json;

    fn memory_phase(model: MemorySpaceModel, modes: Vec<MemoryRetrievalMode>) -> EffectivePhase {
        let memory = MemorySpaceRuntimeSnapshot {
            package: "@zack/memory".into(),
            package_version: "0.1.0".into(),
            space: "notes".into(),
            model,
            description: "Notes.".into(),
            root: None,
            runtime: "local".into(),
            source: "agent_binding".into(),
            state: "available".into(),
            readiness_reason: None,
            binding_scope: "global".into(),
            scope_keys: vec!["user".into()],
            retrieval_modes: modes,
            semantic: None,
            append_only: false,
            record_types: vec![MemoryRecordTypeRuntimeSnapshot {
                name: "note".into(),
                schema_version: "1.0.0".into(),
                content_schema: json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "tag": { "type": "string" }
                    }
                }),
            }],
        };
        EffectivePhase {
            phase_id: "remember".into(),
            tools_allowed: None,
            knowledge_allowed: None,
            memory_read_allowed: None,
            memory_write_allowed: None,
            authored_profile_candidates: Vec::new(),
            active_profiles: Vec::new(),
            active_tools: Vec::new(),
            active_skills: Vec::new(),
            active_knowledge: Vec::new(),
            active_memory: vec![memory],
            active_memory_operations: Vec::new(),
            capability_catalog: vec![CapabilityDescriptor {
                action_kind: "memory_read".into(),
                identity: "@zack/memory/notes".into(),
                description: "Read notes.".into(),
                source: "agent_binding".into(),
            }],
            suppressed_capabilities: Vec::new(),
        }
    }

    fn memory_read(
        mode: MemoryReadMode,
        record_id: Option<&str>,
        filter: BTreeMap<String, Value>,
        query: Option<&str>,
    ) -> SemanticAction {
        SemanticAction::MemoryRead {
            package: "@zack/memory".into(),
            space: "notes".into(),
            mode,
            record_id: record_id.map(str::to_string),
            record_type: Some("note".into()),
            filter,
            query: query.map(str::to_string),
            limit: None,
        }
    }

    #[test]
    fn memory_read_argument_errors_name_authorized_alternative_shapes() {
        let phase = memory_phase(
            MemorySpaceModel::Collection,
            vec![
                MemoryRetrievalMode::Key,
                MemoryRetrievalMode::Chronological,
                MemoryRetrievalMode::Filter,
            ],
        );

        let err = validate_semantic_action(
            &memory_read(MemoryReadMode::Key, None, BTreeMap::new(), None),
            &phase,
        )
        .expect_err("collection key read without record_id should fail");

        assert!(err.contains("Memory key read requires a record_id"));
        assert!(err.contains("Authorized Memory read shapes for `@zack/memory/notes`"));
        assert!(err.contains("key record read: provide record_id"));
        assert!(err.contains("chronological read"));
        assert!(err.contains("filter read"));
        assert!(!err.contains("full_text read"));
        assert!(!err.contains("semantic read"));
    }

    #[test]
    fn memory_read_validation_rejects_filter_record_id_and_full_text_filter() {
        let phase = memory_phase(
            MemorySpaceModel::Collection,
            vec![MemoryRetrievalMode::Filter, MemoryRetrievalMode::FullText],
        );

        let filter_err = validate_semantic_action(
            &memory_read(
                MemoryReadMode::Filter,
                Some("mem-1"),
                BTreeMap::from([("tag".into(), json!("release"))]),
                None,
            ),
            &phase,
        )
        .expect_err("filter read with record_id should fail");
        assert!(filter_err.contains("Memory filter read must not include record_id"));

        let full_text_err = validate_semantic_action(
            &memory_read(
                MemoryReadMode::FullText,
                None,
                BTreeMap::from([("tag".into(), json!("release"))]),
                Some("launch"),
            ),
            &phase,
        )
        .expect_err("full_text read with filter should fail");
        assert!(full_text_err.contains("Memory full_text read must not include filter"));
    }

    #[test]
    fn memory_read_validation_preserves_semantic_filter_candidate_restriction() {
        let phase = memory_phase(
            MemorySpaceModel::Collection,
            vec![MemoryRetrievalMode::Semantic],
        );

        validate_semantic_action(
            &memory_read(
                MemoryReadMode::Semantic,
                None,
                BTreeMap::from([("tag".into(), json!("release"))]),
                Some("launch"),
            ),
            &phase,
        )
        .expect("semantic read with filter should remain valid");
    }
}
