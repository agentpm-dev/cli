use super::effective_phase::{
    active_memory_space, memory_read_mode_label, memory_write_operation_label,
};
use super::*;
use crate::harness_runtime::{EmbeddingProvider, ModelProviderSelection};

pub(super) fn local_memory_read_mode(mode: MemoryReadMode) -> Result<LocalMemoryReadMode> {
    match mode {
        MemoryReadMode::Key => Ok(LocalMemoryReadMode::Key),
        MemoryReadMode::Filter => Ok(LocalMemoryReadMode::Filter),
        MemoryReadMode::Chronological => Ok(LocalMemoryReadMode::Chronological),
        MemoryReadMode::FullText => Ok(LocalMemoryReadMode::FullText),
        MemoryReadMode::Semantic => Ok(LocalMemoryReadMode::Semantic),
    }
}

pub(super) fn local_memory_write_operation(
    operation: MemoryWriteOperation,
) -> LocalMemoryWriteOperation {
    match operation {
        MemoryWriteOperation::Create => LocalMemoryWriteOperation::Create,
        MemoryWriteOperation::Upsert => LocalMemoryWriteOperation::Upsert,
        MemoryWriteOperation::Update => LocalMemoryWriteOperation::Update,
        MemoryWriteOperation::Delete => LocalMemoryWriteOperation::Delete,
        MemoryWriteOperation::Archive => LocalMemoryWriteOperation::Archive,
    }
}

pub(super) fn memory_runtime_failure_output(
    phase: &EffectivePhase,
    action: &SemanticAction,
    code: &str,
    message: &str,
) -> Value {
    let (package, space) = match action {
        SemanticAction::MemoryRead { package, space, .. }
        | SemanticAction::MemoryWrite { package, space, .. } => (package.as_str(), space.as_str()),
        _ => ("", ""),
    };
    let memory = active_memory_space(phase, package, space);
    json!({
        "ok": false,
        "package": package,
        "package_version": memory.map(|memory| memory.package_version.as_str()),
        "space": space,
        "runtime": memory.map(|memory| memory.runtime.as_str()),
        "error": {
            "code": code,
            "message": message
        }
    })
}

pub(super) fn resolved_memory_scope(
    memory: &MemorySpaceRuntimeSnapshot,
    runtime_scopes: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>> {
    let mut scope = BTreeMap::new();
    for key in &memory.scope_keys {
        let value = runtime_scopes.get(key).with_context(|| {
            format!(
                "Memory scope key `{key}` is unresolved for space `{}`",
                memory.space
            )
        })?;
        if value.is_empty() {
            bail!(
                "Memory scope key `{key}` has an empty value for space `{}`",
                memory.space
            );
        }
        scope.insert(key.clone(), value.clone());
    }
    Ok(scope)
}

fn memory_semantic_config(
    session: &HarnessSession,
    memory: &MemorySpaceRuntimeSnapshot,
) -> Result<LocalMemorySemanticConfig> {
    let semantic = memory.semantic.as_ref().ok_or_else(|| {
        LocalMemoryActionError::backend(
            "Memory semantic retrieval is unavailable for this local Memory space",
        )
    })?;
    if semantic.dimensions == 0 {
        return Err(LocalMemoryActionError::contract_violation(
            "Memory semantic dimensions must be greater than 0",
        )
        .into());
    }
    if session
        .runtime_snapshot
        .capability_candidates
        .iter()
        .any(|candidate| {
            candidate.kind == "embedding_provider"
                && candidate.identity == semantic.provider
                && matches!(candidate.state.as_str(), "unavailable" | "suppressed")
        })
    {
        return Err(LocalMemoryActionError::backend(format!(
            "EmbeddingProvider `{}` is unavailable for Memory semantic retrieval",
            semantic.provider
        ))
        .into());
    }
    Ok(LocalMemorySemanticConfig {
        embedding_provider: semantic.provider.clone(),
        embedding_model: semantic.model.clone(),
        dimensions: semantic.dimensions,
        normalized: semantic.normalized,
    })
}

fn emit_memory_embedding_events(
    engine: &HarnessEngine,
    session: &mut HarnessSession,
    phase: &EffectivePhase,
    action: &SemanticAction,
    result: &ActionDispatchResult,
    action_source: Option<&str>,
    phase_execution_id: &str,
) -> Result<()> {
    if result.usage.embedding_requests == 0 {
        return Ok(());
    }
    let (package, space) = match action {
        SemanticAction::MemoryRead { package, space, .. }
        | SemanticAction::MemoryWrite { package, space, .. } => (package, space),
        _ => return Ok(()),
    };
    let run_id = engine.active_run(session)?.run_id().to_string();
    let Some(memory) = active_memory_space(phase, package, space) else {
        return Ok(());
    };
    let Some(semantic) = memory.semantic.as_ref() else {
        return Ok(());
    };
    let mut fields = BTreeMap::from([
        ("package".into(), json!(memory.package.clone())),
        (
            "package_version".into(),
            json!(memory.package_version.clone()),
        ),
        ("space".into(), json!(memory.space.clone())),
        ("runtime".into(), json!(memory.runtime.clone())),
        ("embedding_id".into(), json!(semantic.id.clone())),
        ("provider".into(), json!(semantic.provider.clone())),
        ("model".into(), json!(semantic.model.clone())),
        ("dimensions".into(), json!(semantic.dimensions)),
        ("metric".into(), json!(semantic.metric.clone())),
        ("normalized".into(), json!(semantic.normalized)),
        (
            "embedding_requests".into(),
            json!(result.usage.embedding_requests),
        ),
    ]);
    if let Some(source) = action_source {
        fields.insert("source".into(), json!(source));
    }
    session.emitter.emit(
        HarnessEventType::EmbeddingRequestStarted,
        HarnessEventPayload::Action {
            action_kind: "embedding_request".into(),
            identity: action.identity(),
            status: "requested".into(),
            fields: fields.clone(),
        },
        HarnessEventBuilder {
            run_id: Some(run_id.clone()),
            phase_execution_id: Some(phase_execution_id.to_string()),
            ..HarnessEventBuilder::default()
        },
    )?;
    if let Some(duration_ms) = result.embedding_request_duration_ms {
        fields.insert("duration_ms".into(), json!(duration_ms));
    }
    let embedding_failed = result.output.get("semantic_embedding_error").is_some()
        || result
            .output
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str)
            .is_some_and(|code| code.starts_with("embedding_") || code == "memory_runtime_failed");
    session.emitter.emit(
        if embedding_failed {
            HarnessEventType::EmbeddingRequestFailed
        } else {
            HarnessEventType::EmbeddingRequestCompleted
        },
        HarnessEventPayload::Action {
            action_kind: "embedding_request".into(),
            identity: action.identity(),
            status: if embedding_failed {
                "failed".into()
            } else {
                "completed".into()
            },
            fields,
        },
        HarnessEventBuilder {
            run_id: Some(run_id),
            phase_execution_id: Some(phase_execution_id.to_string()),
            ..HarnessEventBuilder::default()
        },
    )?;
    Ok(())
}

impl HarnessEngine {
    pub(super) fn dispatch_memory(
        &self,
        session: &mut HarnessSession,
        phase: &EffectivePhase,
        action: &SemanticAction,
        embedding_provider: &mut Option<Box<dyn EmbeddingProvider>>,
        action_source: Option<&str>,
        phase_execution_id: &str,
    ) -> Result<ActionDispatchResult> {
        let run_id = self.active_run(session)?.run_id().to_string();
        self.active_run_mut(session)?.usage.memory_requests += 1;
        let mut fields = action_trace_fields(action);
        if let Some(source) = action_source {
            fields.insert("source".into(), json!(source));
        }
        session.emitter.emit(
            action_request_event_type(action).expect("Memory actions have request events"),
            HarnessEventPayload::Action {
                action_kind: action.kind().into(),
                identity: action.identity(),
                status: "requested".into(),
                fields,
            },
            HarnessEventBuilder {
                run_id: Some(run_id.clone()),
                phase_execution_id: Some(phase_execution_id.to_string()),
                ..HarnessEventBuilder::default()
            },
        )?;

        let result = match self.execute_local_memory_action(
            session,
            phase,
            action,
            &run_id,
            phase_execution_id,
            embedding_provider,
            action_source,
        ) {
            Ok(output) => {
                let mut result = ActionDispatchResult::success(output);
                let embedding_requests = result
                    .output
                    .get("embedding_requests")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                if embedding_requests > 0 {
                    let usage = RunUsage {
                        embedding_requests,
                        ..Default::default()
                    };
                    result = result.with_usage(usage);
                    if let Some(duration_ms) = result
                        .output
                        .get("embedding_request_duration_ms")
                        .and_then(Value::as_u64)
                    {
                        result = result.with_embedding_request_duration_ms(duration_ms);
                    }
                }
                result
            }
            Err(err) => {
                let message = err.to_string();
                if local_memory_action_failure_category(&err) == Some(ActionFailureCategory::Schema)
                {
                    ActionDispatchResult::failure_with_category(
                        ActionFailureCategory::Schema,
                        message,
                    )
                } else {
                    ActionDispatchResult::success(memory_runtime_failure_output(
                        phase,
                        action,
                        local_memory_action_error_code(&err),
                        &message,
                    ))
                }
            }
        };
        self.merge_usage(session, &result.usage);
        emit_memory_embedding_events(
            self,
            session,
            phase,
            action,
            &result,
            action_source,
            phase_execution_id,
        )?;
        let output_failed = result
            .output
            .get("ok")
            .and_then(Value::as_bool)
            .is_some_and(|ok| !ok);
        let status = if result.ok && !output_failed {
            "completed"
        } else {
            "failed"
        };
        session.emitter.emit(
            action_dispatch_event_type(action, result.ok && !output_failed),
            HarnessEventPayload::Action {
                action_kind: action.kind().into(),
                identity: action.identity(),
                status: status.into(),
                fields: action_result_trace_fields(action, &result, 1, action_source),
            },
            HarnessEventBuilder {
                run_id: Some(run_id),
                phase_execution_id: Some(phase_execution_id.to_string()),
                ..HarnessEventBuilder::default()
            },
        )?;
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn execute_local_memory_action(
        &self,
        session: &mut HarnessSession,
        phase: &EffectivePhase,
        action: &SemanticAction,
        run_id: &str,
        phase_execution_id: &str,
        embedding_provider: &mut Option<Box<dyn EmbeddingProvider>>,
        action_source: Option<&str>,
    ) -> Result<Value> {
        let (package, space) = match action {
            SemanticAction::MemoryRead { package, space, .. }
            | SemanticAction::MemoryWrite { package, space, .. } => (package, space),
            _ => return Err(anyhow!("not a Memory action")),
        };
        let memory = active_memory_space(phase, package, space)
            .ok_or_else(|| anyhow!("Memory space `{space}` is not active"))?
            .clone();
        if memory.runtime != "local" {
            return Ok(json!({
                "ok": false,
                "package": memory.package,
                "package_version": memory.package_version,
                "space": memory.space,
                "runtime": memory.runtime,
                "error": {
                    "code": "memory_runtime_unavailable",
                    "message": memory.readiness_reason.unwrap_or_else(|| "configured MemoryRuntime is unavailable".into())
                }
            }));
        }
        let root = memory
            .root
            .as_ref()
            .ok_or_else(|| anyhow!("Memory package `{package}` has no resolved root"))?
            .clone();
        let manifest_path = root.join("agent.json");
        let (manifest_value, _) = load_manifest_value(&manifest_path)?;
        let manifest = parse_memory_manifest(&manifest_value)?;
        let contracts = session.memory_contract_cache.validate_and_load(&root)?;
        let scope = resolved_memory_scope(&memory, &session.runtime_snapshot.runtime_scopes)?;
        let now = Utc::now();
        match action {
            SemanticAction::MemoryRead {
                mode,
                record_id,
                record_type,
                filter,
                query,
                limit,
                ..
            } => {
                let read_mode = local_memory_read_mode(*mode)?;
                let request = LocalMemoryReadRequest {
                    package: &memory.package,
                    package_version: &memory.package_version,
                    manifest: &manifest,
                    space: &memory.space,
                    scope,
                    mode: read_mode,
                    record_id: record_id.clone(),
                    record_type: record_type.clone(),
                    filter: filter.clone(),
                    query: query.clone(),
                    limit: *limit,
                    now,
                };
                let (records, usage, embedding_request_duration_ms, output_metadata) =
                    if matches!(read_mode, LocalMemoryReadMode::Semantic) {
                        let semantic = memory_semantic_config(session, &memory)?;
                        let embedding_provider =
                            embedding_provider.as_deref_mut().ok_or_else(|| {
                                LocalMemoryActionError::backend(
                                    "Memory semantic read requires an active EmbeddingProvider",
                                )
                            })?;
                        let result = session.local_memory_runtime()?.read_records_semantic(
                            request,
                            &semantic,
                            embedding_provider,
                        )?;
                        let usage = RunUsage {
                            embedding_requests: result.embedding_requests,
                            ..Default::default()
                        };
                        let mut output_metadata = BTreeMap::new();
                        output_metadata
                            .insert("vectors_materialized", json!(result.vectors_materialized));
                        output_metadata.insert("vectors_pending", json!(result.vectors_pending));
                        (
                            result.records,
                            usage,
                            result.embedding_request_duration_ms,
                            output_metadata,
                        )
                    } else {
                        (
                            session.local_memory_runtime()?.read_records(request)?,
                            RunUsage::default(),
                            None,
                            BTreeMap::new(),
                        )
                    };
                let mut output = json!({
                    "ok": true,
                    "package": memory.package,
                    "package_version": memory.package_version,
                    "space": memory.space,
                    "mode": memory_read_mode_label(*mode),
                    "records": records,
                    "count": records.len()
                });
                if usage.embedding_requests > 0 {
                    output["embedding_requests"] = json!(usage.embedding_requests);
                    if let Some(duration_ms) = embedding_request_duration_ms {
                        output["embedding_request_duration_ms"] = json!(duration_ms);
                    }
                }
                for (key, value) in output_metadata {
                    output[key] = value;
                }
                Ok(output)
            }
            SemanticAction::MemoryWrite {
                operation,
                record_type,
                record_id,
                content,
                ..
            } => {
                let provenance = memory_write_provenance(
                    phase,
                    action,
                    session.runtime_snapshot.model.as_ref(),
                    run_id,
                    phase_execution_id,
                    action_source,
                );
                let request = LocalMemoryWriteRequest {
                    package: &memory.package,
                    package_version: &memory.package_version,
                    manifest: &manifest,
                    contracts: &contracts,
                    space: &memory.space,
                    record_type,
                    scope,
                    operation: local_memory_write_operation(*operation),
                    record_id: record_id.clone(),
                    content: content.clone(),
                    provenance,
                    now,
                };
                let semantic_config = memory_semantic_config(session, &memory).ok();
                let write_result = if let (Some(semantic), Some(provider)) =
                    (semantic_config.as_ref(), embedding_provider.as_deref_mut())
                {
                    session
                        .local_memory_runtime()?
                        .write_record_with_semantic(request, semantic, provider)?
                } else {
                    session.local_memory_runtime()?.write_record(request)?
                };
                let mut output = json!({
                    "ok": true,
                    "package": memory.package,
                    "package_version": memory.package_version,
                    "space": memory.space,
                    "operation": memory_write_operation_label(*operation),
                    "record_id": write_result.affected_record_id,
                    "record": write_result.record
                });
                if write_result.embedding_requests > 0 {
                    output["embedding_requests"] = json!(write_result.embedding_requests);
                    if let Some(duration_ms) = write_result.embedding_request_duration_ms {
                        output["embedding_request_duration_ms"] = json!(duration_ms);
                    }
                }
                if let Some(error) = write_result.semantic_embedding_error {
                    output["semantic_embedding_error"] = json!(error);
                }
                Ok(output)
            }
            _ => unreachable!(),
        }
    }
}

fn memory_write_provenance(
    phase: &EffectivePhase,
    action: &SemanticAction,
    model: Option<&ModelProviderSelection>,
    run_id: &str,
    phase_execution_id: &str,
    action_source: Option<&str>,
) -> Value {
    let operation = match action {
        SemanticAction::MemoryWrite { operation, .. } => memory_write_operation_label(*operation),
        _ => "",
    };
    let mut harness = serde_json::Map::new();
    harness.insert("kind".into(), json!("harness_direct_memory_write"));
    harness.insert("run_id".into(), json!(run_id));
    harness.insert("phase_execution_id".into(), json!(phase_execution_id));
    harness.insert("phase_id".into(), json!(phase.phase_id));
    harness.insert("action_kind".into(), json!("memory_write"));
    harness.insert("operation".into(), json!(operation));
    if let Some(source) = action_source {
        harness.insert("source".into(), json!(source));
    }
    if let Some(model) = model {
        harness.insert("model_provider".into(), json!(model.provider));
        harness.insert("model_id".into(), json!(model.model));
    }
    let mut provenance = serde_json::Map::new();
    provenance.insert("harness".into(), Value::Object(harness));
    Value::Object(provenance)
}

fn local_memory_action_failure_category(error: &anyhow::Error) -> Option<ActionFailureCategory> {
    error.chain().find_map(|cause| {
        cause
            .downcast_ref::<LocalMemoryActionError>()
            .and_then(|error| {
                error
                    .is_model_correctable()
                    .then_some(ActionFailureCategory::Schema)
            })
    })
}

fn local_memory_action_error_code(error: &anyhow::Error) -> &'static str {
    error
        .chain()
        .find_map(|cause| cause.downcast_ref::<LocalMemoryActionError>())
        .map_or("memory_runtime_failed", LocalMemoryActionError::code)
}
