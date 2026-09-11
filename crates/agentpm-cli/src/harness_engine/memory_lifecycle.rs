use super::effective_phase::{candidate_scope_matches_phase, memory_operation_identity};
use super::*;
use crate::harness_runtime::hook::BeforeMemoryOperationHook;
use crate::harness_runtime::memory::{
    LocalMemoryLifecycleCommitRequest, LocalMemoryLifecycleCommitResult,
    LocalMemoryLifecycleSourceSnapshot, LocalMemoryLifecycleTriggerPrecondition,
    LocalMemoryOperationStateRow, StoredMemoryRecord, ValidatedMemoryContracts,
    durable_memory_content_projection_for_record, generated_memory_content_schema,
};
use crate::harness_runtime::model::{ActionAlias, LogicalPrompt, PromptSection};
use crate::harness_runtime::{MemoryOperationRefRuntimeSnapshot, MemoryOperationRuntimeSnapshot};
use crate::manifest::{
    MemoryManifest, MemoryOperation, MemoryOperationRef, MemoryOperationTarget,
    MemorySourceHandling, MemoryTransformOutputMode, MemoryTrigger,
    parse_supported_positive_iso8601_duration,
};

#[derive(Debug, Clone)]
pub(super) struct MemoryChangeContext {
    pub package: String,
    pub space: String,
    pub capacity_relief: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MemoryCapacityReliefResult {
    Proceed,
    StillAtCapacity,
}

const MEMORY_OPERATION_FAILURE_BACKOFF_SECONDS: i64 = 30;

fn memory_operation_failure_backoff() -> chrono::Duration {
    chrono::Duration::seconds(MEMORY_OPERATION_FAILURE_BACKOFF_SECONDS)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MemoryOperationInvocationResult {
    pub package: String,
    pub package_version: String,
    pub operation: String,
    pub identity: String,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MemoryOperationControlError {
    pub code: &'static str,
    pub message: String,
}

impl MemoryOperationControlError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for MemoryOperationControlError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for MemoryOperationControlError {}

impl HarnessEngine {
    pub fn invoke_memory_operation(
        &mut self,
        session: &mut HarnessSession,
        package: &str,
        operation: &str,
        current_resolved_scope: BTreeMap<String, String>,
        model: &mut dyn ModelRuntime,
        hooks: &mut dyn HookRuntime,
    ) -> Result<MemoryOperationInvocationResult> {
        let active_run = self.active_run(session).map_err(|_| {
            MemoryOperationControlError::new(
                "memory_operation_no_active_run",
                "external Memory operation invocation requires an active Run",
            )
        })?;
        let run_id = active_run.run_id().to_string();
        let phase_id = active_run.current_phase_id.clone().ok_or_else(|| {
            MemoryOperationControlError::new(
                "memory_operation_no_active_phase",
                "external Memory operation invocation requires an active phase",
            )
        })?;
        let phase = self
            .phase(&phase_id)
            .ok_or_else(|| {
                MemoryOperationControlError::new(
                    "memory_operation_no_active_phase",
                    format!("Loop phase `{phase_id}` is not declared"),
                )
            })?
            .clone();
        self.phase_executions += 1;
        let mut state = PhaseExecutionState {
            phase_execution_id: format!("external-memory-operation-{}", self.phase_executions),
            phase_id: phase.id.clone(),
            transcript: Vec::new(),
            model_calls: 0,
            accepted_actions: 0,
            logical_tool_calls: 0,
            structured_repairs: 0,
            tool_call_repairs: 0,
        };
        let operation_snapshot = session
            .runtime_snapshot
            .memory_operations
            .iter()
            .find(|snapshot| {
                snapshot.package == package
                    && snapshot.operation == operation
                    && candidate_scope_matches_phase(&snapshot.binding_scope, &phase.id)
            })
            .cloned()
            .ok_or_else(|| {
                MemoryOperationControlError::new(
                    "memory_operation_not_participating",
                    format!(
                        "external Memory operation `{package}/operations/{operation}` is not participating in the current phase"
                    ),
                )
            })?;
        if operation_snapshot.state != "available" {
            return Err(MemoryOperationControlError::new(
                "memory_operation_backend_unready",
                format!(
                    "external Memory operation `{}` is not available: {}",
                    memory_operation_identity(
                        &operation_snapshot.package,
                        &operation_snapshot.operation
                    ),
                    operation_snapshot
                        .readiness_reason
                        .as_deref()
                        .unwrap_or("operation backend is unavailable")
                ),
            )
            .into());
        }
        let expected_scope = resolved_operation_scope(
            &operation_snapshot,
            &session.runtime_snapshot.runtime_scopes,
        )
        .map_err(|err| {
            MemoryOperationControlError::new("memory_operation_unresolved_scope", err.to_string())
        })?;
        if current_resolved_scope != expected_scope {
            return Err(MemoryOperationControlError::new(
                "memory_operation_scope_mismatch",
                "external Memory operation scope does not match the current trusted resolved scope",
            )
            .into());
        }
        let effective_phase =
            EffectivePhase::from_phase(&phase, &self.active_run(session)?.context.runtime);
        let operation_snapshot = effective_phase
            .active_memory_operations
            .iter()
            .find(|snapshot| snapshot.package == package && snapshot.operation == operation)
            .cloned()
            .ok_or_else(|| {
                MemoryOperationControlError::new(
                    "memory_operation_not_participating",
                    format!(
                        "external Memory operation `{package}/operations/{operation}` is not participating in the current phase"
                    ),
                )
            })?;
        let Some(root) = operation_snapshot.root.clone() else {
            return Err(MemoryOperationControlError::new(
                "memory_operation_backend_unready",
                "external Memory operation is missing its package root",
            )
            .into());
        };
        if operation_snapshot.runtime != "local" {
            return Err(MemoryOperationControlError::new(
                "memory_operation_backend_unready",
                "external Memory operations currently require the local MemoryRuntime",
            )
            .into());
        }
        let manifest_path = root.join("agent.json");
        let (manifest_value, _) = load_manifest_value(&manifest_path)?;
        let manifest = parse_memory_manifest(&manifest_value)?;
        let Some(operation_manifest) = manifest
            .memory
            .operations
            .get(&operation_snapshot.operation)
        else {
            return Err(MemoryOperationControlError::new(
                "memory_operation_not_declared",
                "external Memory operation is not declared by the Memory package",
            )
            .into());
        };
        if !matches!(
            operation_trigger(operation_manifest),
            MemoryTrigger::External
        ) {
            return Err(MemoryOperationControlError::new(
                "memory_operation_not_external",
                "only Memory operations with trigger.type `external` can be invoked externally",
            )
            .into());
        }
        session.emitter.emit(
            HarnessEventType::MemoryTriggerEvaluated,
            HarnessEventPayload::Lifecycle {
                message: "External Memory lifecycle trigger evaluated.".into(),
                fields: BTreeMap::from([
                    ("package".into(), json!(operation_snapshot.package.clone())),
                    (
                        "package_version".into(),
                        json!(operation_snapshot.package_version.clone()),
                    ),
                    (
                        "operation".into(),
                        json!(operation_snapshot.operation.clone()),
                    ),
                    ("trigger".into(), operation_snapshot.trigger.clone()),
                    ("eligible".into(), json!(true)),
                    ("external_invocation".into(), json!(true)),
                ]),
            },
            HarnessEventBuilder {
                run_id: Some(run_id.clone()),
                phase_execution_id: Some(state.phase_execution_id.clone()),
                ..HarnessEventBuilder::default()
            },
        )?;
        let count = match self.execute_eligible_memory_lifecycle_operation(
            session,
            &effective_phase,
            &mut state,
            &phase,
            model,
            hooks,
            &manifest,
            &operation_snapshot,
            operation_manifest,
            &expected_scope,
            false,
            Utc::now(),
        ) {
            Ok(count) => count,
            Err(err) => {
                let failure_state = failed_operation_state(
                    session,
                    &manifest,
                    &operation_snapshot,
                    operation_manifest,
                    &expected_scope,
                    "operation_failed",
                    &err.to_string(),
                    Utc::now(),
                )?;
                session
                    .local_memory_runtime()?
                    .store_operation_state(&failure_state)?;
                self.emit_memory_lifecycle_failure(
                    session,
                    &run_id,
                    &state.phase_execution_id,
                    &operation_snapshot,
                    "operation_failed",
                    &err.to_string(),
                )?;
                return Err(err);
            }
        };
        self.active_run_mut(session)?
            .operation_summaries
            .push(OperationReportSummary {
                operation_kind: "memory_operation".into(),
                identity: memory_operation_identity(
                    &operation_snapshot.package,
                    &operation_snapshot.operation,
                ),
                status: "completed".into(),
                count,
            });
        session.emitter.emit(
            HarnessEventType::MemoryOperationCompleted,
            HarnessEventPayload::Lifecycle {
                message: "Memory lifecycle operation completed.".into(),
                fields: {
                    let mut fields = memory_operation_event_fields(&operation_snapshot);
                    fields.insert("count".into(), json!(count));
                    fields.insert("external_invocation".into(), json!(true));
                    fields
                },
            },
            HarnessEventBuilder {
                run_id: Some(run_id),
                phase_execution_id: Some(state.phase_execution_id.clone()),
                ..HarnessEventBuilder::default()
            },
        )?;
        Ok(MemoryOperationInvocationResult {
            package: operation_snapshot.package.clone(),
            package_version: operation_snapshot.package_version.clone(),
            operation: operation_snapshot.operation.clone(),
            identity: memory_operation_identity(
                &operation_snapshot.package,
                &operation_snapshot.operation,
            ),
            count,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn relieve_memory_capacity_before_write(
        &self,
        session: &mut HarnessSession,
        effective_phase: &EffectivePhase,
        state: &mut PhaseExecutionState,
        phase: &LoopPhase,
        model: &mut dyn ModelRuntime,
        hooks: &mut dyn HookRuntime,
        action: &SemanticAction,
    ) -> Result<MemoryCapacityReliefResult> {
        let SemanticAction::MemoryWrite {
            package,
            space,
            operation,
            record_id,
            ..
        } = action
        else {
            return Ok(MemoryCapacityReliefResult::Proceed);
        };
        if !matches!(
            operation,
            MemoryWriteOperation::Create | MemoryWriteOperation::Upsert
        ) || record_id.is_some()
        {
            return Ok(MemoryCapacityReliefResult::Proceed);
        }
        let Some(memory) = effective_phase
            .active_memory
            .iter()
            .find(|memory| memory.package == *package && memory.space == *space)
            .cloned()
        else {
            return Ok(MemoryCapacityReliefResult::Proceed);
        };
        if memory.runtime != "local" {
            return Ok(MemoryCapacityReliefResult::Proceed);
        }
        let Some(root) = memory.root.clone() else {
            return Ok(MemoryCapacityReliefResult::Proceed);
        };
        let (manifest_value, _) = load_manifest_value(&root.join("agent.json"))?;
        let manifest = parse_memory_manifest(&manifest_value)?;
        let Some(space_manifest) = manifest.memory.spaces.get(space) else {
            return Ok(MemoryCapacityReliefResult::Proceed);
        };
        let Some(capacity) = &space_manifest.capacity else {
            return Ok(MemoryCapacityReliefResult::Proceed);
        };
        let scope = super::memory_actions::resolved_memory_scope(
            &memory,
            &session.runtime_snapshot.runtime_scopes,
        )?;
        let active_count = session.local_memory_runtime()?.active_record_count(
            &memory.package,
            &memory.package_version,
            &memory.space,
            &scope,
            None,
        )?;
        if active_count < capacity.max_records {
            return Ok(MemoryCapacityReliefResult::Proceed);
        }
        let capacity_operations = effective_phase
            .active_memory_operations
            .iter()
            .filter(|operation| operation.package == *package)
            .filter(|operation| {
                operation.trigger.get("type").and_then(Value::as_str) == Some("capacity")
                    && operation.trigger.get("space").and_then(Value::as_str) == Some(space)
            })
            .cloned()
            .collect::<Vec<_>>();
        if capacity_operations.is_empty() {
            return Ok(MemoryCapacityReliefResult::Proceed);
        }
        for operation in capacity_operations {
            self.evaluate_memory_lifecycle_operation(
                session,
                effective_phase,
                state,
                phase,
                model,
                hooks,
                &operation,
                Some(&MemoryChangeContext {
                    package: package.clone(),
                    space: space.clone(),
                    capacity_relief: true,
                }),
            )?;
            let remaining = session.local_memory_runtime()?.active_record_count(
                &memory.package,
                &memory.package_version,
                &memory.space,
                &scope,
                None,
            )?;
            if remaining < capacity.max_records {
                return Ok(MemoryCapacityReliefResult::Proceed);
            }
        }
        Ok(MemoryCapacityReliefResult::StillAtCapacity)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn evaluate_memory_lifecycle_after_write(
        &self,
        session: &mut HarnessSession,
        effective_phase: &EffectivePhase,
        state: &mut PhaseExecutionState,
        phase: &LoopPhase,
        model: &mut dyn ModelRuntime,
        hooks: &mut dyn HookRuntime,
        changed: MemoryChangeContext,
    ) -> Result<()> {
        let operations = effective_phase
            .active_memory_operations
            .iter()
            .filter(|operation| operation.package == changed.package)
            .filter(|operation| {
                operation
                    .referenced_spaces
                    .iter()
                    .any(|space| space == &changed.space)
                    || memory_trigger_space_json(&operation.trigger) == Some(changed.space.as_str())
            })
            .cloned()
            .collect::<Vec<_>>();
        for operation in operations {
            self.evaluate_memory_lifecycle_operation(
                session,
                effective_phase,
                state,
                phase,
                model,
                hooks,
                &operation,
                Some(&changed),
            )?;
        }
        Ok(())
    }

    pub(super) fn evaluate_memory_lifecycle_intervals_at_phase_start(
        &self,
        session: &mut HarnessSession,
        effective_phase: &EffectivePhase,
        state: &mut PhaseExecutionState,
        phase: &LoopPhase,
        model: &mut dyn ModelRuntime,
        hooks: &mut dyn HookRuntime,
    ) -> Result<()> {
        let operations = effective_phase
            .active_memory_operations
            .iter()
            .filter(|operation| {
                operation.trigger.get("type").and_then(Value::as_str) == Some("interval")
            })
            .cloned()
            .collect::<Vec<_>>();
        for operation in operations {
            self.evaluate_memory_lifecycle_operation(
                session,
                effective_phase,
                state,
                phase,
                model,
                hooks,
                &operation,
                None,
            )?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_memory_lifecycle_operation(
        &self,
        session: &mut HarnessSession,
        effective_phase: &EffectivePhase,
        state: &mut PhaseExecutionState,
        phase: &LoopPhase,
        model: &mut dyn ModelRuntime,
        hooks: &mut dyn HookRuntime,
        operation_snapshot: &MemoryOperationRuntimeSnapshot,
        changed: Option<&MemoryChangeContext>,
    ) -> Result<()> {
        let run_id = self.active_run(session)?.run_id().to_string();
        let phase_execution_id = state.phase_execution_id.clone();
        let Some(root) = operation_snapshot.root.clone() else {
            return Ok(());
        };
        if operation_snapshot.runtime != "local" {
            self.emit_memory_lifecycle_failure(
                session,
                &run_id,
                &phase_execution_id,
                operation_snapshot,
                "unsupported_runtime",
                "Memory lifecycle operations require the local MemoryRuntime",
            )?;
            return Ok(());
        }
        let manifest_path = root.join("agent.json");
        let (manifest_value, _) = load_manifest_value(&manifest_path)?;
        let manifest = parse_memory_manifest(&manifest_value)?;
        let Some(operation) = manifest
            .memory
            .operations
            .get(&operation_snapshot.operation)
        else {
            self.emit_memory_lifecycle_failure(
                session,
                &run_id,
                &phase_execution_id,
                operation_snapshot,
                "unknown_operation",
                "Memory lifecycle operation is not declared by the Memory package",
            )?;
            return Ok(());
        };
        if !memory_trigger_touches_change(operation_trigger(operation), changed) {
            return Ok(());
        }
        let scope =
            resolved_operation_scope(operation_snapshot, &session.runtime_snapshot.runtime_scopes)?;
        let now = Utc::now();
        let eligible = self.evaluate_lifecycle_trigger(
            session,
            &manifest,
            operation_snapshot,
            operation,
            &scope,
            now,
        )?;
        session.emitter.emit(
            HarnessEventType::MemoryTriggerEvaluated,
            HarnessEventPayload::Lifecycle {
                message: "Memory lifecycle trigger evaluated.".into(),
                fields: BTreeMap::from([
                    ("package".into(), json!(operation_snapshot.package.clone())),
                    (
                        "package_version".into(),
                        json!(operation_snapshot.package_version.clone()),
                    ),
                    (
                        "operation".into(),
                        json!(operation_snapshot.operation.clone()),
                    ),
                    ("trigger".into(), operation_snapshot.trigger.clone()),
                    ("eligible".into(), json!(eligible)),
                ]),
            },
            HarnessEventBuilder {
                run_id: Some(run_id.clone()),
                phase_execution_id: Some(phase_execution_id.clone()),
                ..HarnessEventBuilder::default()
            },
        )?;
        if !eligible {
            return Ok(());
        }

        match self.execute_eligible_memory_lifecycle_operation(
            session,
            effective_phase,
            state,
            phase,
            model,
            hooks,
            &manifest,
            operation_snapshot,
            operation,
            &scope,
            changed.is_some_and(|changed| changed.capacity_relief),
            now,
        ) {
            Ok(count) => {
                self.active_run_mut(session)?
                    .operation_summaries
                    .push(OperationReportSummary {
                        operation_kind: "memory_operation".into(),
                        identity: memory_operation_identity(
                            &operation_snapshot.package,
                            &operation_snapshot.operation,
                        ),
                        status: "completed".into(),
                        count,
                    });
                session.emitter.emit(
                    HarnessEventType::MemoryOperationCompleted,
                    HarnessEventPayload::Lifecycle {
                        message: "Memory lifecycle operation completed.".into(),
                        fields: {
                            let mut fields = memory_operation_event_fields(operation_snapshot);
                            fields.insert("count".into(), json!(count));
                            fields
                        },
                    },
                    HarnessEventBuilder {
                        run_id: Some(run_id),
                        phase_execution_id: Some(phase_execution_id),
                        ..HarnessEventBuilder::default()
                    },
                )?;
            }
            Err(err) => {
                let failure_state = failed_operation_state(
                    session,
                    &manifest,
                    operation_snapshot,
                    operation,
                    &scope,
                    "operation_failed",
                    &err.to_string(),
                    Utc::now(),
                )?;
                session
                    .local_memory_runtime()?
                    .store_operation_state(&failure_state)?;
                self.emit_memory_lifecycle_failure(
                    session,
                    &run_id,
                    &phase_execution_id,
                    operation_snapshot,
                    "operation_failed",
                    &err.to_string(),
                )?;
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_eligible_memory_lifecycle_operation(
        &self,
        session: &mut HarnessSession,
        effective_phase: &EffectivePhase,
        state: &mut PhaseExecutionState,
        phase: &LoopPhase,
        model: &mut dyn ModelRuntime,
        hooks: &mut dyn HookRuntime,
        manifest: &MemoryManifest,
        operation_snapshot: &MemoryOperationRuntimeSnapshot,
        operation: &MemoryOperation,
        scope: &BTreeMap<String, String>,
        capacity_relief: bool,
        now: DateTime<Utc>,
    ) -> Result<u64> {
        let run_id = self.active_run(session)?.run_id().to_string();
        let phase_execution_id = state.phase_execution_id.clone();
        let model_guidance = self.invoke_before_memory_operation_hook(
            session,
            hooks,
            phase,
            &phase_execution_id,
            &run_id,
            operation_snapshot,
            manifest,
            operation,
            scope,
        )?;
        session.emitter.emit(
            HarnessEventType::MemoryOperationStarted,
            HarnessEventPayload::Lifecycle {
                message: "Memory lifecycle operation started.".into(),
                fields: memory_operation_event_fields(operation_snapshot),
            },
            HarnessEventBuilder {
                run_id: Some(run_id),
                phase_execution_id: Some(phase_execution_id),
                ..HarnessEventBuilder::default()
            },
        )?;
        match operation {
            MemoryOperation::Delete {
                targets,
                cascade_derived_records,
                ..
            } => self.execute_delete_operation(
                session,
                state,
                manifest,
                operation_snapshot,
                operation,
                targets,
                *cascade_derived_records,
                scope,
                capacity_relief,
                now,
            ),
            MemoryOperation::Transform {
                inputs,
                output,
                source_handling,
                output_mode,
                preserve_provenance,
                ..
            } => self.execute_transform_operation(
                session,
                effective_phase,
                state,
                phase,
                model,
                manifest,
                operation_snapshot,
                operation,
                inputs,
                output,
                source_handling,
                output_mode,
                *preserve_provenance,
                scope,
                capacity_relief,
                model_guidance.as_deref(),
                now,
            ),
            MemoryOperation::Consolidate {
                inputs,
                output,
                source_handling,
                preserve_provenance,
                ..
            } => self.execute_consolidate_operation(
                session,
                effective_phase,
                state,
                phase,
                model,
                manifest,
                operation_snapshot,
                operation,
                inputs,
                output,
                source_handling,
                *preserve_provenance,
                scope,
                capacity_relief,
                model_guidance.as_deref(),
                now,
            ),
        }
    }

    fn evaluate_lifecycle_trigger(
        &self,
        session: &mut HarnessSession,
        manifest: &MemoryManifest,
        operation_snapshot: &MemoryOperationRuntimeSnapshot,
        operation: &MemoryOperation,
        operation_scope: &BTreeMap<String, String>,
        now: DateTime<Utc>,
    ) -> Result<bool> {
        let trigger = operation_trigger(operation);
        let existing = session.local_memory_runtime()?.load_operation_state(
            &operation_snapshot.package,
            &operation_snapshot.package_version,
            &operation_snapshot.operation,
            operation_scope,
        )?;
        match trigger {
            MemoryTrigger::External => Ok(false),
            MemoryTrigger::RecordCount { space, threshold } => {
                let scope = space_scope(manifest, space, operation_scope)?;
                let count = session.local_memory_runtime()?.active_record_count(
                    &operation_snapshot.package,
                    &operation_snapshot.package_version,
                    space,
                    &scope,
                    None,
                )?;
                let failure_cooldown_until = existing
                    .as_ref()
                    .filter(|state| state.last_failed_at.is_some() || state.last_failure.is_some())
                    .and_then(|state| state.next_eligible_at.as_deref())
                    .and_then(parse_optional_time);
                let was_below = existing
                    .as_ref()
                    .and_then(|state| state.last_observed_value)
                    .is_none_or(|value| value < *threshold as i64);
                let armed = existing.as_ref().is_none_or(|state| state.armed);
                let next_state = LocalMemoryOperationStateRow {
                    package: operation_snapshot.package.clone(),
                    package_version: operation_snapshot.package_version.clone(),
                    operation: operation_snapshot.operation.clone(),
                    scope: operation_scope.clone(),
                    trigger_type: "record_count".into(),
                    armed: count < *threshold || (armed && was_below),
                    baseline_at: existing
                        .as_ref()
                        .and_then(|state| state.baseline_at.as_deref())
                        .and_then(parse_optional_time)
                        .or(Some(now)),
                    last_completed_at: existing
                        .as_ref()
                        .and_then(|state| state.last_completed_at.as_deref())
                        .and_then(parse_optional_time),
                    last_failed_at: existing
                        .as_ref()
                        .and_then(|state| state.last_failed_at.as_deref())
                        .and_then(parse_optional_time),
                    next_eligible_at: if count < *threshold {
                        None
                    } else {
                        failure_cooldown_until
                    },
                    last_observed_value: Some(count as i64),
                    last_failure: existing
                        .as_ref()
                        .and_then(|state| state.last_failure.clone()),
                    watermark: existing.as_ref().and_then(|state| state.watermark.clone()),
                    updated_at: now,
                };
                let retry_after_failure = failure_cooldown_until.is_some_and(|next| now >= next);
                let cooling_down = failure_cooldown_until.is_some_and(|next| now < next);
                let eligible = armed && count >= *threshold && (was_below || retry_after_failure);
                if cooling_down {
                    let mut cooldown_state = next_state;
                    cooldown_state.armed = armed;
                    session
                        .local_memory_runtime()?
                        .store_operation_state(&cooldown_state)?;
                    return Ok(false);
                }
                if !eligible {
                    session
                        .local_memory_runtime()?
                        .store_operation_state(&next_state)?;
                }
                Ok(eligible)
            }
            MemoryTrigger::Capacity { space } => {
                let scope = space_scope(manifest, space, operation_scope)?;
                let Some(capacity) = manifest
                    .memory
                    .spaces
                    .get(space)
                    .and_then(|space| space.capacity.as_ref())
                else {
                    return Ok(false);
                };
                let count = session.local_memory_runtime()?.active_record_count(
                    &operation_snapshot.package,
                    &operation_snapshot.package_version,
                    space,
                    &scope,
                    None,
                )?;
                let max_records = capacity.max_records as i64;
                let failure_cooldown_until = existing
                    .as_ref()
                    .filter(|state| state.last_failed_at.is_some() || state.last_failure.is_some())
                    .and_then(|state| state.next_eligible_at.as_deref())
                    .and_then(parse_optional_time);
                let armed = existing.as_ref().is_none_or(|state| state.armed);
                let next_state = LocalMemoryOperationStateRow {
                    package: operation_snapshot.package.clone(),
                    package_version: operation_snapshot.package_version.clone(),
                    operation: operation_snapshot.operation.clone(),
                    scope: operation_scope.clone(),
                    trigger_type: "capacity".into(),
                    armed: count < capacity.max_records || armed,
                    baseline_at: existing
                        .as_ref()
                        .and_then(|state| state.baseline_at.as_deref())
                        .and_then(parse_optional_time)
                        .or(Some(now)),
                    last_completed_at: existing
                        .as_ref()
                        .and_then(|state| state.last_completed_at.as_deref())
                        .and_then(parse_optional_time),
                    last_failed_at: existing
                        .as_ref()
                        .and_then(|state| state.last_failed_at.as_deref())
                        .and_then(parse_optional_time),
                    next_eligible_at: if count < capacity.max_records {
                        None
                    } else {
                        failure_cooldown_until
                    },
                    last_observed_value: Some(count as i64),
                    last_failure: existing
                        .as_ref()
                        .and_then(|state| state.last_failure.clone()),
                    watermark: existing.as_ref().and_then(|state| state.watermark.clone()),
                    updated_at: now,
                };
                let cooling_down = failure_cooldown_until.is_some_and(|next| now < next);
                let eligible = armed && count as i64 >= max_records && !cooling_down;
                if cooling_down {
                    session
                        .local_memory_runtime()?
                        .store_operation_state(&next_state)?;
                    return Ok(false);
                }
                if !eligible {
                    session
                        .local_memory_runtime()?
                        .store_operation_state(&next_state)?;
                }
                Ok(eligible)
            }
            MemoryTrigger::Interval { every } => {
                let duration = parse_lifecycle_duration(every)?;
                if existing.is_none()
                    && !interval_operation_has_relevant_state(
                        session,
                        manifest,
                        operation_snapshot,
                        operation,
                        operation_scope,
                    )?
                {
                    return Ok(false);
                }
                let next = existing
                    .as_ref()
                    .and_then(|state| state.next_eligible_at.as_deref())
                    .and_then(parse_optional_time)
                    .unwrap_or(now + duration);
                let eligible = existing.is_some() && now >= next;
                let next_state = LocalMemoryOperationStateRow {
                    package: operation_snapshot.package.clone(),
                    package_version: operation_snapshot.package_version.clone(),
                    operation: operation_snapshot.operation.clone(),
                    scope: operation_scope.clone(),
                    trigger_type: "interval".into(),
                    armed: true,
                    baseline_at: existing
                        .as_ref()
                        .and_then(|state| state.baseline_at.as_deref())
                        .and_then(parse_optional_time)
                        .or(Some(now)),
                    last_completed_at: existing
                        .as_ref()
                        .and_then(|state| state.last_completed_at.as_deref())
                        .and_then(parse_optional_time),
                    last_failed_at: existing
                        .as_ref()
                        .and_then(|state| state.last_failed_at.as_deref())
                        .and_then(parse_optional_time),
                    next_eligible_at: Some(if eligible { now + duration } else { next }),
                    last_observed_value: existing
                        .as_ref()
                        .and_then(|state| state.last_observed_value),
                    last_failure: existing
                        .as_ref()
                        .and_then(|state| state.last_failure.clone()),
                    watermark: existing.as_ref().and_then(|state| state.watermark.clone()),
                    updated_at: now,
                };
                if !eligible {
                    session
                        .local_memory_runtime()?
                        .store_operation_state(&next_state)?;
                }
                Ok(eligible)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn invoke_before_memory_operation_hook(
        &self,
        session: &mut HarnessSession,
        hooks: &mut dyn HookRuntime,
        phase: &LoopPhase,
        phase_execution_id: &str,
        run_id: &str,
        operation: &MemoryOperationRuntimeSnapshot,
        manifest: &MemoryManifest,
        operation_manifest: &MemoryOperation,
        scope: &BTreeMap<String, String>,
    ) -> Result<Option<String>> {
        let hook = HarnessHookId::BeforeMemoryOperation;
        let binding_count = hooks.binding_count(&hook);
        if binding_count > 0 {
            self.emit_hook_started(
                session,
                HookEventContext {
                    run_id,
                    phase_id: &phase.id,
                    phase_execution_id,
                    hook: &hook,
                    binding_count,
                },
                memory_operation_event_fields(operation),
            )?;
        }
        let source_summary = memory_operation_safe_source_summary(
            session,
            manifest,
            operation,
            operation_manifest,
            scope,
        )?;
        let decision = hooks.before_memory_operation(BeforeMemoryOperationHook {
            phase_id: phase.id.clone(),
            package: operation.package.clone(),
            operation: operation.operation.clone(),
            scope: json!(scope),
            source_summary,
        });
        self.emit_nonfatal_hook_failures(session, run_id, &phase.id, phase_execution_id, hooks)?;
        match decision {
            Ok(decision) => {
                if binding_count > 0 {
                    let mut fields = memory_operation_event_fields(operation);
                    fields.insert("patched".into(), json!(decision.model_guidance.is_some()));
                    self.emit_hook_completed(
                        session,
                        HookEventContext {
                            run_id,
                            phase_id: &phase.id,
                            phase_execution_id,
                            hook: &hook,
                            binding_count,
                        },
                        fields,
                    )?;
                }
                Ok(decision.model_guidance)
            }
            Err(err) => {
                session.emitter.emit(
                    if err.is_rejection() {
                        HarnessEventType::HookRejected
                    } else {
                        HarnessEventType::HookFailed
                    },
                    HarnessEventPayload::Lifecycle {
                        message: err.message.clone(),
                        fields: hook_event_fields(
                            &hook,
                            &phase.id,
                            binding_count,
                            memory_operation_event_fields(operation),
                        ),
                    },
                    HarnessEventBuilder {
                        run_id: Some(run_id.to_string()),
                        phase_execution_id: Some(phase_execution_id.to_string()),
                        ..HarnessEventBuilder::default()
                    },
                )?;
                Err(anyhow!(
                    "before_memory_operation hook failed: {}",
                    err.message
                ))
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_transform_operation(
        &self,
        session: &mut HarnessSession,
        effective_phase: &EffectivePhase,
        state: &mut PhaseExecutionState,
        phase: &LoopPhase,
        model: &mut dyn ModelRuntime,
        manifest: &MemoryManifest,
        operation_snapshot: &MemoryOperationRuntimeSnapshot,
        operation: &MemoryOperation,
        inputs: &[MemoryOperationRef],
        output: &MemoryOperationRef,
        source_handling: &MemorySourceHandling,
        output_mode: &MemoryTransformOutputMode,
        preserve_provenance: bool,
        operation_scope: &BTreeMap<String, String>,
        capacity_relief: bool,
        model_guidance: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<u64> {
        let Some(input) = inputs.first() else {
            return Ok(0);
        };
        let root = operation_snapshot
            .root
            .as_ref()
            .context("missing Memory root")?;
        let contracts = session.memory_contract_cache.validate_and_load(root)?;
        let sources = self.read_lifecycle_sources(
            session,
            manifest,
            operation_snapshot,
            input,
            operation_scope,
            now,
        )?;
        let mut count = 0;
        for source in sources {
            let content = self.generate_lifecycle_content(
                session,
                effective_phase,
                state,
                phase,
                model,
                operation_snapshot,
                output,
                &contracts,
                std::slice::from_ref(&source),
                model_guidance.map(str::to_string),
            )?;
            let record_id = if matches!(output_mode, MemoryTransformOutputMode::ReplaceInput) {
                Some(source.id.clone())
            } else {
                None
            };
            let write_operation = if matches!(output_mode, MemoryTransformOutputMode::ReplaceInput)
            {
                LocalMemoryWriteOperation::Update
            } else {
                LocalMemoryWriteOperation::Create
            };
            let output_scope = space_scope(manifest, &output.space, operation_scope)?;
            let source_snapshot = session
                .local_memory_runtime()?
                .source_snapshot_for_lifecycle(&source)?;
            let source_batch = vec![source.clone()];
            let source_mutations = source_handling_mutations(
                &operation_snapshot.package,
                &operation_snapshot.package_version,
                manifest,
                &contracts,
                source_handling,
                &source_batch,
                operation_scope,
                now,
            )?;
            let output_writes = vec![LocalMemoryWriteRequest {
                package: &operation_snapshot.package,
                package_version: &operation_snapshot.package_version,
                manifest,
                contracts: &contracts,
                space: &output.space,
                record_type: &output.record_type,
                scope: output_scope,
                operation: write_operation,
                record_id,
                content: Some(content),
                provenance: lifecycle_provenance(
                    operation_snapshot,
                    &source_batch,
                    preserve_provenance,
                ),
                now,
            }];
            let operation_state = completed_operation_state(
                session,
                manifest,
                operation_snapshot,
                operation,
                operation_scope,
                &output_writes,
                &source_mutations,
                capacity_relief,
                now,
                Some(json!({
                    "source_ids": [source.id.clone()],
                    "preserve_provenance": preserve_provenance,
                })),
            )?;
            let commit = session.local_memory_runtime()?.commit_lifecycle_operation(
                LocalMemoryLifecycleCommitRequest {
                    trigger_precondition: lifecycle_trigger_precondition(
                        manifest,
                        operation_snapshot,
                        operation,
                        operation_scope,
                    )?,
                    expected_sources: vec![source_snapshot],
                    output_writes,
                    source_mutations,
                    operation_state,
                },
            )?;
            self.emit_lifecycle_commit_events(session, state, operation_snapshot, &commit)?;
            count += 1;
        }
        Ok(count)
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_consolidate_operation(
        &self,
        session: &mut HarnessSession,
        effective_phase: &EffectivePhase,
        state: &mut PhaseExecutionState,
        phase: &LoopPhase,
        model: &mut dyn ModelRuntime,
        manifest: &MemoryManifest,
        operation_snapshot: &MemoryOperationRuntimeSnapshot,
        operation: &MemoryOperation,
        inputs: &[MemoryOperationRef],
        output: &MemoryOperationRef,
        source_handling: &MemorySourceHandling,
        preserve_provenance: bool,
        operation_scope: &BTreeMap<String, String>,
        capacity_relief: bool,
        model_guidance: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<u64> {
        let mut sources = Vec::new();
        let root = operation_snapshot
            .root
            .as_ref()
            .context("missing Memory root")?;
        let contracts = session.memory_contract_cache.validate_and_load(root)?;
        for input in inputs {
            sources.extend(self.read_lifecycle_sources(
                session,
                manifest,
                operation_snapshot,
                input,
                operation_scope,
                now,
            )?);
        }
        sort_lifecycle_sources(&mut sources);
        if sources.is_empty() {
            return Ok(0);
        }
        let content = self.generate_lifecycle_content(
            session,
            effective_phase,
            state,
            phase,
            model,
            operation_snapshot,
            output,
            &contracts,
            &sources,
            model_guidance.map(str::to_string),
        )?;
        let expected_sources = lifecycle_source_snapshots(session, &sources)?;
        let source_mutations = source_handling_mutations(
            &operation_snapshot.package,
            &operation_snapshot.package_version,
            manifest,
            &contracts,
            source_handling,
            &sources,
            operation_scope,
            now,
        )?;
        let output_scope = space_scope(manifest, &output.space, operation_scope)?;
        let output_writes = vec![LocalMemoryWriteRequest {
            package: &operation_snapshot.package,
            package_version: &operation_snapshot.package_version,
            manifest,
            contracts: &contracts,
            space: &output.space,
            record_type: &output.record_type,
            scope: output_scope,
            operation: LocalMemoryWriteOperation::Create,
            record_id: None,
            content: Some(content),
            provenance: lifecycle_provenance(operation_snapshot, &sources, preserve_provenance),
            now,
        }];
        let operation_state = completed_operation_state(
            session,
            manifest,
            operation_snapshot,
            operation,
            operation_scope,
            &output_writes,
            &source_mutations,
            capacity_relief,
            now,
            Some(json!({
                "source_ids": sources.iter().map(|source| source.id.clone()).collect::<Vec<_>>(),
                "preserve_provenance": preserve_provenance,
            })),
        )?;
        let commit = session.local_memory_runtime()?.commit_lifecycle_operation(
            LocalMemoryLifecycleCommitRequest {
                trigger_precondition: lifecycle_trigger_precondition(
                    manifest,
                    operation_snapshot,
                    operation,
                    operation_scope,
                )?,
                expected_sources,
                output_writes,
                source_mutations,
                operation_state,
            },
        )?;
        self.emit_lifecycle_commit_events(session, state, operation_snapshot, &commit)?;
        Ok(1)
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_delete_operation(
        &self,
        session: &mut HarnessSession,
        state: &PhaseExecutionState,
        manifest: &MemoryManifest,
        operation_snapshot: &MemoryOperationRuntimeSnapshot,
        operation: &MemoryOperation,
        targets: &[MemoryOperationTarget],
        cascade_derived_records: bool,
        operation_scope: &BTreeMap<String, String>,
        capacity_relief: bool,
        now: DateTime<Utc>,
    ) -> Result<u64> {
        let root = operation_snapshot
            .root
            .as_ref()
            .context("missing Memory root")?;
        let contracts = session.memory_contract_cache.validate_and_load(root)?;
        let mut sources = Vec::new();
        for target in targets {
            let scope = space_scope(manifest, &target.space, operation_scope)?;
            sources.extend(
                session
                    .local_memory_runtime()?
                    .read_active_records_for_lifecycle(LocalMemoryReadRequest {
                        package: &operation_snapshot.package,
                        package_version: &operation_snapshot.package_version,
                        manifest,
                        space: &target.space,
                        scope: scope.clone(),
                        mode: LocalMemoryReadMode::Chronological,
                        record_id: None,
                        record_type: None,
                        filter: BTreeMap::new(),
                        query: None,
                        limit: None,
                        now,
                    })?,
            );
        }
        sort_lifecycle_sources(&mut sources);
        sources.dedup_by(|left, right| {
            left.package == right.package
                && left.package_version == right.package_version
                && left.space == right.space
                && left.scope_hash == right.scope_hash
                && left.id == right.id
        });
        if cascade_derived_records {
            let mut target_ids = sources
                .iter()
                .map(|source| source.id.clone())
                .collect::<std::collections::BTreeSet<_>>();
            loop {
                let mut added = Vec::new();
                for space_name in manifest.memory.spaces.keys() {
                    let Ok(scope) = space_scope(manifest, space_name, operation_scope) else {
                        continue;
                    };
                    let records = session
                        .local_memory_runtime()?
                        .read_active_records_for_lifecycle(LocalMemoryReadRequest {
                            package: &operation_snapshot.package,
                            package_version: &operation_snapshot.package_version,
                            manifest,
                            space: space_name,
                            scope,
                            mode: LocalMemoryReadMode::Chronological,
                            record_id: None,
                            record_type: None,
                            filter: BTreeMap::new(),
                            query: None,
                            limit: None,
                            now,
                        })?;
                    for record in records {
                        if target_ids.contains(&record.id)
                            || !record_is_derived_from_any(&record, &target_ids)
                        {
                            continue;
                        }
                        target_ids.insert(record.id.clone());
                        added.push(record);
                    }
                }
                if added.is_empty() {
                    break;
                }
                sources.extend(added);
                sort_lifecycle_sources(&mut sources);
            }
        }
        let count = sources.len() as u64;
        let expected_sources = lifecycle_source_snapshots(session, &sources)?;
        let mut source_mutations = Vec::new();
        for source in &sources {
            source_mutations.push(LocalMemoryWriteRequest {
                package: &operation_snapshot.package,
                package_version: &operation_snapshot.package_version,
                manifest,
                contracts: &contracts,
                space: &source.space,
                record_type: &source.record_type,
                scope: serde_json::from_str(&source.scope_json)
                    .context("parsing Memory source scope for delete operation")?,
                operation: LocalMemoryWriteOperation::Delete,
                record_id: Some(source.id.clone()),
                content: None,
                provenance: json!({}),
                now,
            });
        }
        let operation_state = completed_operation_state(
            session,
            manifest,
            operation_snapshot,
            operation,
            operation_scope,
            &[],
            &source_mutations,
            capacity_relief,
            now,
            Some(json!({
                "source_ids": sources.iter().map(|source| source.id.clone()).collect::<Vec<_>>(),
            })),
        )?;
        let commit = session.local_memory_runtime()?.commit_lifecycle_operation(
            LocalMemoryLifecycleCommitRequest {
                trigger_precondition: lifecycle_trigger_precondition(
                    manifest,
                    operation_snapshot,
                    operation,
                    operation_scope,
                )?,
                expected_sources,
                output_writes: Vec::new(),
                source_mutations,
                operation_state,
            },
        )?;
        self.emit_lifecycle_commit_events(session, state, operation_snapshot, &commit)?;
        Ok(count)
    }

    fn read_lifecycle_sources(
        &self,
        session: &mut HarnessSession,
        manifest: &MemoryManifest,
        operation: &MemoryOperationRuntimeSnapshot,
        input: &MemoryOperationRef,
        operation_scope: &BTreeMap<String, String>,
        now: DateTime<Utc>,
    ) -> Result<Vec<StoredMemoryRecord>> {
        let scope = space_scope(manifest, &input.space, operation_scope)?;
        let mut records = session
            .local_memory_runtime()?
            .read_active_records_for_lifecycle(LocalMemoryReadRequest {
                package: &operation.package,
                package_version: &operation.package_version,
                manifest,
                space: &input.space,
                scope,
                mode: LocalMemoryReadMode::Chronological,
                record_id: None,
                record_type: Some(input.record_type.clone()),
                filter: BTreeMap::new(),
                query: None,
                limit: None,
                now,
            })?;
        sort_lifecycle_sources(&mut records);
        Ok(records)
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_lifecycle_content(
        &self,
        session: &mut HarnessSession,
        effective_phase: &EffectivePhase,
        state: &mut PhaseExecutionState,
        phase: &LoopPhase,
        model: &mut dyn ModelRuntime,
        operation: &MemoryOperationRuntimeSnapshot,
        output: &MemoryOperationRef,
        contracts: &ValidatedMemoryContracts,
        sources: &[StoredMemoryRecord],
        model_guidance: Option<String>,
    ) -> Result<Value> {
        let max_repairs = self.options.runtime_limits.max_memory_operation_repairs;
        let mut repair_feedback = None;
        let output_schema =
            generated_memory_content_schema(contracts, &output.space, &output.record_type)?;
        for attempt in 0..=max_repairs {
            if state.model_calls >= self.options.runtime_limits.max_model_calls_per_phase {
                bail!(
                    "Loop max_model_calls_per_phase was reached during Memory lifecycle operation"
                );
            }
            state.model_calls += 1;
            let prompt = lifecycle_logical_prompt(
                operation,
                output,
                sources,
                &output_schema,
                model_guidance.clone(),
                repair_feedback.clone(),
            );
            let request = ModelRequest {
                runtime: self.active_run(session)?.context.runtime.clone(),
                model: session.runtime_snapshot.model.clone(),
                prompt,
                run_id: self.active_run(session)?.run_id().to_string(),
                phase_execution_id: state.phase_execution_id.clone(),
                phase_id: phase.id.clone(),
                phase_objective: phase.objective.clone(),
                run_input: self.active_run(session)?.context.input.clone(),
                prior_phase_results: self.active_run(session)?.phase_results.clone(),
                transcript: state.transcript.clone(),
                effective_phase: effective_phase.clone(),
                repair_feedback: repair_feedback.clone(),
            };
            session.emitter.emit(
                HarnessEventType::PromptPrepared,
                HarnessEventPayload::Lifecycle {
                    message: "Canonical model prompt prepared.".into(),
                    fields: BTreeMap::from([
                        ("phase_id".into(), json!(phase.id.clone())),
                        (
                            "memory_operation".into(),
                            json!(operation.operation.clone()),
                        ),
                        (
                            "operation_type".into(),
                            json!(operation.operation_type.clone()),
                        ),
                        ("sections".into(), json!(request.prompt.sections.len())),
                        ("prompt".into(), json!(request.prompt.render_text())),
                        (
                            "action_descriptors".into(),
                            json!(request.prompt.action_aliases.len()),
                        ),
                    ]),
                },
                HarnessEventBuilder {
                    run_id: Some(self.active_run(session)?.run_id().to_string()),
                    phase_execution_id: Some(state.phase_execution_id.clone()),
                    ..HarnessEventBuilder::default()
                },
            )?;
            if let Some(snapshot) = model.inspect_request(&request) {
                session.emitter.emit(
                    HarnessEventType::ModelRuntimeRequestPrepared,
                    HarnessEventPayload::Lifecycle {
                        message: "Model runtime request prepared.".into(),
                        fields: snapshot.into_trace_fields()?,
                    },
                    HarnessEventBuilder {
                        run_id: Some(self.active_run(session)?.run_id().to_string()),
                        phase_execution_id: Some(state.phase_execution_id.clone()),
                        ..HarnessEventBuilder::default()
                    },
                )?;
            }
            session.emitter.emit(
                HarnessEventType::ModelRequestStarted,
                HarnessEventPayload::Lifecycle {
                    message: "Model request started.".into(),
                    fields: BTreeMap::from([
                        ("phase_id".into(), json!(phase.id.clone())),
                        (
                            "memory_operation".into(),
                            json!(operation.operation.clone()),
                        ),
                    ]),
                },
                HarnessEventBuilder {
                    run_id: Some(self.active_run(session)?.run_id().to_string()),
                    phase_execution_id: Some(state.phase_execution_id.clone()),
                    ..HarnessEventBuilder::default()
                },
            )?;
            let turn = model
                .generate(request)
                .map_err(|err| anyhow!("Memory lifecycle model request failed: {}", err.message))?;
            self.merge_usage(session, &turn.usage);
            self.active_run_mut(session)?.usage.model_calls += 1;
            session.emitter.emit(
                HarnessEventType::ModelRequestCompleted,
                HarnessEventPayload::Lifecycle {
                    message: "Model request completed.".into(),
                    fields: BTreeMap::from([
                        ("finish_reason".into(), json!(turn.finish_reason)),
                        (
                            "memory_operation".into(),
                            json!(operation.operation.clone()),
                        ),
                    ]),
                },
                HarnessEventBuilder {
                    run_id: Some(self.active_run(session)?.run_id().to_string()),
                    phase_execution_id: Some(state.phase_execution_id.clone()),
                    ..HarnessEventBuilder::default()
                },
            )?;
            let validation_result = turn
                .assistant_content
                .as_deref()
                .ok_or_else(|| anyhow!("Memory lifecycle model output must be JSON record content"))
                .and_then(|content| {
                    serde_json::from_str::<Value>(content)
                        .context("parsing Memory lifecycle model output as JSON")
                })
                .and_then(|content| {
                    durable_memory_content_projection_for_record(
                        contracts,
                        &output.space,
                        &output.record_type,
                        &content,
                    )
                    .map(|_| content)
                });
            match validation_result {
                Ok(content) => return Ok(content),
                Err(err) if attempt < max_repairs => {
                    let feedback = err.to_string();
                    session.emitter.emit(
                        HarnessEventType::ModelRepairRequested,
                        HarnessEventPayload::Lifecycle {
                            message: feedback.clone(),
                            fields: BTreeMap::from([
                                ("repair_attempt".into(), json!(attempt + 1)),
                                ("repair_kind".into(), json!("memory_operation_output")),
                                (
                                    "memory_operation".into(),
                                    json!(operation.operation.clone()),
                                ),
                            ]),
                        },
                        HarnessEventBuilder {
                            run_id: Some(self.active_run(session)?.run_id().to_string()),
                            phase_execution_id: Some(state.phase_execution_id.clone()),
                            ..HarnessEventBuilder::default()
                        },
                    )?;
                    repair_feedback = Some(feedback);
                }
                Err(err) => {
                    bail!("Memory lifecycle model output validation failed: {err}");
                }
            }
        }
        unreachable!("bounded Memory lifecycle repair loop must return or fail")
    }

    fn emit_lifecycle_commit_events(
        &self,
        session: &mut HarnessSession,
        state: &PhaseExecutionState,
        operation: &MemoryOperationRuntimeSnapshot,
        commit: &LocalMemoryLifecycleCommitResult,
    ) -> Result<()> {
        let run_id = self.active_run(session)?.run_id().to_string();
        if !commit.source_record_ids.is_empty() {
            let mut fields = memory_operation_event_fields(operation);
            fields.insert("record_ids".into(), json!(commit.source_record_ids.clone()));
            session.emitter.emit(
                HarnessEventType::MemoryOperationSource,
                HarnessEventPayload::Lifecycle {
                    message: "Memory lifecycle operation source records mutated.".into(),
                    fields,
                },
                HarnessEventBuilder {
                    run_id: Some(run_id.clone()),
                    phase_execution_id: Some(state.phase_execution_id.clone()),
                    ..HarnessEventBuilder::default()
                },
            )?;
        }
        if !commit.output_record_ids.is_empty() {
            let mut fields = memory_operation_event_fields(operation);
            fields.insert("record_ids".into(), json!(commit.output_record_ids.clone()));
            session.emitter.emit(
                HarnessEventType::MemoryOperationOutput,
                HarnessEventPayload::Lifecycle {
                    message: "Memory lifecycle operation output records written.".into(),
                    fields,
                },
                HarnessEventBuilder {
                    run_id: Some(run_id),
                    phase_execution_id: Some(state.phase_execution_id.clone()),
                    ..HarnessEventBuilder::default()
                },
            )?;
        }
        Ok(())
    }

    fn emit_memory_lifecycle_failure(
        &self,
        session: &mut HarnessSession,
        run_id: &str,
        phase_execution_id: &str,
        operation: &MemoryOperationRuntimeSnapshot,
        code: &str,
        message: &str,
    ) -> Result<()> {
        self.active_run_mut(session)?
            .operation_summaries
            .push(OperationReportSummary {
                operation_kind: "memory_operation".into(),
                identity: memory_operation_identity(&operation.package, &operation.operation),
                status: "failed".into(),
                count: 1,
            });
        let mut fields = memory_operation_event_fields(operation);
        fields.insert("error".into(), json!({ "code": code, "message": message }));
        session.emitter.emit(
            HarnessEventType::MemoryOperationFailed,
            HarnessEventPayload::Lifecycle {
                message: "Memory lifecycle operation failed.".into(),
                fields,
            },
            HarnessEventBuilder {
                run_id: Some(run_id.to_string()),
                phase_execution_id: Some(phase_execution_id.to_string()),
                ..HarnessEventBuilder::default()
            },
        )?;
        Ok(())
    }
}

fn lifecycle_logical_prompt(
    operation: &MemoryOperationRuntimeSnapshot,
    output: &MemoryOperationRef,
    sources: &[StoredMemoryRecord],
    output_schema: &Value,
    model_guidance: Option<String>,
    repair_feedback: Option<String>,
) -> LogicalPrompt {
    let mut control = format!(
        "Harness authority: return exactly one JSON content object for Memory lifecycle operation `{}` matching the output content schema. Do not include record envelope, provenance, scope, id, timestamps, markdown, or semantic actions.",
        operation.operation
    );
    if let Some(model_guidance) = model_guidance {
        control.push_str(&format!(
            "\nAdditional operation-model guidance: {model_guidance}"
        ));
    }
    if let Some(repair_feedback) = repair_feedback {
        control.push_str(&format!(
            "\nRepair feedback from previous lifecycle output: {repair_feedback}"
        ));
    }
    LogicalPrompt {
        sections: vec![
            PromptSection {
                number: 1,
                title: "HARNESS CONTROL".into(),
                content: control,
            },
            PromptSection {
                number: 2,
                title: "MEMORY OPERATION".into(),
                content: format!(
                    "Type: {}\nDescription: {}\nOutput: {}/{}",
                    operation.operation_type,
                    operation.description,
                    output.space,
                    output.record_type
                ),
            },
            PromptSection {
                number: 3,
                title: "SOURCE RECORDS".into(),
                content: serde_json::to_string_pretty(&sources).unwrap_or_else(|_| "[]".into()),
            },
            PromptSection {
                number: 4,
                title: "OUTPUT CONTENT SCHEMA".into(),
                content: serde_json::to_string_pretty(output_schema)
                    .unwrap_or_else(|_| "{}".into()),
            },
        ],
        action_aliases: Vec::<ActionAlias>::new(),
        completion: CompletionContract {
            phase_id: String::new(),
            explicit_outcomes: Vec::new(),
            implicit_complete: true,
        },
        diagnostics: Vec::new(),
    }
}

fn memory_operation_event_fields(
    operation: &MemoryOperationRuntimeSnapshot,
) -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("package".into(), json!(operation.package.clone())),
        (
            "package_version".into(),
            json!(operation.package_version.clone()),
        ),
        ("operation".into(), json!(operation.operation.clone())),
        (
            "operation_type".into(),
            json!(operation.operation_type.clone()),
        ),
        (
            "identity".into(),
            json!(memory_operation_identity(
                &operation.package,
                &operation.operation
            )),
        ),
    ])
}

fn memory_operation_safe_source_summary(
    session: &mut HarnessSession,
    manifest: &MemoryManifest,
    operation: &MemoryOperationRuntimeSnapshot,
    operation_manifest: &MemoryOperation,
    operation_scope: &BTreeMap<String, String>,
) -> Result<Value> {
    let mut sources = Vec::new();
    for reference in operation_source_summary_refs(operation_manifest) {
        let scope = space_scope(manifest, &reference.space, operation_scope)?;
        let count = session.local_memory_runtime()?.active_record_count(
            &operation.package,
            &operation.package_version,
            &reference.space,
            &scope,
            reference.record_type.as_deref(),
        )?;
        sources.push(json!({
            "space": reference.space,
            "record_type": reference.record_type,
            "active_count": count,
        }));
    }
    Ok(json!({
        "operation": {
            "package": operation.package.clone(),
            "package_version": operation.package_version.clone(),
            "operation": operation.operation.clone(),
            "operation_type": operation.operation_type.clone(),
            "description": operation.description.clone(),
            "trigger": operation.trigger.clone(),
            "inputs": operation.inputs.clone(),
            "output": operation.output.clone(),
            "targets": operation.targets.clone(),
            "source_handling": operation.source_handling.clone(),
            "output_mode": operation.output_mode.clone(),
            "preserve_provenance": operation.preserve_provenance,
            "cascade_derived_records": operation.cascade_derived_records,
            "binding_scope": operation.binding_scope.clone(),
        },
        "referenced_spaces": operation.referenced_spaces.clone(),
        "sources": sources,
    }))
}

fn interval_operation_has_relevant_state(
    session: &mut HarnessSession,
    manifest: &MemoryManifest,
    operation: &MemoryOperationRuntimeSnapshot,
    operation_manifest: &MemoryOperation,
    operation_scope: &BTreeMap<String, String>,
) -> Result<bool> {
    for reference in operation_source_summary_refs(operation_manifest) {
        let scope = space_scope(manifest, &reference.space, operation_scope)?;
        let count = session.local_memory_runtime()?.active_record_count(
            &operation.package,
            &operation.package_version,
            &reference.space,
            &scope,
            reference.record_type.as_deref(),
        )?;
        if count > 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

fn operation_source_summary_refs(
    operation: &MemoryOperation,
) -> Vec<MemoryOperationRefRuntimeSnapshot> {
    match operation {
        MemoryOperation::Transform { inputs, .. } | MemoryOperation::Consolidate { inputs, .. } => {
            inputs
                .iter()
                .map(|input| MemoryOperationRefRuntimeSnapshot {
                    space: input.space.clone(),
                    record_type: Some(input.record_type.clone()),
                })
                .collect()
        }
        MemoryOperation::Delete { targets, .. } => targets
            .iter()
            .map(|target| MemoryOperationRefRuntimeSnapshot {
                space: target.space.clone(),
                record_type: None,
            })
            .collect(),
    }
}

fn resolved_operation_scope(
    operation: &MemoryOperationRuntimeSnapshot,
    runtime_scopes: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>> {
    let mut scope = BTreeMap::new();
    for key in &operation.scope_keys {
        let value = runtime_scopes.get(key).with_context(|| {
            format!(
                "Memory scope key `{key}` is unresolved for operation `{}`",
                operation.operation
            )
        })?;
        if value.is_empty() {
            bail!(
                "Memory scope key `{key}` has an empty value for operation `{}`",
                operation.operation
            );
        }
        scope.insert(key.clone(), value.clone());
    }
    Ok(scope)
}

fn space_scope(
    manifest: &MemoryManifest,
    space: &str,
    operation_scope: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>> {
    let space = manifest
        .memory
        .spaces
        .get(space)
        .with_context(|| format!("unknown Memory space `{space}`"))?;
    let mut scope = BTreeMap::new();
    for key in &space.scope {
        let value = operation_scope
            .get(key)
            .with_context(|| format!("Memory operation scope missing key `{key}`"))?;
        scope.insert(key.clone(), value.clone());
    }
    Ok(scope)
}

fn operation_trigger(operation: &MemoryOperation) -> &MemoryTrigger {
    match operation {
        MemoryOperation::Transform { trigger, .. }
        | MemoryOperation::Consolidate { trigger, .. }
        | MemoryOperation::Delete { trigger, .. } => trigger,
    }
}

fn memory_trigger_type(trigger: &MemoryTrigger) -> &'static str {
    match trigger {
        MemoryTrigger::External => "external",
        MemoryTrigger::RecordCount { .. } => "record_count",
        MemoryTrigger::Capacity { .. } => "capacity",
        MemoryTrigger::Interval { .. } => "interval",
    }
}

fn lifecycle_trigger_precondition(
    manifest: &MemoryManifest,
    operation: &MemoryOperationRuntimeSnapshot,
    operation_manifest: &MemoryOperation,
    operation_scope: &BTreeMap<String, String>,
) -> Result<Option<LocalMemoryLifecycleTriggerPrecondition>> {
    match operation_trigger(operation_manifest) {
        MemoryTrigger::External | MemoryTrigger::Interval { .. } => Ok(None),
        MemoryTrigger::RecordCount { space, threshold } => Ok(Some(
            LocalMemoryLifecycleTriggerPrecondition::ActiveCountAtLeast {
                package: operation.package.clone(),
                package_version: operation.package_version.clone(),
                space: space.clone(),
                scope: space_scope(manifest, space, operation_scope)?,
                threshold: *threshold,
            },
        )),
        MemoryTrigger::Capacity { space } => {
            let max_records = manifest
                .memory
                .spaces
                .get(space)
                .and_then(|space| space.capacity.as_ref())
                .map(|capacity| capacity.max_records)
                .context("capacity trigger references a space without capacity")?;
            Ok(Some(
                LocalMemoryLifecycleTriggerPrecondition::ActiveCountAtCapacity {
                    package: operation.package.clone(),
                    package_version: operation.package_version.clone(),
                    space: space.clone(),
                    scope: space_scope(manifest, space, operation_scope)?,
                    max_records,
                },
            ))
        }
    }
}

fn memory_trigger_touches_change(
    trigger: &MemoryTrigger,
    changed: Option<&MemoryChangeContext>,
) -> bool {
    let Some(changed) = changed else {
        return matches!(trigger, MemoryTrigger::Interval { .. });
    };
    match trigger {
        MemoryTrigger::External => false,
        MemoryTrigger::RecordCount { space, .. } | MemoryTrigger::Capacity { space } => {
            space == &changed.space
        }
        MemoryTrigger::Interval { .. } => true,
    }
}

fn memory_trigger_space_json(trigger: &Value) -> Option<&str> {
    match trigger.get("type").and_then(Value::as_str) {
        Some("record_count") | Some("capacity") => trigger.get("space").and_then(Value::as_str),
        _ => None,
    }
}

fn memory_trigger_type_json(trigger: &Value) -> &str {
    trigger
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
}

fn record_is_derived_from_any(
    record: &StoredMemoryRecord,
    source_ids: &std::collections::BTreeSet<String>,
) -> bool {
    record
        .provenance
        .get("harness_lifecycle")
        .and_then(|provenance| provenance.get("source_record_ids"))
        .and_then(Value::as_array)
        .is_some_and(|ids| {
            ids.iter()
                .filter_map(Value::as_str)
                .any(|id| source_ids.contains(id))
        })
}

fn sort_lifecycle_sources(records: &mut [StoredMemoryRecord]) {
    records.sort_by(|left, right| {
        left.space
            .cmp(&right.space)
            .then_with(|| left.record_type.cmp(&right.record_type))
            .then_with(|| left.created_at.cmp(&right.created_at))
            .then_with(|| left.ordinal.cmp(&right.ordinal))
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn lifecycle_source_snapshots(
    session: &mut HarnessSession,
    sources: &[StoredMemoryRecord],
) -> Result<Vec<LocalMemoryLifecycleSourceSnapshot>> {
    sources
        .iter()
        .map(|source| {
            session
                .local_memory_runtime()?
                .source_snapshot_for_lifecycle(source)
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn source_handling_mutations<'a>(
    package: &'a str,
    package_version: &'a str,
    manifest: &'a MemoryManifest,
    contracts: &'a crate::harness_runtime::memory::ValidatedMemoryContracts,
    source_handling: &MemorySourceHandling,
    sources: &'a [StoredMemoryRecord],
    operation_scope: &BTreeMap<String, String>,
    now: DateTime<Utc>,
) -> Result<Vec<LocalMemoryWriteRequest<'a>>> {
    match source_handling {
        MemorySourceHandling::Retain => return Ok(Vec::new()),
        MemorySourceHandling::RetainUntilExpiration => {
            for source in sources {
                let space = manifest.memory.spaces.get(&source.space).ok_or_else(|| {
                    anyhow!(
                        "Memory lifecycle source space `{}` is not declared",
                        source.space
                    )
                })?;
                if space.retention.is_none() || source.expires_at.is_none() {
                    bail!(
                        "Memory lifecycle source_handling `retain_until_expiration` requires source space `{}` to declare retention and source record `{}` to have an expiration",
                        source.space,
                        source.id
                    );
                }
            }
            return Ok(Vec::new());
        }
        MemorySourceHandling::DeleteAfterSuccess => {}
    }
    let mut mutations = Vec::new();
    for source in sources {
        mutations.push(LocalMemoryWriteRequest {
            package,
            package_version,
            manifest,
            contracts,
            space: &source.space,
            record_type: &source.record_type,
            scope: space_scope(manifest, &source.space, operation_scope)?,
            operation: LocalMemoryWriteOperation::Delete,
            record_id: Some(source.id.clone()),
            content: None,
            provenance: json!({}),
            now,
        });
    }
    Ok(mutations)
}

#[allow(clippy::too_many_arguments)]
fn completed_operation_state<'a>(
    session: &mut HarnessSession,
    manifest: &MemoryManifest,
    operation: &MemoryOperationRuntimeSnapshot,
    operation_manifest: &MemoryOperation,
    operation_scope: &BTreeMap<String, String>,
    output_writes: &[LocalMemoryWriteRequest<'a>],
    source_mutations: &[LocalMemoryWriteRequest<'a>],
    capacity_relief: bool,
    now: DateTime<Utc>,
    watermark: Option<Value>,
) -> Result<LocalMemoryOperationStateRow> {
    let trigger = operation_trigger(operation_manifest);
    let existing = session.local_memory_runtime()?.load_operation_state(
        &operation.package,
        &operation.package_version,
        &operation.operation,
        operation_scope,
    )?;
    let baseline_at = existing
        .as_ref()
        .and_then(|state| state.baseline_at.as_deref())
        .and_then(parse_optional_time)
        .or(Some(now));
    let mut state = LocalMemoryOperationStateRow {
        package: operation.package.clone(),
        package_version: operation.package_version.clone(),
        operation: operation.operation.clone(),
        scope: operation_scope.clone(),
        trigger_type: memory_trigger_type(trigger).into(),
        armed: false,
        baseline_at,
        last_completed_at: Some(now),
        last_failed_at: existing
            .as_ref()
            .and_then(|state| state.last_failed_at.as_deref())
            .and_then(parse_optional_time),
        next_eligible_at: None,
        last_observed_value: None,
        last_failure: existing
            .as_ref()
            .and_then(|state| state.last_failure.clone()),
        watermark,
        updated_at: now,
    };
    match trigger {
        MemoryTrigger::External => {}
        MemoryTrigger::RecordCount { space, threshold } => {
            let count = projected_active_count_after_commit(
                session,
                manifest,
                operation,
                space,
                operation_scope,
                output_writes,
                source_mutations,
            )?;
            state.armed = count < *threshold as i64;
            state.last_observed_value = Some(count);
        }
        MemoryTrigger::Capacity { space } => {
            let max_records = manifest
                .memory
                .spaces
                .get(space)
                .and_then(|space| space.capacity.as_ref())
                .map(|capacity| capacity.max_records as i64)
                .context("capacity trigger references a space without capacity")?;
            let count = projected_active_count_after_commit(
                session,
                manifest,
                operation,
                space,
                operation_scope,
                output_writes,
                source_mutations,
            )?;
            state.armed = !capacity_relief && count < max_records;
            state.last_observed_value = Some(count.min(max_records));
        }
        MemoryTrigger::Interval { every } => {
            state.armed = true;
            state.next_eligible_at = Some(now + parse_lifecycle_duration(every)?);
        }
    }
    Ok(state)
}

#[allow(clippy::too_many_arguments)]
fn failed_operation_state(
    session: &mut HarnessSession,
    manifest: &MemoryManifest,
    operation: &MemoryOperationRuntimeSnapshot,
    operation_manifest: &MemoryOperation,
    operation_scope: &BTreeMap<String, String>,
    code: &str,
    message: &str,
    now: DateTime<Utc>,
) -> Result<LocalMemoryOperationStateRow> {
    let trigger = operation_trigger(operation_manifest);
    let existing = session.local_memory_runtime()?.load_operation_state(
        &operation.package,
        &operation.package_version,
        &operation.operation,
        operation_scope,
    )?;
    let (next_eligible_at, last_observed_value) = match trigger {
        MemoryTrigger::External => (
            existing
                .as_ref()
                .and_then(|state| state.next_eligible_at.as_deref())
                .and_then(parse_optional_time),
            existing
                .as_ref()
                .and_then(|state| state.last_observed_value),
        ),
        MemoryTrigger::RecordCount { space, threshold } => {
            let scope = space_scope(manifest, space, operation_scope)?;
            let count = session.local_memory_runtime()?.active_record_count(
                &operation.package,
                &operation.package_version,
                space,
                &scope,
                None,
            )?;
            (
                (count >= *threshold).then(|| now + memory_operation_failure_backoff()),
                Some(count as i64),
            )
        }
        MemoryTrigger::Capacity { space } => {
            let scope = space_scope(manifest, space, operation_scope)?;
            let count = session.local_memory_runtime()?.active_record_count(
                &operation.package,
                &operation.package_version,
                space,
                &scope,
                None,
            )?;
            let max_records = manifest
                .memory
                .spaces
                .get(space)
                .and_then(|space| space.capacity.as_ref())
                .map(|capacity| capacity.max_records)
                .context("capacity trigger references a space without capacity")?;
            (
                (count >= max_records).then(|| now + memory_operation_failure_backoff()),
                Some((count as i64).min(max_records as i64)),
            )
        }
        MemoryTrigger::Interval { every } => (
            Some(now + parse_lifecycle_duration(every)?),
            existing
                .as_ref()
                .and_then(|state| state.last_observed_value),
        ),
    };
    Ok(LocalMemoryOperationStateRow {
        package: operation.package.clone(),
        package_version: operation.package_version.clone(),
        operation: operation.operation.clone(),
        scope: operation_scope.clone(),
        trigger_type: memory_trigger_type(trigger).into(),
        armed: existing.as_ref().is_none_or(|state| state.armed),
        baseline_at: existing
            .as_ref()
            .and_then(|state| state.baseline_at.as_deref())
            .and_then(parse_optional_time)
            .or(Some(now)),
        last_completed_at: existing
            .as_ref()
            .and_then(|state| state.last_completed_at.as_deref())
            .and_then(parse_optional_time),
        last_failed_at: Some(now),
        next_eligible_at,
        last_observed_value,
        last_failure: Some(json!({
            "code": code,
            "message": message,
        })),
        watermark: existing.as_ref().and_then(|state| state.watermark.clone()),
        updated_at: now,
    })
}

#[allow(clippy::too_many_arguments)]
fn projected_active_count_after_commit<'a>(
    session: &mut HarnessSession,
    manifest: &MemoryManifest,
    operation: &MemoryOperationRuntimeSnapshot,
    trigger_space: &str,
    operation_scope: &BTreeMap<String, String>,
    output_writes: &[LocalMemoryWriteRequest<'a>],
    source_mutations: &[LocalMemoryWriteRequest<'a>],
) -> Result<i64> {
    let trigger_scope = space_scope(manifest, trigger_space, operation_scope)?;
    let mut count = session.local_memory_runtime()?.active_record_count(
        &operation.package,
        &operation.package_version,
        trigger_space,
        &trigger_scope,
        None,
    )? as i64;
    let mut removed = std::collections::BTreeSet::new();
    for write in source_mutations {
        if write.package == operation.package
            && write.package_version == operation.package_version
            && write.space == trigger_space
            && write.scope == trigger_scope
            && matches!(
                write.operation,
                LocalMemoryWriteOperation::Delete | LocalMemoryWriteOperation::Archive
            )
            && let Some(record_id) = write.record_id.as_deref()
            && removed.insert(record_id.to_string())
        {
            count -= 1;
        }
    }
    for write in output_writes {
        if write.package == operation.package
            && write.package_version == operation.package_version
            && write.space == trigger_space
            && write.scope == trigger_scope
            && matches!(write.operation, LocalMemoryWriteOperation::Create)
        {
            count += 1;
        }
    }
    Ok(count.max(0))
}

fn lifecycle_provenance(
    operation: &MemoryOperationRuntimeSnapshot,
    sources: &[StoredMemoryRecord],
    preserve_provenance: bool,
) -> Value {
    if sources.is_empty() {
        return json!({});
    }
    let source_records = sources
        .iter()
        .map(|source| {
            json!({
                "package": source.package.clone(),
                "package_version": source.package_version.clone(),
                "space": source.space.clone(),
                "record_type": source.record_type.clone(),
                "id": source.id.clone(),
                "scope_hash": source.scope_hash.clone()
            })
        })
        .collect::<Vec<_>>();
    let mut lifecycle = serde_json::Map::from_iter([
        ("kind".into(), json!("harness_memory_lifecycle_operation")),
        ("operation".into(), json!(operation.operation.clone())),
        (
            "operation_identity".into(),
            json!(memory_operation_identity(
                &operation.package,
                &operation.operation
            )),
        ),
        (
            "source_record_ids".into(),
            json!(
                sources
                    .iter()
                    .map(|source| source.id.clone())
                    .collect::<Vec<_>>()
            ),
        ),
        ("source_records".into(), Value::Array(source_records)),
    ]);
    if preserve_provenance {
        lifecycle.insert(
            "preserved_source_provenance".into(),
            Value::Array(
                sources
                    .iter()
                    .map(|source| source.provenance.clone())
                    .collect(),
            ),
        );
    }
    let mut provenance = serde_json::Map::new();
    provenance.insert("harness_lifecycle".into(), Value::Object(lifecycle));
    Value::Object(provenance)
}

fn parse_optional_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

fn parse_lifecycle_duration(value: &str) -> Result<chrono::Duration> {
    parse_supported_positive_iso8601_duration(value)
        .with_context(|| format!("unsupported Memory lifecycle interval duration `{value}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_duration_parser_rejects_non_iso_shorthand() {
        assert_eq!(
            parse_lifecycle_duration("PT5M").unwrap(),
            chrono::Duration::minutes(5)
        );

        for value in ["5m", "30s", "2h", "7d"] {
            assert!(
                parse_lifecycle_duration(value).is_err(),
                "expected runtime lifecycle duration `{value}` to be rejected"
            );
        }
    }
}
