use super::effective_phase::{
    memory_action_identity, memory_read_mode_label, memory_write_operation_label,
};
use super::*;

pub(super) fn operation_summaries_for_action_kind(
    action_summaries: &[ActionReportSummary],
    action_kind: &str,
) -> Vec<OperationReportSummary> {
    let mut counts: BTreeMap<(String, String), u64> = BTreeMap::new();
    for action in action_summaries
        .iter()
        .filter(|action| action.action_kind == action_kind)
    {
        *counts
            .entry((action.identity.clone(), action.status.clone()))
            .or_insert(0) += 1;
    }
    counts
        .into_iter()
        .map(|((identity, status), count)| OperationReportSummary {
            operation_kind: action_kind.to_string(),
            identity,
            status,
            count,
        })
        .collect()
}

pub(super) fn memory_summaries_for_actions(
    action_summaries: &[ActionReportSummary],
) -> Vec<OperationReportSummary> {
    let mut summaries = operation_summaries_for_action_kind(action_summaries, "memory_read");
    summaries.extend(operation_summaries_for_action_kind(
        action_summaries,
        "memory_write",
    ));
    summaries
}

pub(super) fn model_turn_trace_fields(turn: &ModelTurn) -> BTreeMap<String, Value> {
    let mut fields = BTreeMap::from([("semantic_actions".into(), json!(turn.actions.len()))]);
    if let Some(content) = &turn.assistant_content {
        fields.insert("assistant_content".into(), json!(content));
    }
    if let Some(reason) = &turn.finish_reason {
        fields.insert("finish_reason".into(), json!(reason));
    }
    if !turn.actions.is_empty() {
        fields.insert(
            "proposed_actions".into(),
            json!(
                turn.actions
                    .iter()
                    .map(|proposal| {
                        json!({
                            "id": proposal.id,
                            "action_kind": proposal.action.kind(),
                            "identity": proposal.action.identity(),
                            "fields": action_trace_fields(&proposal.action),
                        })
                    })
                    .collect::<Vec<_>>()
            ),
        );
    }
    fields
}

pub(super) fn action_result_trace_fields(
    action: &SemanticAction,
    result: &ActionDispatchResult,
    attempt: u64,
    action_source: Option<&str>,
) -> BTreeMap<String, Value> {
    let mut fields = action_trace_fields(action);
    fields.insert("attempt".into(), json!(attempt));
    if let Some(source) = action_source {
        fields.insert("source".into(), json!(source));
    }
    fields.insert("result".into(), result.output.clone());
    if let Some(error) = &result.error {
        fields.insert("error".into(), json!(error));
    }
    if let Some(category) = result.failure_category {
        fields.insert("failure_category".into(), json!(category));
    }
    if let Some(status) = result.terminal_status {
        fields.insert("terminal_status".into(), json!(status));
    }
    fields
}

pub(super) fn should_retry_tool_failure(result: &ActionDispatchResult) -> bool {
    if result.terminal_status.is_some() {
        return false;
    }
    result
        .failure_category
        .map(|category| category.is_retryable_tool_failure())
        .unwrap_or(true)
}

pub(super) fn action_source(action: &SemanticAction, phase: &EffectivePhase) -> Option<String> {
    match action {
        SemanticAction::AgentPmTool { tool, .. } => capability_source(phase, "agentpm_tool", tool),
        SemanticAction::SkillResourceRead { skill, .. } => {
            capability_source(phase, "skill_resource_read", skill)
        }
        SemanticAction::KnowledgeRequest { package, .. } => {
            capability_source(phase, "knowledge_request", package)
        }
        SemanticAction::MemoryRead { package, space, .. } => capability_source(
            phase,
            "memory_read",
            &memory_action_identity(package, space),
        ),
        SemanticAction::MemoryWrite { package, space, .. } => capability_source(
            phase,
            "memory_write",
            &memory_action_identity(package, space),
        ),
        _ => None,
    }
}

pub(super) fn capability_source(
    phase: &EffectivePhase,
    action_kind: &str,
    identity: &str,
) -> Option<String> {
    phase
        .capability_catalog
        .iter()
        .find(|descriptor| descriptor.action_kind == action_kind && descriptor.identity == identity)
        .map(|descriptor| descriptor.source.clone())
}

pub(super) fn action_result_transcript_content(action: &SemanticAction, result: Value) -> Value {
    json!({
        "action_kind": action.kind(),
        "identity": action.identity(),
        "result": result,
    })
}

pub(super) fn action_trace_fields(action: &SemanticAction) -> BTreeMap<String, Value> {
    match action {
        SemanticAction::AgentPmTool { arguments, .. } => {
            BTreeMap::from([("arguments".into(), arguments.clone())])
        }
        SemanticAction::ExternalMcpTool {
            server,
            tool,
            arguments,
        } => BTreeMap::from([
            ("server".into(), json!(server)),
            ("tool".into(), json!(tool)),
            ("arguments".into(), arguments.clone()),
        ]),
        SemanticAction::SkillResourceRead { resource, .. } => {
            BTreeMap::from([("resource".into(), json!(resource))])
        }
        SemanticAction::KnowledgeRequest {
            mode,
            document,
            query,
            top_k,
            score_threshold,
            return_citations,
            ..
        } => {
            let mut fields = BTreeMap::new();
            if let Some(mode) = mode {
                fields.insert("mode".into(), json!(mode));
            }
            if let Some(document) = document {
                fields.insert("document".into(), json!(document));
            }
            if let Some(query) = query {
                fields.insert("query".into(), json!(query));
            }
            if let Some(top_k) = top_k {
                fields.insert("top_k".into(), json!(top_k));
            }
            if let Some(score_threshold) = score_threshold {
                fields.insert("score_threshold".into(), json!(score_threshold));
            }
            if let Some(return_citations) = return_citations {
                fields.insert("return_citations".into(), json!(return_citations));
            }
            fields
        }
        SemanticAction::MemoryRead {
            space,
            mode,
            record_id,
            record_type,
            filter,
            query,
            limit,
            ..
        } => {
            let mut fields = BTreeMap::from([
                ("space".into(), json!(space)),
                ("mode".into(), json!(memory_read_mode_label(*mode))),
            ]);
            if let Some(record_id) = record_id {
                fields.insert("record_id".into(), json!(record_id));
            }
            if let Some(record_type) = record_type {
                fields.insert("record_type".into(), json!(record_type));
            }
            if !filter.is_empty() {
                fields.insert("filter".into(), json!(filter));
            }
            if let Some(query) = query {
                fields.insert("query".into(), json!(query));
            }
            if let Some(limit) = limit {
                fields.insert("limit".into(), json!(limit));
            }
            fields
        }
        SemanticAction::MemoryWrite {
            space,
            operation,
            record_type,
            record_id,
            content,
            ..
        } => {
            let mut fields = BTreeMap::from([
                ("space".into(), json!(space)),
                (
                    "operation".into(),
                    json!(memory_write_operation_label(*operation)),
                ),
                ("record_type".into(), json!(record_type)),
            ]);
            if let Some(record_id) = record_id {
                fields.insert("record_id".into(), json!(record_id));
            }
            if let Some(content) = content {
                fields.insert("content".into(), content.clone());
            }
            fields
        }
        SemanticAction::PhaseCompletion { outcome, output } => {
            let mut fields = BTreeMap::new();
            if let Some(outcome) = outcome {
                fields.insert("outcome".into(), json!(outcome));
            }
            if let Some(output) = output {
                fields.insert("output".into(), output.clone());
            }
            fields
        }
    }
}

pub(super) fn action_request_event_type(action: &SemanticAction) -> Option<HarnessEventType> {
    match action {
        SemanticAction::AgentPmTool { .. } => Some(HarnessEventType::ToolInvoked),
        SemanticAction::ExternalMcpTool { .. } => Some(HarnessEventType::McpToolInvoked),
        SemanticAction::SkillResourceRead { .. } => Some(HarnessEventType::SkillResourceRequested),
        SemanticAction::MemoryRead { .. } => Some(HarnessEventType::MemoryReadStarted),
        SemanticAction::MemoryWrite { .. } => Some(HarnessEventType::MemoryWriteStarted),
        _ => None,
    }
}

pub(super) fn action_dispatch_event_type(action: &SemanticAction, ok: bool) -> HarnessEventType {
    match action {
        SemanticAction::AgentPmTool { .. } => {
            if ok {
                HarnessEventType::ToolCompleted
            } else {
                HarnessEventType::ToolFailed
            }
        }
        SemanticAction::ExternalMcpTool { .. } => {
            if ok {
                HarnessEventType::McpToolCompleted
            } else {
                HarnessEventType::McpToolFailed
            }
        }
        SemanticAction::SkillResourceRead { .. } => {
            if ok {
                HarnessEventType::SkillResourceLoaded
            } else {
                HarnessEventType::SkillResourceFailed
            }
        }
        SemanticAction::KnowledgeRequest { .. } => {
            if ok {
                HarnessEventType::KnowledgeRetrieved
            } else {
                HarnessEventType::KnowledgeFailed
            }
        }
        SemanticAction::MemoryRead { .. } => {
            if ok {
                HarnessEventType::MemoryReadCompleted
            } else {
                HarnessEventType::MemoryReadFailed
            }
        }
        SemanticAction::MemoryWrite { .. } => {
            if ok {
                HarnessEventType::MemoryWriteCompleted
            } else {
                HarnessEventType::MemoryWriteFailed
            }
        }
        SemanticAction::PhaseCompletion { .. } => HarnessEventType::SemanticActionCompleted,
    }
}

pub(super) fn terminal_event_type(status: HarnessTerminalStatus) -> HarnessEventType {
    match status {
        HarnessTerminalStatus::Ended | HarnessTerminalStatus::HandedOff => {
            HarnessEventType::RunCompleted
        }
        HarnessTerminalStatus::Aborted | HarnessTerminalStatus::Failed => {
            HarnessEventType::RunFailed
        }
        HarnessTerminalStatus::Cancelled => HarnessEventType::RunCancelled,
        HarnessTerminalStatus::LimitReached => HarnessEventType::RunLimitReached,
        HarnessTerminalStatus::ApprovalRequired => HarnessEventType::RunApprovalRequired,
    }
}

pub(super) fn hook_id_label(hook: &HarnessHookId) -> &'static str {
    match hook {
        HarnessHookId::BeforeModelRequest => "before_model_request",
        HarnessHookId::BeforeToolSelection => "before_tool_selection",
        HarnessHookId::BeforeToolCall => "before_tool_call",
        HarnessHookId::BeforeKnowledgeRequest => "before_knowledge_request",
        HarnessHookId::AfterKnowledgeRetrieval => "after_knowledge_retrieval",
        HarnessHookId::BeforeMemoryRead => "before_memory_read",
        HarnessHookId::BeforeMemoryWrite => "before_memory_write",
        HarnessHookId::BeforeMemoryOperation => "before_memory_operation",
    }
}

pub(super) fn hook_event_fields(
    hook: &HarnessHookId,
    phase_id: &str,
    binding_count: usize,
    mut extra_fields: BTreeMap<String, Value>,
) -> BTreeMap<String, Value> {
    extra_fields.insert("hook".into(), json!(hook_id_label(hook)));
    extra_fields.insert("phase_id".into(), json!(phase_id));
    extra_fields.insert("binding_count".into(), json!(binding_count));
    extra_fields
}

pub(super) fn tool_candidate_ids(effective_phase: &EffectivePhase) -> Vec<String> {
    effective_phase
        .capability_catalog
        .iter()
        .filter(|descriptor| descriptor.action_kind == "agentpm_tool")
        .map(|descriptor| descriptor.identity.clone())
        .collect()
}

pub(super) fn argument_keys(arguments: &Value) -> Vec<String> {
    let Some(arguments) = arguments.as_object() else {
        return Vec::new();
    };
    let mut keys = arguments.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    keys
}

pub(super) fn provider_option_keys(request: &ModelRequest) -> Vec<String> {
    let Some(model) = &request.model else {
        return Vec::new();
    };
    let Some(options) = model.options.as_object() else {
        return Vec::new();
    };
    let mut keys = options.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    keys
}

pub(super) fn mutable_model_request_sections(request: &ModelRequest) -> usize {
    request
        .prompt
        .sections
        .iter()
        .filter(|section| section.title == CONSUMER_RUN_CONTEXT_SECTION_TITLE)
        .count()
}

pub(super) fn status_str(status: HarnessTerminalStatus) -> &'static str {
    match status {
        HarnessTerminalStatus::Ended => "ended",
        HarnessTerminalStatus::HandedOff => "handed_off",
        HarnessTerminalStatus::Aborted => "aborted",
        HarnessTerminalStatus::Failed => "failed",
        HarnessTerminalStatus::Cancelled => "cancelled",
        HarnessTerminalStatus::LimitReached => "limit_reached",
        HarnessTerminalStatus::ApprovalRequired => "approval_required",
    }
}

pub(super) fn harness_status_from_str(value: &str) -> Option<HarnessTerminalStatus> {
    match value {
        "ended" => Some(HarnessTerminalStatus::Ended),
        "handed_off" => Some(HarnessTerminalStatus::HandedOff),
        "aborted" => Some(HarnessTerminalStatus::Aborted),
        "failed" => Some(HarnessTerminalStatus::Failed),
        "cancelled" => Some(HarnessTerminalStatus::Cancelled),
        "limit_reached" => Some(HarnessTerminalStatus::LimitReached),
        "approval_required" => Some(HarnessTerminalStatus::ApprovalRequired),
        _ => None,
    }
}

pub(super) fn add_optional<T>(left: Option<T>, right: Option<T>) -> Option<T>
where
    T: std::ops::Add<Output = T>,
{
    match (left, right) {
        (Some(left), Some(right)) => Some(left + right),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}
