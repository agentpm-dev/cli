#![allow(dead_code)]

use super::action::SemanticAction;
use super::model::{
    CONSUMER_RUN_CONTEXT_SECTION_TITLE, CompletionContract, ModelProviderSelection, ModelRequest,
    PromptSection,
};
use super::service::{
    HostServiceInvoker, ProcessServiceClient, ProcessServiceConfig, ServiceLifecycleEmitter,
};
use crate::harness_config::{
    HarnessHookBinding, HarnessHookFailurePolicy, HarnessHookId, HarnessImplementation,
    HarnessImplementationEntry,
};
use crate::harness_engine::EffectivePhase;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeforeToolSelectionHook {
    pub phase: HookPhaseSnapshot,
    pub candidates: Vec<BeforeToolSelectionCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookPhaseSnapshot {
    pub phase_id: String,
    pub phase_objective: String,
    pub completion: CompletionContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeforeToolSelectionCandidate {
    pub canonical_id: String,
    pub description: String,
    pub source: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BeforeToolSelectionDecision {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeforeModelRequestHook {
    pub run_id: String,
    pub phase_execution_id: String,
    pub phase: BeforeModelRequestPhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelProviderSelection>,
    pub sections: Vec<BeforeModelRequestSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repair_feedback: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BeforeModelRequestPhase {
    pub phase_id: String,
    pub phase_objective: String,
    pub completion: CompletionContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeforeModelRequestSection {
    pub number: u8,
    pub title: String,
    pub content: String,
    pub mutable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeforeModelRequestContextSection {
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BeforeModelRequestDecision {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub context_sections: Vec<BeforeModelRequestContextSection>,
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub provider_options: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeforeToolCallHook {
    pub phase_id: String,
    pub tool: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BeforeToolCallDecision {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookRuntimeFailure {
    pub hook: HarnessHookId,
    pub message: String,
    pub kind: HookRuntimeFailureKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookRuntimeFailureKind {
    Failure,
    Rejection,
}

impl HookRuntimeFailure {
    pub fn new(hook: HarnessHookId, message: impl Into<String>) -> Self {
        Self {
            hook,
            message: message.into(),
            kind: HookRuntimeFailureKind::Failure,
        }
    }

    pub fn rejection(hook: HarnessHookId, message: impl Into<String>) -> Self {
        Self {
            hook,
            message: message.into(),
            kind: HookRuntimeFailureKind::Rejection,
        }
    }

    pub fn is_rejection(&self) -> bool {
        self.kind == HookRuntimeFailureKind::Rejection
    }
}

pub trait HookRuntime {
    fn has_hook(&self, _hook: HarnessHookId) -> bool {
        false
    }

    fn binding_count(&self, hook: &HarnessHookId) -> usize {
        usize::from(self.has_hook(hook.clone()))
    }

    fn before_tool_selection(
        &mut self,
        _hook: BeforeToolSelectionHook,
    ) -> Result<BeforeToolSelectionDecision, HookRuntimeFailure> {
        Ok(BeforeToolSelectionDecision::default())
    }

    fn before_model_request(
        &mut self,
        _hook: BeforeModelRequestHook,
    ) -> Result<BeforeModelRequestDecision, HookRuntimeFailure> {
        Ok(BeforeModelRequestDecision::default())
    }

    fn before_tool_call(
        &mut self,
        _hook: BeforeToolCallHook,
    ) -> Result<BeforeToolCallDecision, HookRuntimeFailure> {
        Ok(BeforeToolCallDecision::default())
    }

    fn drain_nonfatal_failures(&mut self) -> Vec<HookRuntimeFailure> {
        Vec::new()
    }
}

pub struct NoopHookRuntime;

impl HookRuntime for NoopHookRuntime {}

pub struct ConfiguredHookRuntime {
    bindings: Vec<HarnessHookBinding>,
    implementations: HashMap<String, HookImplementationRuntime>,
    host_invoker: Option<Box<dyn HostServiceInvoker>>,
    nonfatal_failures: Vec<HookRuntimeFailure>,
}

#[derive(Debug, Clone)]
pub struct SdkHostHookRegistration {
    pub registry_id: String,
    pub hook: HarnessHookId,
    pub request_timeout_ms: u64,
}

enum HookImplementationRuntime {
    Process(Box<ProcessServiceClient>),
    Host { request_timeout_ms: u64 },
}

impl ConfiguredHookRuntime {
    pub fn from_config(
        workspace_root: &Path,
        bindings: &[HarnessHookBinding],
        implementations: &HashMap<String, HarnessImplementationEntry>,
        lifecycle_events: Option<ServiceLifecycleEmitter>,
    ) -> anyhow::Result<Self> {
        let mut runtimes = HashMap::new();
        for binding in bindings {
            if runtimes.contains_key(&binding.implementation) {
                continue;
            }
            let Some(entry) = implementations.get(&binding.implementation) else {
                continue;
            };
            let runtime = match &entry.implementation {
                HarnessImplementation::Process { .. } => {
                    let hook_ids =
                        configured_hook_ids_for_implementation(bindings, &binding.implementation);
                    let mut initialize_payload = Map::new();
                    initialize_payload.insert(
                        "hooks".into(),
                        json!(hook_ids.iter().map(hook_method).collect::<Vec<_>>()),
                    );
                    let client = ProcessServiceClient::start(ProcessServiceConfig {
                        service: "hook".into(),
                        registry_id: binding.implementation.clone(),
                        initialize_payload,
                        implementation: entry.implementation.clone(),
                        workspace_root: workspace_root.to_path_buf(),
                        lifecycle_events: lifecycle_events.clone(),
                    })?;
                    validate_hook_service_initialization(
                        client.initialization_result(),
                        &binding.implementation,
                        &hook_ids,
                    )?;
                    HookImplementationRuntime::Process(Box::new(client))
                }
                HarnessImplementation::Host { request_timeout_ms } => {
                    HookImplementationRuntime::Host {
                        request_timeout_ms: *request_timeout_ms,
                    }
                }
            };
            runtimes.insert(binding.implementation.clone(), runtime);
        }
        Ok(Self {
            bindings: bindings.to_vec(),
            implementations: runtimes,
            host_invoker: None,
            nonfatal_failures: Vec::new(),
        })
    }

    pub fn with_host_invoker(mut self, host_invoker: Box<dyn HostServiceInvoker>) -> Self {
        self.host_invoker = Some(host_invoker);
        self
    }

    pub fn add_sdk_host_registrations(
        &mut self,
        registrations: impl IntoIterator<Item = SdkHostHookRegistration>,
    ) {
        for registration in registrations {
            self.implementations
                .entry(registration.registry_id.clone())
                .or_insert(HookImplementationRuntime::Host {
                    request_timeout_ms: registration.request_timeout_ms,
                });
            self.bindings.push(HarnessHookBinding {
                hook: registration.hook,
                implementation: registration.registry_id,
                failure_policy: HarnessHookFailurePolicy::Closed,
            });
        }
    }

    fn invoke_binding<T, U>(
        &mut self,
        hook: HarnessHookId,
        binding: &HarnessHookBinding,
        payload: &T,
    ) -> Result<Option<U>, HookRuntimeFailure>
    where
        T: Serialize,
        U: for<'de> Deserialize<'de>,
    {
        let result = match self.implementations.get_mut(&binding.implementation) {
            Some(HookImplementationRuntime::Process(client)) => client
                .request(
                    hook_method(&hook),
                    json!({ "hook": hook, "input": payload }),
                )
                .map_err(|err| HookRuntimeFailure::new(hook.clone(), err.to_string()))
                .and_then(|value| decode_hook_response(hook.clone(), value)),
            Some(HookImplementationRuntime::Host { request_timeout_ms }) => {
                let Some(invoker) = self.host_invoker.as_mut() else {
                    return Err(HookRuntimeFailure::new(
                        hook.clone(),
                        "host hook implementation is not registered",
                    ));
                };
                invoker
                    .invoke_host_service(
                        "hook",
                        &binding.implementation,
                        hook_method(&hook),
                        json!({ "hook": hook, "input": payload }),
                        *request_timeout_ms,
                    )
                    .map_err(|err| HookRuntimeFailure::new(hook.clone(), err.to_string()))
                    .and_then(|value| decode_hook_response(hook.clone(), value))
            }
            None => Err(HookRuntimeFailure::new(
                hook.clone(),
                "hook implementation is not configured",
            )),
        };
        match result {
            Ok(decision) => Ok(decision),
            Err(err) if err.is_rejection() => Err(err),
            Err(err) if binding.failure_policy == HarnessHookFailurePolicy::Continue => {
                self.nonfatal_failures.push(err);
                Ok(None)
            }
            Err(err) => Err(err),
        }
    }

    fn handle_binding_failure(
        &mut self,
        binding: &HarnessHookBinding,
        failure: HookRuntimeFailure,
    ) -> Result<(), HookRuntimeFailure> {
        if failure.is_rejection() {
            return Err(failure);
        }
        if binding.failure_policy == HarnessHookFailurePolicy::Continue {
            self.nonfatal_failures.push(failure);
            Ok(())
        } else {
            Err(failure)
        }
    }
}

impl HookRuntime for ConfiguredHookRuntime {
    fn has_hook(&self, hook: HarnessHookId) -> bool {
        self.bindings.iter().any(|binding| binding.hook == hook)
    }

    fn binding_count(&self, hook: &HarnessHookId) -> usize {
        self.bindings
            .iter()
            .filter(|binding| &binding.hook == hook)
            .count()
    }

    fn before_tool_selection(
        &mut self,
        mut hook: BeforeToolSelectionHook,
    ) -> Result<BeforeToolSelectionDecision, HookRuntimeFailure> {
        let mut patched = false;
        for binding in self
            .bindings
            .iter()
            .filter(|binding| binding.hook == HarnessHookId::BeforeToolSelection)
            .cloned()
            .collect::<Vec<_>>()
        {
            let Some(decision) =
                self.invoke_binding(HarnessHookId::BeforeToolSelection, &binding, &hook)?
            else {
                continue;
            };
            let decision: BeforeToolSelectionDecision = decision;
            if decision.candidate_ids.is_some() {
                if let Err(err) = apply_before_tool_selection_decision_to_hook(&mut hook, decision)
                {
                    self.handle_binding_failure(
                        &binding,
                        HookRuntimeFailure::new(HarnessHookId::BeforeToolSelection, err),
                    )?;
                    continue;
                }
                patched = true;
            }
        }
        Ok(BeforeToolSelectionDecision {
            candidate_ids: patched.then(|| {
                hook.candidates
                    .iter()
                    .map(|candidate| candidate.canonical_id.clone())
                    .collect()
            }),
        })
    }

    fn before_model_request(
        &mut self,
        mut hook: BeforeModelRequestHook,
    ) -> Result<BeforeModelRequestDecision, HookRuntimeFailure> {
        let mut combined = BeforeModelRequestDecision::default();
        for binding in self
            .bindings
            .iter()
            .filter(|binding| binding.hook == HarnessHookId::BeforeModelRequest)
            .cloned()
            .collect::<Vec<_>>()
        {
            let Some(decision) =
                self.invoke_binding(HarnessHookId::BeforeModelRequest, &binding, &hook)?
            else {
                continue;
            };
            let decision: BeforeModelRequestDecision = decision;
            if let Err(err) = apply_before_model_request_decision_to_hook(&mut hook, &decision) {
                self.handle_binding_failure(
                    &binding,
                    HookRuntimeFailure::new(HarnessHookId::BeforeModelRequest, err),
                )?;
                continue;
            }
            combined.context_sections.extend(decision.context_sections);
            combined.provider_options.extend(decision.provider_options);
        }
        Ok(combined)
    }

    fn before_tool_call(
        &mut self,
        mut hook: BeforeToolCallHook,
    ) -> Result<BeforeToolCallDecision, HookRuntimeFailure> {
        let mut patched = false;
        for binding in self
            .bindings
            .iter()
            .filter(|binding| binding.hook == HarnessHookId::BeforeToolCall)
            .cloned()
            .collect::<Vec<_>>()
        {
            let Some(decision) =
                self.invoke_binding(HarnessHookId::BeforeToolCall, &binding, &hook)?
            else {
                continue;
            };
            let decision: BeforeToolCallDecision = decision;
            if let Some(arguments) = decision.arguments {
                hook.arguments = arguments;
                patched = true;
            }
        }
        Ok(BeforeToolCallDecision {
            arguments: patched.then_some(hook.arguments),
        })
    }

    fn drain_nonfatal_failures(&mut self) -> Vec<HookRuntimeFailure> {
        std::mem::take(&mut self.nonfatal_failures)
    }
}

fn configured_hook_ids_for_implementation(
    bindings: &[HarnessHookBinding],
    implementation: &str,
) -> Vec<HarnessHookId> {
    let mut hook_ids = Vec::new();
    for binding in bindings
        .iter()
        .filter(|binding| binding.implementation == implementation)
    {
        if !hook_ids.contains(&binding.hook) {
            hook_ids.push(binding.hook.clone());
        }
    }
    hook_ids
}

pub(crate) fn validate_hook_service_initialization(
    initialization_result: &Value,
    expected_registry_id: &str,
    expected_hooks: &[HarnessHookId],
) -> anyhow::Result<()> {
    if initialization_result
        .get("ready")
        .and_then(Value::as_bool)
        .is_some_and(|ready| !ready)
    {
        anyhow::bail!("hook service `{expected_registry_id}` initialized but reported not ready");
    }
    if let Some(registry_id) = initialization_result
        .get("registry_id")
        .and_then(Value::as_str)
        && registry_id != expected_registry_id
    {
        anyhow::bail!(
            "hook service initialized as `{registry_id}`, expected `{expected_registry_id}`"
        );
    }
    let Some(advertised_hooks) = initialization_result.get("hooks") else {
        return Ok(());
    };
    let advertised_hooks: Vec<String> = serde_json::from_value(advertised_hooks.clone())?;
    for advertised_hook in &advertised_hooks {
        if !is_known_hook_id(advertised_hook) {
            anyhow::bail!(
                "hook service `{expected_registry_id}` advertised unknown hook `{advertised_hook}`"
            );
        }
    }
    for expected_hook in expected_hooks {
        let expected_hook = hook_method(expected_hook);
        if !advertised_hooks
            .iter()
            .any(|advertised_hook| advertised_hook == expected_hook)
        {
            anyhow::bail!(
                "hook service `{expected_registry_id}` does not advertise configured hook `{expected_hook}`"
            );
        }
    }
    Ok(())
}

fn is_known_hook_id(value: &str) -> bool {
    matches!(
        value,
        "before_model_request"
            | "before_tool_selection"
            | "before_tool_call"
            | "before_knowledge_request"
            | "after_knowledge_retrieval"
            | "before_memory_read"
            | "before_memory_write"
            | "before_memory_operation"
    )
}

fn hook_method(hook: &HarnessHookId) -> &'static str {
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

fn decode_hook_response<T>(
    hook: HarnessHookId,
    value: Value,
) -> Result<Option<T>, HookRuntimeFailure>
where
    T: for<'de> Deserialize<'de>,
{
    let Some(decision) = value.get("decision").and_then(Value::as_str) else {
        return Err(HookRuntimeFailure::new(
            hook,
            "hook response is missing required decision",
        ));
    };
    match decision {
        "continue" => {
            serde_json::from_value(value.get("patch").cloned().unwrap_or_else(|| json!({})))
                .map(Some)
                .map_err(|err| HookRuntimeFailure::new(hook, format!("invalid hook patch: {err}")))
        }
        "reject" => Err(HookRuntimeFailure::rejection(
            hook,
            value
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("hook rejected request"),
        )),
        other => Err(HookRuntimeFailure::new(
            hook,
            format!("unsupported hook decision `{other}`"),
        )),
    }
}

fn apply_before_tool_selection_decision_to_hook(
    hook: &mut BeforeToolSelectionHook,
    decision: BeforeToolSelectionDecision,
) -> Result<(), String> {
    let Some(candidate_ids) = decision.candidate_ids else {
        return Ok(());
    };
    let mut seen = std::collections::BTreeSet::new();
    let current_candidates = hook
        .candidates
        .iter()
        .cloned()
        .map(|candidate| (candidate.canonical_id.clone(), candidate))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut reordered_candidates = Vec::new();
    for candidate_id in candidate_ids {
        if !seen.insert(candidate_id.clone()) {
            return Err(format!(
                "before_tool_selection duplicated Tool `{candidate_id}`"
            ));
        }
        let Some(candidate) = current_candidates.get(&candidate_id) else {
            return Err(format!(
                "before_tool_selection introduced Tool `{candidate_id}`"
            ));
        };
        reordered_candidates.push(candidate.clone());
    }
    hook.candidates = reordered_candidates;
    Ok(())
}

pub fn apply_before_tool_selection_decision(
    phase: &mut EffectivePhase,
    decision: BeforeToolSelectionDecision,
) -> Result<(), String> {
    let Some(candidate_ids) = decision.candidate_ids else {
        return Ok(());
    };
    let mut seen = std::collections::BTreeSet::new();
    let original_tools = phase
        .capability_catalog
        .iter()
        .filter(|descriptor| descriptor.action_kind == "agentpm_tool")
        .cloned()
        .map(|descriptor| (descriptor.identity.clone(), descriptor))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut reordered_tools = Vec::new();
    for candidate_id in candidate_ids {
        if !seen.insert(candidate_id.clone()) {
            return Err(format!(
                "before_tool_selection duplicated Tool `{candidate_id}`"
            ));
        }
        let Some(original) = original_tools.get(&candidate_id) else {
            return Err(format!(
                "before_tool_selection introduced Tool `{candidate_id}`"
            ));
        };
        reordered_tools.push(original.clone());
    }
    let mut catalog = phase
        .capability_catalog
        .iter()
        .filter(|descriptor| descriptor.action_kind != "agentpm_tool")
        .cloned()
        .collect::<Vec<_>>();
    catalog.extend(reordered_tools.clone());
    phase.capability_catalog = catalog;
    phase.active_tools.retain(|tool| {
        reordered_tools
            .iter()
            .any(|descriptor| descriptor.identity == tool.name)
    });
    phase.active_tools.sort_by_key(|tool| {
        reordered_tools
            .iter()
            .position(|descriptor| descriptor.identity == tool.name)
            .unwrap_or(usize::MAX)
    });
    Ok(())
}

pub fn before_tool_selection_hook_from_phase(
    phase_id: &str,
    phase_objective: &str,
    completion: CompletionContract,
    effective_phase: &EffectivePhase,
) -> BeforeToolSelectionHook {
    BeforeToolSelectionHook {
        phase: HookPhaseSnapshot {
            phase_id: phase_id.to_string(),
            phase_objective: phase_objective.to_string(),
            completion,
        },
        candidates: effective_phase
            .capability_catalog
            .iter()
            .filter(|descriptor| descriptor.action_kind == "agentpm_tool")
            .map(|descriptor| BeforeToolSelectionCandidate {
                canonical_id: descriptor.identity.clone(),
                description: descriptor.description.clone(),
                source: descriptor.source.clone(),
            })
            .collect(),
    }
}

pub fn before_model_request_hook_from_request(request: &ModelRequest) -> BeforeModelRequestHook {
    BeforeModelRequestHook {
        run_id: request.run_id.clone(),
        phase_execution_id: request.phase_execution_id.clone(),
        phase: BeforeModelRequestPhase {
            phase_id: request.phase_id.clone(),
            phase_objective: request.phase_objective.clone(),
            completion: request.prompt.completion.clone(),
        },
        model: request.model.clone(),
        sections: request
            .prompt
            .sections
            .iter()
            .map(|section| BeforeModelRequestSection {
                number: section.number,
                title: section.title.clone(),
                content: section.content.clone(),
                mutable: is_before_model_request_mutable_section(section),
            })
            .collect(),
        repair_feedback: request.repair_feedback.clone(),
    }
}

pub fn apply_before_model_request_decision(
    request: &mut ModelRequest,
    decision: BeforeModelRequestDecision,
) -> Result<(), String> {
    if !decision.context_sections.is_empty() {
        append_before_model_request_context(
            &mut request.prompt.sections,
            decision.context_sections,
        )?;
    }
    if !decision.provider_options.is_empty() {
        merge_before_model_request_provider_options(request, decision.provider_options)?;
    }
    Ok(())
}

fn apply_before_model_request_decision_to_hook(
    hook: &mut BeforeModelRequestHook,
    decision: &BeforeModelRequestDecision,
) -> Result<(), String> {
    if !decision.context_sections.is_empty() {
        append_before_model_request_context_to_hook(
            &mut hook.sections,
            &decision.context_sections,
        )?;
    }
    if !decision.provider_options.is_empty() {
        let Some(model) = hook.model.as_mut() else {
            return Err(
                "before_model_request cannot patch provider options without a selected model"
                    .into(),
            );
        };
        let Some(options) = model.options.as_object_mut() else {
            return Err("before_model_request can only patch object provider options".into());
        };
        options.extend(decision.provider_options.clone());
    }
    Ok(())
}

fn append_before_model_request_context(
    sections: &mut [PromptSection],
    context_sections: Vec<BeforeModelRequestContextSection>,
) -> Result<(), String> {
    let Some(section) = sections
        .iter_mut()
        .find(|section| is_before_model_request_mutable_section(section))
    else {
        return Err("before_model_request could not find a mutable context section".into());
    };
    section.content.push_str("\n\nHook Context:");
    for context in context_sections {
        if context.title.trim().is_empty() {
            return Err("before_model_request context section title cannot be empty".into());
        }
        if context.content.trim().is_empty() {
            return Err("before_model_request context section content cannot be empty".into());
        }
        section
            .content
            .push_str(&format!("\n\n{}:\n{}", context.title, context.content));
    }
    Ok(())
}

fn append_before_model_request_context_to_hook(
    sections: &mut [BeforeModelRequestSection],
    context_sections: &[BeforeModelRequestContextSection],
) -> Result<(), String> {
    let Some(section) = sections
        .iter_mut()
        .find(|section| section.title == CONSUMER_RUN_CONTEXT_SECTION_TITLE && section.mutable)
    else {
        return Err("before_model_request could not find a mutable context section".into());
    };
    section.content.push_str("\n\nHook Context:");
    for context in context_sections {
        if context.title.trim().is_empty() {
            return Err("before_model_request context section title cannot be empty".into());
        }
        if context.content.trim().is_empty() {
            return Err("before_model_request context section content cannot be empty".into());
        }
        section
            .content
            .push_str(&format!("\n\n{}:\n{}", context.title, context.content));
    }
    Ok(())
}

fn merge_before_model_request_provider_options(
    request: &mut ModelRequest,
    provider_options: Map<String, Value>,
) -> Result<(), String> {
    let Some(model) = request.model.as_mut() else {
        return Err(
            "before_model_request cannot patch provider options without a selected model".into(),
        );
    };
    let Some(options) = model.options.as_object_mut() else {
        return Err("before_model_request can only patch object provider options".into());
    };
    options.extend(provider_options);
    if let Some(runtime_model) = request.runtime.model.as_mut() {
        if runtime_model.provider != model.provider || runtime_model.model != model.model {
            return Err("before_model_request found inconsistent model metadata".into());
        }
        runtime_model.options = model.options.clone();
    }
    Ok(())
}

fn is_before_model_request_mutable_section(section: &PromptSection) -> bool {
    section.title == CONSUMER_RUN_CONTEXT_SECTION_TITLE
}

pub fn apply_before_tool_call_decision(
    action: &SemanticAction,
    decision: BeforeToolCallDecision,
) -> Result<SemanticAction, String> {
    let Some(arguments) = decision.arguments else {
        return Ok(action.clone());
    };
    match action {
        SemanticAction::AgentPmTool { tool, .. } => Ok(SemanticAction::AgentPmTool {
            tool: tool.clone(),
            arguments,
        }),
        SemanticAction::ExternalMcpTool { .. } => {
            Err("before_tool_call cannot patch external MCP Tool arguments yet".into())
        }
        _ => Ok(action.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::CapabilityDescriptor;
    use super::*;
    use anyhow::Result;
    use std::collections::{HashMap, VecDeque};
    use std::fs;
    use std::sync::{Arc, Mutex};

    struct FakeHostInvoker {
        response: Value,
    }

    impl HostServiceInvoker for FakeHostInvoker {
        fn invoke_host_service(
            &mut self,
            _role: &str,
            _registry_id: &str,
            _method: &str,
            _payload: Value,
            _timeout_ms: u64,
        ) -> Result<Value> {
            Ok(self.response.clone())
        }
    }

    struct SequenceHostInvoker {
        responses: VecDeque<Value>,
        payloads: Arc<Mutex<Vec<Value>>>,
    }

    impl HostServiceInvoker for SequenceHostInvoker {
        fn invoke_host_service(
            &mut self,
            _role: &str,
            _registry_id: &str,
            _method: &str,
            payload: Value,
            _timeout_ms: u64,
        ) -> Result<Value> {
            self.payloads.lock().unwrap().push(payload);
            Ok(self.responses.pop_front().expect("host response"))
        }
    }

    #[test]
    fn before_tool_selection_rejects_unknown_patch_fields() {
        let err = serde_json::from_value::<BeforeToolSelectionDecision>(json!({
            "capability_catalog": []
        }))
        .unwrap_err();
        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn hook_service_initialization_validates_advertised_hooks() {
        validate_hook_service_initialization(
            &json!({
                "registry_id": "policy-hooks",
                "ready": true,
                "hooks": ["before_model_request", "before_tool_call"]
            }),
            "policy-hooks",
            &[
                HarnessHookId::BeforeModelRequest,
                HarnessHookId::BeforeToolCall,
            ],
        )
        .unwrap();

        let err = validate_hook_service_initialization(
            &json!({
                "registry_id": "policy-hooks",
                "ready": true,
                "hooks": ["before_model_request"]
            }),
            "policy-hooks",
            &[
                HarnessHookId::BeforeModelRequest,
                HarnessHookId::BeforeToolCall,
            ],
        )
        .unwrap_err();
        assert!(err.to_string().contains("before_tool_call"));

        let err = validate_hook_service_initialization(
            &json!({
                "registry_id": "other-hooks",
                "ready": true,
                "hooks": ["before_model_request"]
            }),
            "policy-hooks",
            &[HarnessHookId::BeforeModelRequest],
        )
        .unwrap_err();
        assert!(err.to_string().contains("expected `policy-hooks`"));

        let err = validate_hook_service_initialization(
            &json!({
                "registry_id": "policy-hooks",
                "ready": true,
                "hooks": ["before_typo"]
            }),
            "policy-hooks",
            &[HarnessHookId::BeforeModelRequest],
        )
        .unwrap_err();
        assert!(err.to_string().contains("unknown hook `before_typo`"));
    }

    #[test]
    fn process_hook_runtime_initializes_with_configured_hook_ids() {
        let temp = std::env::temp_dir().join(format!(
            "agentpm-process-hook-runtime-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&temp);
        let script = temp.join("hook_service.py");
        fs::write(
            &script,
            r#"
import json, sys
for line in sys.stdin:
    msg = json.loads(line)
    if msg["kind"] == "initialize":
        assert msg["payload"]["hooks"] == ["before_model_request", "before_tool_call"]
        kind = "initialized"
        result = {
            "registry_id": "policy-hooks",
            "ready": True,
            "hooks": ["before_model_request", "before_tool_call"]
        }
    else:
        assert msg["method"] == "before_tool_call"
        kind = "response"
        result = {
            "decision": "continue",
            "patch": {"arguments": {"query": "patched"}}
        }
    print(json.dumps({
        "protocol": "agentpm-service",
        "version": 1,
        "kind": kind,
        "id": msg.get("id"),
        "service": msg["service"],
        "result": result
    }), flush=True)
"#,
        )
        .unwrap();
        let mut implementations = HashMap::new();
        implementations.insert(
            "policy-hooks".into(),
            HarnessImplementationEntry {
                implementation: HarnessImplementation::Process {
                    command: "python3".into(),
                    args: vec![script.display().to_string()],
                    cwd: None,
                    env: Vec::new(),
                    startup_timeout_ms: 1_000,
                    request_timeout_ms: 1_000,
                    restart: crate::harness_config::HarnessRestartPolicy::default(),
                },
            },
        );
        let bindings = vec![
            HarnessHookBinding {
                hook: HarnessHookId::BeforeModelRequest,
                implementation: "policy-hooks".into(),
                failure_policy: HarnessHookFailurePolicy::Closed,
            },
            HarnessHookBinding {
                hook: HarnessHookId::BeforeToolCall,
                implementation: "policy-hooks".into(),
                failure_policy: HarnessHookFailurePolicy::Closed,
            },
        ];
        let mut runtime =
            ConfiguredHookRuntime::from_config(&temp, &bindings, &implementations, None).unwrap();

        let decision = runtime
            .before_tool_call(BeforeToolCallHook {
                phase_id: "classify".into(),
                tool: "@zack/search".into(),
                arguments: json!({ "query": "original" }),
            })
            .unwrap();

        assert_eq!(decision.arguments, Some(json!({ "query": "patched" })));
    }

    #[test]
    fn hook_reject_is_not_ignored_by_continue_failure_policy() {
        let temp = std::env::temp_dir();
        let mut implementations = HashMap::new();
        implementations.insert(
            "policy-hooks".into(),
            HarnessImplementationEntry {
                implementation: HarnessImplementation::Host {
                    request_timeout_ms: 1_000,
                },
            },
        );
        let bindings = vec![HarnessHookBinding {
            hook: HarnessHookId::BeforeToolCall,
            implementation: "policy-hooks".into(),
            failure_policy: HarnessHookFailurePolicy::Continue,
        }];
        let mut runtime =
            ConfiguredHookRuntime::from_config(&temp, &bindings, &implementations, None)
                .unwrap()
                .with_host_invoker(Box::new(FakeHostInvoker {
                    response: json!({
                        "decision": "reject",
                        "reason": "blocked by policy"
                    }),
                }));

        let err = runtime
            .before_tool_call(BeforeToolCallHook {
                phase_id: "classify".into(),
                tool: "@zack/search".into(),
                arguments: json!({ "query": "original" }),
            })
            .unwrap_err();

        assert!(err.is_rejection());
        assert_eq!(err.message, "blocked by policy");
        assert!(runtime.drain_nonfatal_failures().is_empty());
    }

    #[test]
    fn hook_invalid_patch_is_nonfatal_with_continue_failure_policy() {
        let temp = std::env::temp_dir();
        let mut implementations = HashMap::new();
        implementations.insert(
            "policy-hooks".into(),
            HarnessImplementationEntry {
                implementation: HarnessImplementation::Host {
                    request_timeout_ms: 1_000,
                },
            },
        );
        let bindings = vec![HarnessHookBinding {
            hook: HarnessHookId::BeforeToolCall,
            implementation: "policy-hooks".into(),
            failure_policy: HarnessHookFailurePolicy::Continue,
        }];
        let mut runtime =
            ConfiguredHookRuntime::from_config(&temp, &bindings, &implementations, None)
                .unwrap()
                .with_host_invoker(Box::new(FakeHostInvoker {
                    response: json!({
                        "decision": "continue",
                        "patch": {
                            "unknown": true
                        }
                    }),
                }));

        let decision = runtime
            .before_tool_call(BeforeToolCallHook {
                phase_id: "classify".into(),
                tool: "@zack/search".into(),
                arguments: json!({ "query": "original" }),
            })
            .unwrap();

        assert_eq!(decision, BeforeToolCallDecision::default());
        let failures = runtime.drain_nonfatal_failures();
        assert_eq!(failures.len(), 1);
        assert!(!failures[0].is_rejection());
        assert!(failures[0].message.contains("invalid hook patch"));
    }

    #[test]
    fn hook_invalid_patch_fails_closed_by_default() {
        let temp = std::env::temp_dir();
        let mut implementations = HashMap::new();
        implementations.insert(
            "policy-hooks".into(),
            HarnessImplementationEntry {
                implementation: HarnessImplementation::Host {
                    request_timeout_ms: 1_000,
                },
            },
        );
        let bindings = vec![HarnessHookBinding {
            hook: HarnessHookId::BeforeToolCall,
            implementation: "policy-hooks".into(),
            failure_policy: HarnessHookFailurePolicy::Closed,
        }];
        let mut runtime =
            ConfiguredHookRuntime::from_config(&temp, &bindings, &implementations, None)
                .unwrap()
                .with_host_invoker(Box::new(FakeHostInvoker {
                    response: json!({
                        "decision": "continue",
                        "patch": {
                            "unknown": true
                        }
                    }),
                }));

        let err = runtime
            .before_tool_call(BeforeToolCallHook {
                phase_id: "classify".into(),
                tool: "@zack/search".into(),
                arguments: json!({ "query": "original" }),
            })
            .unwrap_err();

        assert!(!err.is_rejection());
        assert!(err.message.contains("invalid hook patch"));
        assert!(runtime.drain_nonfatal_failures().is_empty());
    }

    #[test]
    fn tool_selection_bindings_receive_patched_candidate_snapshot() {
        let temp = std::env::temp_dir();
        let mut implementations = HashMap::new();
        for implementation in ["first-hook", "second-hook"] {
            implementations.insert(
                implementation.to_string(),
                HarnessImplementationEntry {
                    implementation: HarnessImplementation::Host {
                        request_timeout_ms: 1_000,
                    },
                },
            );
        }
        let bindings = vec![
            HarnessHookBinding {
                hook: HarnessHookId::BeforeToolSelection,
                implementation: "first-hook".into(),
                failure_policy: HarnessHookFailurePolicy::Closed,
            },
            HarnessHookBinding {
                hook: HarnessHookId::BeforeToolSelection,
                implementation: "second-hook".into(),
                failure_policy: HarnessHookFailurePolicy::Continue,
            },
        ];
        let payloads = Arc::new(Mutex::new(Vec::new()));
        let mut runtime =
            ConfiguredHookRuntime::from_config(&temp, &bindings, &implementations, None)
                .unwrap()
                .with_host_invoker(Box::new(SequenceHostInvoker {
                    responses: VecDeque::from([
                        json!({
                            "decision": "continue",
                            "patch": {
                                "candidate_ids": ["@zack/a"]
                            }
                        }),
                        json!({
                            "decision": "continue",
                            "patch": {
                                "candidate_ids": ["@zack/a", "@zack/b"]
                            }
                        }),
                    ]),
                    payloads: Arc::clone(&payloads),
                }));

        let decision = runtime
            .before_tool_selection(BeforeToolSelectionHook {
                phase: HookPhaseSnapshot {
                    phase_id: "classify".into(),
                    phase_objective: "Classify.".into(),
                    completion: CompletionContract {
                        phase_id: "classify".into(),
                        explicit_outcomes: vec!["draft".into()],
                        implicit_complete: false,
                    },
                },
                candidates: vec![
                    BeforeToolSelectionCandidate {
                        canonical_id: "@zack/a".into(),
                        description: "A".into(),
                        source: "agent_binding".into(),
                    },
                    BeforeToolSelectionCandidate {
                        canonical_id: "@zack/b".into(),
                        description: "B".into(),
                        source: "agent_binding".into(),
                    },
                ],
            })
            .unwrap();

        assert_eq!(decision.candidate_ids, Some(vec!["@zack/a".into()]));
        let payloads = payloads.lock().unwrap();
        assert_eq!(
            payloads[0]["input"]["candidates"].as_array().unwrap().len(),
            2
        );
        assert_eq!(
            payloads[1]["input"]["candidates"].as_array().unwrap().len(),
            1
        );
        let failures = runtime.drain_nonfatal_failures();
        assert_eq!(failures.len(), 1);
        assert!(failures[0].message.contains("introduced Tool `@zack/b`"));
    }

    #[test]
    fn tool_call_bindings_receive_patched_arguments_snapshot() {
        let temp = std::env::temp_dir();
        let mut implementations = HashMap::new();
        for implementation in ["first-hook", "second-hook"] {
            implementations.insert(
                implementation.to_string(),
                HarnessImplementationEntry {
                    implementation: HarnessImplementation::Host {
                        request_timeout_ms: 1_000,
                    },
                },
            );
        }
        let bindings = vec![
            HarnessHookBinding {
                hook: HarnessHookId::BeforeToolCall,
                implementation: "first-hook".into(),
                failure_policy: HarnessHookFailurePolicy::Closed,
            },
            HarnessHookBinding {
                hook: HarnessHookId::BeforeToolCall,
                implementation: "second-hook".into(),
                failure_policy: HarnessHookFailurePolicy::Closed,
            },
        ];
        let payloads = Arc::new(Mutex::new(Vec::new()));
        let mut runtime =
            ConfiguredHookRuntime::from_config(&temp, &bindings, &implementations, None)
                .unwrap()
                .with_host_invoker(Box::new(SequenceHostInvoker {
                    responses: VecDeque::from([
                        json!({
                            "decision": "continue",
                            "patch": {
                                "arguments": {
                                    "query": "first"
                                }
                            }
                        }),
                        json!({
                            "decision": "continue",
                            "patch": {
                                "arguments": {
                                    "query": "second"
                                }
                            }
                        }),
                    ]),
                    payloads: Arc::clone(&payloads),
                }));

        let decision = runtime
            .before_tool_call(BeforeToolCallHook {
                phase_id: "classify".into(),
                tool: "@zack/search".into(),
                arguments: json!({ "query": "original" }),
            })
            .unwrap();

        assert_eq!(decision.arguments, Some(json!({ "query": "second" })));
        let payloads = payloads.lock().unwrap();
        assert_eq!(
            payloads[0]["input"]["arguments"]["query"],
            json!("original")
        );
        assert_eq!(payloads[1]["input"]["arguments"]["query"], json!("first"));
    }

    #[test]
    fn hook_response_requires_explicit_decision() {
        let err = decode_hook_response::<BeforeToolCallDecision>(
            HarnessHookId::BeforeToolCall,
            json!({
                "patch": {
                    "arguments": {
                        "query": "patched"
                    }
                }
            }),
        )
        .unwrap_err();

        assert!(!err.is_rejection());
        assert!(err.message.contains("missing required decision"));
    }

    #[test]
    fn explicit_continue_decision_decodes_empty_patch() {
        let decision = decode_hook_response::<BeforeToolCallDecision>(
            HarnessHookId::BeforeToolCall,
            json!({
                "decision": "continue"
            }),
        )
        .unwrap()
        .unwrap();

        assert_eq!(decision, BeforeToolCallDecision::default());
    }

    #[test]
    fn before_tool_selection_hook_input_contains_only_tool_candidates() {
        let phase = EffectivePhase {
            phase_id: "classify".into(),
            tools_allowed: Some(true),
            knowledge_allowed: None,
            memory_read_allowed: None,
            memory_write_allowed: None,
            authored_profile_candidates: Vec::new(),
            active_profiles: Vec::new(),
            active_tools: vec![tool("@zack/a")],
            active_skills: Vec::new(),
            capability_catalog: vec![
                descriptor("phase_completion", "classify/completion"),
                descriptor("agentpm_tool", "@zack/a"),
                descriptor("skill_resource_read", "@zack/skill"),
            ],
            suppressed_capabilities: Vec::new(),
        };
        let hook = before_tool_selection_hook_from_phase(
            "classify",
            "Classify.",
            CompletionContract {
                phase_id: "classify".into(),
                explicit_outcomes: vec!["draft".into()],
                implicit_complete: false,
            },
            &phase,
        );

        assert_eq!(hook.phase.phase_id, "classify");
        assert_eq!(hook.candidates.len(), 1);
        assert_eq!(hook.candidates[0].canonical_id, "@zack/a");
    }

    #[test]
    fn before_model_request_rejects_unknown_patch_fields() {
        let err = serde_json::from_value::<BeforeModelRequestDecision>(json!({
            "phase_id": "other",
        }))
        .unwrap_err();
        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn before_tool_call_rejects_unknown_patch_fields() {
        let err = serde_json::from_value::<BeforeToolCallDecision>(json!({
            "tool": "@zack/other",
        }))
        .unwrap_err();
        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn before_model_request_rejects_provider_options_without_model() {
        let mut request = minimal_model_request();
        request.model = None;
        request.runtime.model = None;
        let mut provider_options = Map::new();
        provider_options.insert("temperature".into(), json!(0.2));

        let err = apply_before_model_request_decision(
            &mut request,
            BeforeModelRequestDecision {
                provider_options,
                ..BeforeModelRequestDecision::default()
            },
        )
        .unwrap_err();

        assert!(err.contains("without a selected model"));
    }

    #[test]
    fn before_model_request_appends_context_only_to_mutable_context_section() {
        let mut request = minimal_model_request();
        apply_before_model_request_decision(
            &mut request,
            BeforeModelRequestDecision {
                context_sections: vec![BeforeModelRequestContextSection {
                    title: "Extra".into(),
                    content: "Additional safe context.".into(),
                }],
                ..BeforeModelRequestDecision::default()
            },
        )
        .unwrap();

        let control = request
            .prompt
            .sections
            .iter()
            .find(|section| section.title == "HARNESS CONTROL")
            .unwrap();
        let context = request
            .prompt
            .sections
            .iter()
            .find(|section| section.title == CONSUMER_RUN_CONTEXT_SECTION_TITLE)
            .unwrap();
        assert!(!control.content.contains("Additional safe context."));
        assert!(context.content.contains("Additional safe context."));
    }

    #[test]
    fn tool_selection_can_subset_and_reorder_only_existing_tools() {
        let mut phase = EffectivePhase {
            phase_id: "classify".into(),
            tools_allowed: Some(true),
            knowledge_allowed: None,
            memory_read_allowed: None,
            memory_write_allowed: None,
            authored_profile_candidates: Vec::new(),
            active_profiles: Vec::new(),
            active_tools: vec![tool("@zack/a"), tool("@zack/b")],
            active_skills: Vec::new(),
            capability_catalog: vec![
                descriptor("phase_completion", "classify/completion"),
                descriptor("agentpm_tool", "@zack/a"),
                descriptor("agentpm_tool", "@zack/b"),
            ],
            suppressed_capabilities: Vec::new(),
        };
        apply_before_tool_selection_decision(
            &mut phase,
            BeforeToolSelectionDecision {
                candidate_ids: Some(vec!["@zack/b".into()]),
            },
        )
        .unwrap();
        assert_eq!(phase.active_tools[0].name, "@zack/b");
        assert!(phase.active_tools.iter().all(|tool| tool.name != "@zack/a"));
    }

    #[test]
    fn tool_selection_rejects_added_or_duplicate_tools() {
        let phase = EffectivePhase {
            phase_id: "classify".into(),
            tools_allowed: Some(true),
            knowledge_allowed: None,
            memory_read_allowed: None,
            memory_write_allowed: None,
            authored_profile_candidates: Vec::new(),
            active_profiles: Vec::new(),
            active_tools: vec![tool("@zack/a")],
            active_skills: Vec::new(),
            capability_catalog: vec![descriptor("agentpm_tool", "@zack/a")],
            suppressed_capabilities: Vec::new(),
        };

        let mut introduced = phase.clone();
        let err = apply_before_tool_selection_decision(
            &mut introduced,
            BeforeToolSelectionDecision {
                candidate_ids: Some(vec!["@zack/new".into()]),
            },
        )
        .unwrap_err();
        assert!(err.contains("introduced Tool `@zack/new`"));

        let mut duplicated = phase;
        let err = apply_before_tool_selection_decision(
            &mut duplicated,
            BeforeToolSelectionDecision {
                candidate_ids: Some(vec!["@zack/a".into(), "@zack/a".into()]),
            },
        )
        .unwrap_err();
        assert!(err.contains("duplicated Tool `@zack/a`"));
    }

    fn descriptor(kind: &str, identity: &str) -> CapabilityDescriptor {
        CapabilityDescriptor {
            action_kind: kind.into(),
            identity: identity.into(),
            description: "desc".into(),
            source: "test".into(),
        }
    }

    fn tool(name: &str) -> super::super::model::ToolRuntimeSnapshot {
        super::super::model::ToolRuntimeSnapshot {
            name: name.into(),
            version: "0.1.0".into(),
            description: "desc".into(),
            root: None,
            input_schema: json!({ "type": "object" }),
            state: "available".into(),
            source: "test".into(),
        }
    }

    fn minimal_model_request() -> ModelRequest {
        let selection = ModelProviderSelection {
            provider: "test-provider".into(),
            model: "test-model".into(),
            options: json!({}),
        };
        ModelRequest {
            runtime: super::super::model::RuntimeSnapshot {
                session_id: "session".into(),
                workspace_root: Default::default(),
                state_dir: Default::default(),
                agent: None,
                loop_package: None,
                package_graph: Vec::new(),
                runtime_config_sources: Default::default(),
                runtime_scopes: Default::default(),
                consumer_context: None,
                services: Vec::new(),
                hook_registrations: Vec::new(),
                profiles: Vec::new(),
                profile_bindings: Default::default(),
                tools: Vec::new(),
                skills: Vec::new(),
                capability_candidates: Vec::new(),
                model: Some(selection.clone()),
            },
            model: Some(selection),
            prompt: super::super::model::LogicalPrompt {
                sections: vec![
                    PromptSection {
                        number: 1,
                        title: "HARNESS CONTROL".into(),
                        content: "control".into(),
                    },
                    PromptSection {
                        number: 3,
                        title: CONSUMER_RUN_CONTEXT_SECTION_TITLE.into(),
                        content: "context".into(),
                    },
                ],
                action_aliases: Vec::new(),
                completion: CompletionContract {
                    phase_id: "assess".into(),
                    explicit_outcomes: vec!["done".into()],
                    implicit_complete: false,
                },
                diagnostics: Vec::new(),
            },
            run_id: "run".into(),
            phase_execution_id: "phase-exec-1".into(),
            phase_id: "assess".into(),
            phase_objective: "Assess.".into(),
            run_input: "input".into(),
            prior_phase_results: Vec::new(),
            transcript: Vec::new(),
            effective_phase: EffectivePhase {
                phase_id: "assess".into(),
                tools_allowed: None,
                knowledge_allowed: None,
                memory_read_allowed: None,
                memory_write_allowed: None,
                authored_profile_candidates: Vec::new(),
                active_profiles: Vec::new(),
                active_tools: Vec::new(),
                active_skills: Vec::new(),
                capability_catalog: Vec::new(),
                suppressed_capabilities: Vec::new(),
            },
            repair_feedback: None,
        }
    }
}
