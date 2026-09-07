use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomMemoryReadRequest {
    pub package: String,
    pub package_version: String,
    pub space: String,
    pub scope: BTreeMap<String, String>,
    pub mode: MemoryRetrievalMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_type: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub filter: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    pub now: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomMemoryWriteRequest {
    pub package: String,
    pub package_version: String,
    pub space: String,
    pub space_model: MemorySpaceModel,
    pub record_type: String,
    pub schema_version: String,
    pub scope: BTreeMap<String, String>,
    pub operation: CustomMemoryWriteOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Value>,
    pub provenance: Value,
    pub now: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomMemoryWriteOperation {
    Create,
    Upsert,
    Update,
    Delete,
    Archive,
}

impl From<LocalMemoryWriteOperation> for CustomMemoryWriteOperation {
    fn from(value: LocalMemoryWriteOperation) -> Self {
        match value {
            LocalMemoryWriteOperation::Create => Self::Create,
            LocalMemoryWriteOperation::Upsert => Self::Upsert,
            LocalMemoryWriteOperation::Update => Self::Update,
            LocalMemoryWriteOperation::Delete => Self::Delete,
            LocalMemoryWriteOperation::Archive => Self::Archive,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomMemoryRuntimeFailure {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomMemoryReadResult {
    pub ok: bool,
    pub package: String,
    pub package_version: String,
    pub space: String,
    pub mode: MemoryRetrievalMode,
    #[serde(default)]
    pub records: Vec<StoredMemoryRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_requests: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_request_duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vectors_materialized: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vectors_pending: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CustomMemoryRuntimeFailure>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomMemoryWriteResult {
    pub ok: bool,
    pub package: String,
    pub package_version: String,
    pub space: String,
    pub operation: CustomMemoryWriteOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record: Option<StoredMemoryRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_requests: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_request_duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_embedding_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CustomMemoryRuntimeFailure>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct CustomMemoryResultEnvelope {
    pub ok: bool,
    pub package: String,
    pub package_version: String,
    pub space: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CustomMemoryRuntimeFailure>,
}

pub struct CustomMemoryRuntime {
    spaces: BTreeMap<(String, String), MemorySpaceRuntimeRoute>,
    runtimes: HashMap<String, ServiceRuntime>,
    lifecycle_events: Option<crate::harness_runtime::service::ServiceLifecycleEmitter>,
}

#[derive(Debug, Clone)]
struct MemorySpaceRuntimeRoute {
    package: String,
    package_version: String,
    space: String,
    runtime: String,
}

impl CustomMemoryRuntime {
    pub(crate) fn new(
        spaces: Vec<crate::harness_runtime::model::MemorySpaceRuntimeSnapshot>,
        runtimes: HashMap<String, ServiceRuntime>,
    ) -> Self {
        Self::with_lifecycle(spaces, runtimes, None)
    }

    pub(crate) fn with_lifecycle(
        spaces: Vec<crate::harness_runtime::model::MemorySpaceRuntimeSnapshot>,
        runtimes: HashMap<String, ServiceRuntime>,
        lifecycle_events: Option<crate::harness_runtime::service::ServiceLifecycleEmitter>,
    ) -> Self {
        Self {
            spaces: spaces
                .into_iter()
                .map(|space| {
                    (
                        (space.package.clone(), space.space.clone()),
                        MemorySpaceRuntimeRoute {
                            package: space.package,
                            package_version: space.package_version,
                            space: space.space,
                            runtime: space.runtime,
                        },
                    )
                })
                .collect(),
            runtimes,
            lifecycle_events,
        }
    }

    pub fn dispatch_read(&mut self, request: CustomMemoryReadRequest) -> ActionDispatchResult {
        let Some(route) = self
            .spaces
            .get(&(request.package.clone(), request.space.clone()))
            .cloned()
        else {
            return ActionDispatchResult::failure_with_category(
                ActionFailureCategory::Resolution,
                format!(
                    "Memory space `{}/{}` is not available in the current EffectivePhase",
                    request.package, request.space
                ),
            );
        };
        let expected_mode = request.mode.clone();
        let payload = json!({ "request": request });
        match self.request_runtime("read", &route, payload) {
            Ok(output) => {
                match validate_custom_memory_read_result(&output, &route, expected_mode) {
                    Ok(()) => ActionDispatchResult::success(output),
                    Err(err) => ActionDispatchResult::success(
                        malformed_custom_memory_runtime_response(&route, err.to_string()),
                    ),
                }
            }
            Err(err) => ActionDispatchResult::success(custom_memory_runtime_failed_response(
                &route,
                err.to_string(),
            )),
        }
    }

    pub fn dispatch_write(&mut self, request: CustomMemoryWriteRequest) -> ActionDispatchResult {
        let Some(route) = self
            .spaces
            .get(&(request.package.clone(), request.space.clone()))
            .cloned()
        else {
            return ActionDispatchResult::failure_with_category(
                ActionFailureCategory::Resolution,
                format!(
                    "Memory space `{}/{}` is not available in the current EffectivePhase",
                    request.package, request.space
                ),
            );
        };
        let expected_operation = request.operation;
        let payload = json!({ "request": request });
        match self.request_runtime("write", &route, payload) {
            Ok(output) => {
                match validate_custom_memory_write_result(&output, &route, expected_operation) {
                    Ok(()) => ActionDispatchResult::success(output),
                    Err(err) => ActionDispatchResult::success(
                        malformed_custom_memory_runtime_response(&route, err.to_string()),
                    ),
                }
            }
            Err(err) => ActionDispatchResult::success(custom_memory_runtime_failed_response(
                &route,
                err.to_string(),
            )),
        }
    }

    fn request_runtime(
        &mut self,
        method: &str,
        route: &MemorySpaceRuntimeRoute,
        payload: Value,
    ) -> Result<Value> {
        let lifecycle_events = self.lifecycle_events.clone();
        match self.runtimes.get_mut(&route.runtime) {
            Some(ServiceRuntime::Process(client)) => client.request(method, payload),
            Some(ServiceRuntime::Host {
                invoker,
                request_timeout_ms,
            }) => {
                let emits_lifecycle_events = invoker.emits_lifecycle_events();
                let result = invoker.invoke_host_service(
                    "memory",
                    &route.runtime,
                    method,
                    payload,
                    *request_timeout_ms,
                );
                if let Err(err) = &result
                    && !emits_lifecycle_events
                {
                    emit_memory_host_service_failure(
                        lifecycle_events.as_ref(),
                        &route.runtime,
                        err.to_string(),
                    );
                }
                result
            }
            None => Err(anyhow!("MemoryRuntime `{}` is not active", route.runtime)),
        }
    }
}

fn validate_custom_memory_read_result(
    output: &Value,
    route: &MemorySpaceRuntimeRoute,
    expected_mode: MemoryRetrievalMode,
) -> Result<()> {
    if let Some(envelope) = validate_custom_memory_result_envelope(output, route)? {
        if envelope.ok {
            return Ok(());
        }
        return Ok(());
    }
    let result: CustomMemoryReadResult =
        serde_json::from_value(output.clone()).with_context(|| {
            format!(
                "MemoryRuntime `{}` returned malformed read result",
                route.runtime
            )
        })?;
    validate_custom_memory_common_result(
        result.ok,
        &result.package,
        &result.package_version,
        &result.space,
        result.error.as_ref(),
        route,
    )?;
    if result.mode != expected_mode {
        bail!(
            "MemoryRuntime `{}` returned mode `{:?}`, expected `{:?}`",
            route.runtime,
            result.mode,
            expected_mode
        );
    }
    Ok(())
}

fn validate_custom_memory_write_result(
    output: &Value,
    route: &MemorySpaceRuntimeRoute,
    expected_operation: CustomMemoryWriteOperation,
) -> Result<()> {
    if let Some(envelope) = validate_custom_memory_result_envelope(output, route)? {
        if envelope.ok {
            return Ok(());
        }
        return Ok(());
    }
    let result: CustomMemoryWriteResult =
        serde_json::from_value(output.clone()).with_context(|| {
            format!(
                "MemoryRuntime `{}` returned malformed write result",
                route.runtime
            )
        })?;
    validate_custom_memory_common_result(
        result.ok,
        &result.package,
        &result.package_version,
        &result.space,
        result.error.as_ref(),
        route,
    )?;
    if result.operation != expected_operation {
        bail!(
            "MemoryRuntime `{}` returned operation `{:?}`, expected `{:?}`",
            route.runtime,
            result.operation,
            expected_operation
        );
    }
    Ok(())
}

fn validate_custom_memory_common_result(
    ok: bool,
    package: &str,
    package_version: &str,
    space: &str,
    error: Option<&CustomMemoryRuntimeFailure>,
    route: &MemorySpaceRuntimeRoute,
) -> Result<()> {
    if package != route.package {
        bail!(
            "MemoryRuntime `{}` returned package `{package}`, expected `{}`",
            route.runtime,
            route.package
        );
    }
    if package_version != route.package_version {
        bail!(
            "MemoryRuntime `{}` returned package_version `{package_version}`, expected `{}`",
            route.runtime,
            route.package_version
        );
    }
    if space != route.space {
        bail!(
            "MemoryRuntime `{}` returned space `{space}`, expected `{}`",
            route.runtime,
            route.space
        );
    }
    if !ok && error.is_none() {
        bail!(
            "MemoryRuntime `{}` returned ok=false without an error",
            route.runtime
        );
    }
    Ok(())
}

fn validate_custom_memory_result_envelope(
    output: &Value,
    route: &MemorySpaceRuntimeRoute,
) -> Result<Option<CustomMemoryResultEnvelope>> {
    let envelope: CustomMemoryResultEnvelope = serde_json::from_value(output.clone())
        .with_context(|| {
            format!(
                "MemoryRuntime `{}` returned malformed result",
                route.runtime
            )
        })?;
    validate_custom_memory_common_result(
        envelope.ok,
        &envelope.package,
        &envelope.package_version,
        &envelope.space,
        envelope.error.as_ref(),
        route,
    )?;
    if envelope.ok {
        Ok(None)
    } else {
        Ok(Some(envelope))
    }
}

fn custom_memory_runtime_failed_response(
    route: &MemorySpaceRuntimeRoute,
    message: impl Into<String>,
) -> Value {
    json!({
        "ok": false,
        "package": route.package,
        "package_version": route.package_version,
        "space": route.space,
        "runtime": route.runtime,
        "error": {
            "code": "memory_runtime_failed",
            "message": message.into()
        }
    })
}

fn malformed_custom_memory_runtime_response(
    route: &MemorySpaceRuntimeRoute,
    message: impl Into<String>,
) -> Value {
    json!({
        "ok": false,
        "package": route.package,
        "package_version": route.package_version,
        "space": route.space,
        "runtime": route.runtime,
        "error": {
            "code": "malformed_memory_runtime_response",
            "message": message.into()
        }
    })
}

pub fn custom_memory_action_error_from_output(output: &Value) -> Option<LocalMemoryActionError> {
    if output
        .get("ok")
        .and_then(Value::as_bool)
        .is_none_or(|ok| ok)
    {
        return None;
    }
    let error = output.get("error")?;
    let code = error.get("code").and_then(Value::as_str)?;
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or(code)
        .to_string();
    match code {
        "not_found" => Some(LocalMemoryActionError::not_found(message)),
        "capacity_exceeded" => Some(LocalMemoryActionError::capacity_exceeded(message)),
        "constraint_violation" | "record_type_mismatch" => {
            Some(LocalMemoryActionError::constraint_violation(message))
        }
        "contract_violation" => Some(LocalMemoryActionError::contract_violation(message)),
        _ => None,
    }
}

pub(crate) fn emit_memory_host_service_failure(
    lifecycle_events: Option<&crate::harness_runtime::service::ServiceLifecycleEmitter>,
    registry_id: &str,
    message: impl Into<String>,
) {
    let Some(events) = lifecycle_events else {
        return;
    };
    let message = message.into();
    events.emit(
        crate::harness_observability::HarnessEventType::ServiceUnhealthy,
        "memory",
        registry_id,
        "unhealthy",
        format!("Host MemoryRuntime request failed: {message}"),
    );
    events.emit(
        crate::harness_observability::HarnessEventType::ServiceFailed,
        "memory",
        registry_id,
        "failed",
        format!("Host MemoryRuntime request failed: {message}"),
    );
}

pub fn custom_memory_read_request_from_local(
    request: &LocalMemoryReadRequest<'_>,
) -> Result<CustomMemoryReadRequest> {
    validate_memory_scope(request.manifest, request.space, &request.scope)?;
    ensure_retrieval_mode(
        request.manifest,
        request.space,
        memory_retrieval_mode_from_local(request.mode),
    )?;
    Ok(CustomMemoryReadRequest {
        package: request.package.to_string(),
        package_version: request.package_version.to_string(),
        space: request.space.to_string(),
        scope: request.scope.clone(),
        mode: memory_retrieval_mode_from_local(request.mode),
        record_id: request.record_id.clone(),
        record_type: request.record_type.clone(),
        filter: request.filter.clone(),
        query: request.query.clone(),
        limit: request.limit,
        now: request.now.to_rfc3339(),
    })
}

pub fn custom_memory_write_request_from_local(
    request: &LocalMemoryWriteRequest<'_>,
) -> Result<CustomMemoryWriteRequest> {
    validate_memory_scope(request.manifest, request.space, &request.scope)?;
    let space = memory_space(request.manifest, request.space)?;
    if !space
        .record_types
        .contains(&request.record_type.to_string())
    {
        return Err(LocalMemoryActionError::contract_violation(format!(
            "record type `{}` is not permitted in Memory space `{}`",
            request.record_type, request.space
        ))
        .into());
    }
    if append_only_enabled(space)
        && matches!(
            request.operation,
            LocalMemoryWriteOperation::Update
                | LocalMemoryWriteOperation::Delete
                | LocalMemoryWriteOperation::Archive
        )
    {
        return Err(LocalMemoryActionError::constraint_violation(format!(
            "append_only Memory space `{}` rejects direct mutation",
            request.space
        ))
        .into());
    }
    if matches!(
        request.operation,
        LocalMemoryWriteOperation::Create | LocalMemoryWriteOperation::Upsert
    ) && request.record_id.is_some()
    {
        return Err(LocalMemoryActionError::constraint_violation(
            "Memory create/upsert cannot assign an authoritative record id",
        )
        .into());
    }

    let content = match request.operation {
        LocalMemoryWriteOperation::Create
        | LocalMemoryWriteOperation::Upsert
        | LocalMemoryWriteOperation::Update => {
            let proposed = request.content.as_ref().ok_or_else(|| {
                LocalMemoryActionError::constraint_violation(
                    "Memory create/update requires record content",
                )
            })?;
            Some(
                durable_memory_content_projection_for_record(
                    request.contracts,
                    request.space,
                    request.record_type,
                    proposed,
                )
                .map_err(|err| {
                    if has_local_memory_schema_error(&err) {
                        err
                    } else {
                        err.context("projecting durable Memory content")
                    }
                })?,
            )
        }
        LocalMemoryWriteOperation::Delete | LocalMemoryWriteOperation::Archive => None,
    };
    let schema_version = request
        .manifest
        .memory
        .record_types
        .get(request.record_type)
        .ok_or_else(|| {
            LocalMemoryActionError::contract_violation(format!(
                "unknown Memory record type `{}`",
                request.record_type
            ))
        })?
        .version
        .clone();
    Ok(CustomMemoryWriteRequest {
        package: request.package.to_string(),
        package_version: request.package_version.to_string(),
        space: request.space.to_string(),
        space_model: space.model.clone(),
        record_type: request.record_type.to_string(),
        schema_version,
        scope: request.scope.clone(),
        operation: request.operation.into(),
        record_id: request.record_id.clone(),
        content,
        provenance: match &request.provenance {
            Value::Null => json!({}),
            other => other.clone(),
        },
        now: request.now.to_rfc3339(),
    })
}

fn memory_retrieval_mode_from_local(mode: LocalMemoryReadMode) -> MemoryRetrievalMode {
    match mode {
        LocalMemoryReadMode::Key => MemoryRetrievalMode::Key,
        LocalMemoryReadMode::Filter => MemoryRetrievalMode::Filter,
        LocalMemoryReadMode::Chronological => MemoryRetrievalMode::Chronological,
        LocalMemoryReadMode::FullText => MemoryRetrievalMode::FullText,
        LocalMemoryReadMode::Semantic => MemoryRetrievalMode::Semantic,
    }
}

pub fn process_memory_runtime_service(
    workspace_root: &Path,
    registry_id: &str,
    entry: &HarnessImplementationEntry,
    spaces: &[crate::harness_runtime::model::MemorySpaceRuntimeSnapshot],
    lifecycle_events: Option<crate::harness_runtime::service::ServiceLifecycleEmitter>,
) -> Result<(ServiceRuntime, Value)> {
    let mut initialize_payload = Map::new();
    initialize_payload.insert("spaces".into(), json!(spaces));
    let runtime = ServiceRuntime::process(
        "memory",
        registry_id,
        entry,
        workspace_root,
        initialize_payload,
        lifecycle_events,
    )?;
    let capabilities = runtime
        .initialization_result()
        .cloned()
        .unwrap_or_else(|| json!({}));
    Ok((runtime, capabilities))
}

pub fn validate_memory_runtime_capabilities(
    value: &Value,
    registry_id: &str,
    spaces: &[crate::harness_runtime::model::MemorySpaceRuntimeSnapshot],
) -> Result<MemoryRuntimeCapabilityDescriptor> {
    if value
        .get("ready")
        .and_then(Value::as_bool)
        .is_some_and(|ready| !ready)
    {
        bail!("MemoryRuntime `{registry_id}` reported ready=false");
    }
    if let Some(role) = value.get("role").and_then(Value::as_str)
        && role != "memory"
    {
        bail!("MemoryRuntime `{registry_id}` reported role `{role}`, expected `memory`");
    }
    if let Some(protocol_version) = value.get("protocol_version").and_then(Value::as_u64)
        && protocol_version != crate::harness_runtime::service::AGENTPM_SERVICE_VERSION as u64
    {
        bail!(
            "MemoryRuntime `{registry_id}` reported protocol_version {protocol_version}, expected {}",
            crate::harness_runtime::service::AGENTPM_SERVICE_VERSION
        );
    }
    if let Some(advertised_registry_id) = value.get("registry_id").and_then(Value::as_str)
        && advertised_registry_id != registry_id
    {
        bail!("MemoryRuntime initialized as `{advertised_registry_id}`, expected `{registry_id}`");
    }
    let capabilities_value = value.get("capabilities").unwrap_or(value);
    let descriptor: MemoryRuntimeCapabilityDescriptor =
        serde_json::from_value(capabilities_value.clone())
            .with_context(|| format!("validating MemoryRuntime `{registry_id}` capabilities"))?;
    validate_memory_runtime_packages(value, registry_id, spaces)?;
    Ok(descriptor)
}

fn validate_memory_runtime_packages(
    value: &Value,
    registry_id: &str,
    spaces: &[crate::harness_runtime::model::MemorySpaceRuntimeSnapshot],
) -> Result<()> {
    let Some(advertised_packages) = value
        .get("packages")
        .or_else(|| value.get("attestations"))
        .or_else(|| value.get("capabilities").and_then(|v| v.get("packages")))
    else {
        if !spaces.is_empty() {
            bail!("MemoryRuntime `{registry_id}` must attest routed Memory packages");
        }
        return Ok(());
    };
    let advertised_packages = advertised_packages
        .as_array()
        .ok_or_else(|| anyhow!("MemoryRuntime packages capability must be an array"))?;
    let required = spaces
        .iter()
        .map(|space| (space.package.clone(), space.package_version.clone()))
        .collect::<std::collections::BTreeSet<_>>();
    for (package, version) in required {
        let matched = advertised_packages.iter().any(|value| {
            let Some(obj) = value.as_object() else {
                return false;
            };
            let name_matches = obj.get("package").and_then(Value::as_str) == Some(package.as_str())
                || obj.get("name").and_then(Value::as_str) == Some(package.as_str());
            let version_matches =
                obj.get("version").and_then(Value::as_str) == Some(version.as_str());
            let ready_matches = obj.get("ready").and_then(Value::as_bool) == Some(true);
            name_matches && version_matches && ready_matches
        });
        if !matched {
            bail!("MemoryRuntime `{registry_id}` does not attest {package}@{version} as ready");
        }
    }
    Ok(())
}
