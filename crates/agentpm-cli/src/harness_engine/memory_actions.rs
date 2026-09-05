use super::effective_phase::{
    active_memory_space, memory_read_mode_label, memory_write_operation_label,
};
use super::*;
use crate::harness_runtime::ModelProviderSelection;

pub(super) fn local_memory_read_mode(mode: MemoryReadMode) -> Result<LocalMemoryReadMode> {
    match mode {
        MemoryReadMode::Key => Ok(LocalMemoryReadMode::Key),
        MemoryReadMode::Filter => Ok(LocalMemoryReadMode::Filter),
        MemoryReadMode::Chronological => Ok(LocalMemoryReadMode::Chronological),
        MemoryReadMode::FullText => Ok(LocalMemoryReadMode::FullText),
        MemoryReadMode::Semantic => Err(anyhow!(
            "local semantic Memory retrieval is deferred to Milestone 14d"
        )),
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

impl HarnessEngine {
    pub(super) fn dispatch_memory(
        &self,
        session: &mut HarnessSession,
        phase: &EffectivePhase,
        action: &SemanticAction,
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
            action_source,
        ) {
            Ok(output) => ActionDispatchResult::success(output),
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

    pub(super) fn execute_local_memory_action(
        &self,
        session: &mut HarnessSession,
        phase: &EffectivePhase,
        action: &SemanticAction,
        run_id: &str,
        phase_execution_id: &str,
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
                let records =
                    session
                        .local_memory_runtime()?
                        .read_records(LocalMemoryReadRequest {
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
                        })?;
                Ok(json!({
                    "ok": true,
                    "package": memory.package,
                    "package_version": memory.package_version,
                    "space": memory.space,
                    "mode": memory_read_mode_label(*mode),
                    "records": records,
                    "count": records.len()
                }))
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
                let write_result =
                    session
                        .local_memory_runtime()?
                        .write_record(LocalMemoryWriteRequest {
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
                        })?;
                Ok(json!({
                    "ok": true,
                    "package": memory.package,
                    "package_version": memory.package_version,
                    "space": memory.space,
                    "operation": memory_write_operation_label(*operation),
                    "record_id": write_result.affected_record_id,
                    "record": write_result.record
                }))
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
