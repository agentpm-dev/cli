use super::*;

pub(super) fn embedding_request_trace_fields(
    action: &SemanticAction,
    runtime: &RuntimeSnapshot,
    action_source: Option<&str>,
) -> Option<BTreeMap<String, Value>> {
    let SemanticAction::KnowledgeRequest {
        package,
        mode,
        query,
        ..
    } = action
    else {
        return None;
    };
    if matches!(
        mode,
        Some(crate::harness_runtime::KnowledgeRequestMode::ContextDocument)
    ) || query.is_none()
    {
        return None;
    }
    let knowledge = runtime
        .knowledge
        .iter()
        .find(|knowledge| knowledge.name == *package)?;
    if knowledge.mode != "vector" || knowledge.runtime != "local" {
        return None;
    }
    let embedding = knowledge.embedding.as_ref()?;
    let mut fields = BTreeMap::from([
        ("package".into(), json!(package)),
        ("package_version".into(), json!(knowledge.version.clone())),
        ("runtime".into(), json!(knowledge.runtime.clone())),
        ("embedding_id".into(), json!(embedding.id.clone())),
        ("provider".into(), json!(embedding.provider.clone())),
        ("model".into(), json!(embedding.model.clone())),
        ("dimensions".into(), json!(embedding.dimensions)),
        ("metric".into(), json!(embedding.metric.clone())),
        ("normalized".into(), json!(embedding.normalized)),
    ]);
    if let Some(source) = action_source {
        fields.insert("source".into(), json!(source));
    }
    Some(fields)
}

pub(super) fn embedding_failure_code(result: &ActionDispatchResult) -> Option<String> {
    let code = result
        .output
        .get("error")
        .and_then(|error| error.get("code"))
        .and_then(Value::as_str)?;
    if code.starts_with("embedding_") || code.starts_with("malformed_embedding_") {
        Some(code.to_string())
    } else {
        None
    }
}

impl HarnessEngine {
    pub(super) fn dispatch_knowledge(
        &self,
        session: &mut HarnessSession,
        knowledge: &mut dyn KnowledgeRuntime,
        action: &SemanticAction,
        action_source: Option<&str>,
        phase_execution_id: &str,
    ) -> Result<ActionDispatchResult> {
        let run_id = self.active_run(session)?.run_id().to_string();
        self.active_run_mut(session)?.usage.knowledge_requests += 1;
        let mut fields = action_trace_fields(action);
        if let Some(source) = action_source {
            fields.insert("source".into(), json!(source));
        }
        let embedding_fields =
            embedding_request_trace_fields(action, &session.runtime_snapshot, action_source);
        session.emitter.emit(
            HarnessEventType::KnowledgeRequestStarted,
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
        let dispatch_started = Instant::now();
        let result = knowledge.dispatch(action);
        let dispatch_duration_ms: u64 = dispatch_started
            .elapsed()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX);
        self.merge_usage(session, &result.usage);
        if result.usage.embedding_requests > 0
            && let Some(mut fields) = embedding_fields
        {
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
            let embedding_error_code = embedding_failure_code(&result);
            let embedding_failed = embedding_error_code.is_some();
            if let Some(code) = embedding_error_code {
                fields.insert("error_code".into(), json!(code));
            }
            fields.insert(
                "duration_ms".into(),
                json!(
                    result
                        .embedding_request_duration_ms
                        .unwrap_or(dispatch_duration_ms)
                ),
            );
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
                    run_id: Some(run_id.clone()),
                    phase_execution_id: Some(phase_execution_id.to_string()),
                    ..HarnessEventBuilder::default()
                },
            )?;
        }
        let status = if !result.ok
            || result
                .output
                .get("ok")
                .and_then(Value::as_bool)
                .is_some_and(|ok| !ok)
        {
            "failed"
        } else {
            "completed"
        };
        session.emitter.emit(
            action_dispatch_event_type(action, result.ok && status == "completed"),
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
    pub(super) fn apply_after_knowledge_retrieval_hook(
        &self,
        session: &mut HarnessSession,
        hooks: &mut dyn HookRuntime,
        phase_id: &str,
        phase_execution_id: &str,
        run_id: &str,
        result: ActionDispatchResult,
    ) -> Result<ActionDispatchResult> {
        let hook = HarnessHookId::AfterKnowledgeRetrieval;
        let binding_count = hooks.binding_count(&hook);
        if binding_count == 0 {
            return Ok(result);
        }
        let knowledge_result: crate::harness_runtime::knowledge::KnowledgeRuntimeResult =
            serde_json::from_value(result.output.clone())
                .map_err(|err| anyhow!("KnowledgeRuntime returned malformed result: {err}"))?;
        self.emit_hook_started(
            session,
            HookEventContext {
                run_id,
                phase_id,
                phase_execution_id,
                hook: &hook,
                binding_count,
            },
            BTreeMap::from([("package".into(), json!(knowledge_result.package.clone()))]),
        )?;
        match hooks.after_knowledge_retrieval(after_knowledge_retrieval_hook_from_result(
            phase_id,
            knowledge_result.clone(),
        )) {
            Ok(decision) => {
                self.emit_nonfatal_hook_failures(
                    session,
                    run_id,
                    phase_id,
                    phase_execution_id,
                    hooks,
                )?;
                let patched = decision.content.is_some() || decision.results.is_some();
                self.emit_hook_completed(
                    session,
                    HookEventContext {
                        run_id,
                        phase_id,
                        phase_execution_id,
                        hook: &hook,
                        binding_count,
                    },
                    BTreeMap::from([("patched".into(), json!(patched))]),
                )?;
                Ok(if patched {
                    let patched_result =
                        apply_after_knowledge_retrieval_decision(&knowledge_result, decision)
                            .map_err(|err| {
                                anyhow!(
                                    "after_knowledge_retrieval hook returned invalid patch: {err}"
                                )
                            })?;
                    ActionDispatchResult::success(json!(patched_result))
                } else {
                    result
                })
            }
            Err(err) => {
                let is_rejection = err.is_rejection();
                self.emit_nonfatal_hook_failures(
                    session,
                    run_id,
                    phase_id,
                    phase_execution_id,
                    hooks,
                )?;
                session.emitter.emit(
                    if is_rejection {
                        HarnessEventType::HookRejected
                    } else {
                        HarnessEventType::HookFailed
                    },
                    HarnessEventPayload::Lifecycle {
                        message: err.message.clone(),
                        fields: hook_event_fields(&hook, phase_id, binding_count, BTreeMap::new()),
                    },
                    HarnessEventBuilder {
                        run_id: Some(run_id.to_string()),
                        phase_execution_id: Some(phase_execution_id.to_string()),
                        ..HarnessEventBuilder::default()
                    },
                )?;
                Ok(ActionDispatchResult::failure(format!(
                    "after_knowledge_retrieval hook {} result: {}",
                    if is_rejection { "rejected" } else { "failed" },
                    err.message
                )))
            }
        }
    }
}
