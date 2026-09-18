#![allow(dead_code)]

use super::action::{MemoryReadMode, MemoryWriteOperation, SemanticAction, SemanticActionProposal};
use super::model::{
    ActionAlias, CapabilityDescriptor, ModelCapabilityAdvertisement, ModelProviderSelection,
    ModelRequest, ModelRequestTurn, ModelRuntime, ModelRuntimeFailure, ModelTurn,
    memory_content_filter_path_enumeration, memory_filter_shape_supported,
};
use super::service::{ProcessServiceClient, ProcessServiceConfig, ServiceLifecycleEmitter};
use crate::harness_config::HarnessImplementation;
use crate::harness_observability::RunUsage;
use crate::manifest::MemoryRetrievalMode;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderRequest {
    pub selection: ModelProviderSelection,
    pub prompt: String,
    pub include_capability_catalog: bool,
    pub turn_strategy: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub turns: Vec<ModelRequestTurn>,
    // Provider-safe alias -> canonical Harness identity. Provider adapters use
    // aliases in native tool/function definitions and map calls back here.
    pub action_aliases: BTreeMap<String, String>,
    pub actions: Vec<ProviderActionTool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderActionTool {
    pub alias: String,
    pub action_kind: String,
    pub identity: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderActionCall {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub alias: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderResponse {
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_calls: Vec<ProviderActionCall>,
    pub usage: RunUsage,
    pub finish_reason: Option<String>,
    pub metadata: BTreeMap<String, Value>,
}

pub trait ModelProviderTransport {
    fn send(&mut self, request: ProviderRequest) -> Result<ProviderResponse, ModelRuntimeFailure>;
}

pub struct BuiltInModelRuntime {
    selection: ModelProviderSelection,
    capabilities: ModelCapabilityAdvertisement,
    transport: Box<dyn ModelProviderTransport>,
}

pub struct ProcessModelRuntime {
    selection: ModelProviderSelection,
    capabilities: ModelCapabilityAdvertisement,
    client: ProcessServiceClient,
}

impl BuiltInModelRuntime {
    pub fn new(
        selection: ModelProviderSelection,
        transport: Box<dyn ModelProviderTransport>,
    ) -> Self {
        let capabilities = built_in_capabilities(&selection);
        Self {
            selection,
            capabilities,
            transport,
        }
    }

    pub fn from_selection(selection: ModelProviderSelection) -> Result<Self, ModelRuntimeFailure> {
        let transport: Box<dyn ModelProviderTransport> = match selection.provider.as_str() {
            "openai" => Box::new(OpenAiTransport::default()),
            "anthropic" => Box::new(AnthropicTransport::default()),
            "ollama" => Box::new(OllamaTransport::default()),
            other => {
                return Err(ModelRuntimeFailure::new(format!(
                    "model provider `{other}` is not a built-in provider; custom provider transport becomes live in a later milestone"
                )));
            }
        };
        Ok(Self::new(selection, transport))
    }
}

impl ProcessModelRuntime {
    pub fn start(
        selection: ModelProviderSelection,
        implementation: HarnessImplementation,
        workspace_root: PathBuf,
        lifecycle_events: Option<ServiceLifecycleEmitter>,
    ) -> Result<Self, ModelRuntimeFailure> {
        let mut initialize_payload = Map::new();
        initialize_payload.insert("model".into(), json!(selection.model.clone()));
        let client = ProcessServiceClient::start(ProcessServiceConfig {
            service: "model".into(),
            registry_id: selection.provider.clone(),
            initialize_payload,
            implementation,
            workspace_root,
            lifecycle_events,
        })
        .map_err(|err| ModelRuntimeFailure::new(format!("model service failed to start: {err}")))?;
        let capabilities = process_model_capabilities_from_initialization(
            client.initialization_result(),
            &selection.provider,
            &selection.model,
        )?;
        Ok(Self {
            selection,
            capabilities,
            client,
        })
    }
}

impl ModelRuntime for ProcessModelRuntime {
    fn capabilities(&self) -> ModelCapabilityAdvertisement {
        self.capabilities.clone()
    }

    fn inspect_request(
        &self,
        request: &ModelRequest,
    ) -> Option<super::model::ModelRuntimeRequestSnapshot> {
        let selection = request
            .model
            .clone()
            .unwrap_or_else(|| self.selection.clone());
        let prompt = request.prompt.render_text();
        Some(super::model::ModelRuntimeRequestSnapshot {
            runtime_kind: "process".into(),
            request_kind: "canonical_model_request".into(),
            provider: selection.provider,
            model: selection.model,
            action_descriptors: request.prompt.action_aliases.len(),
            structured_actions: None,
            capability_catalog_in_prompt: request.prompt.has_capability_catalog_section(),
            action_aliases: request.prompt.action_aliases.clone(),
            turn_strategy: "canonical_request".into(),
            ordered_turns: request.ordered_turns.clone(),
            diagnostics: request.prompt.diagnostics.clone(),
            prompt,
        })
    }

    fn generate(&mut self, request: ModelRequest) -> Result<ModelTurn, ModelRuntimeFailure> {
        let selection = request
            .model
            .clone()
            .unwrap_or_else(|| self.selection.clone());
        let payload = json!({
            "selection": selection,
            "request": request,
        });
        let response = self.client.request("generate", payload).map_err(|err| {
            ModelRuntimeFailure::new(format!("model service generate failed: {err}"))
        })?;
        serde_json::from_value(response).map_err(|err| {
            ModelRuntimeFailure::new(format!("model service returned invalid ModelTurn: {err}"))
        })
    }
}

pub(crate) fn built_in_capabilities(
    selection: &ModelProviderSelection,
) -> ModelCapabilityAdvertisement {
    let model = selection.model.to_ascii_lowercase();
    ModelCapabilityAdvertisement {
        context_window_tokens: match selection.provider.as_str() {
            "openai" => openai_context_window(&model),
            "anthropic" => anthropic_context_window(&model),
            "ollama" => ollama_context_window(&model),
            _ => None,
        },
        ..ModelCapabilityAdvertisement::default()
    }
}

#[derive(Default, Deserialize)]
struct PartialModelCapabilityAdvertisement {
    semantic_actions: Option<bool>,
    structured_output: Option<bool>,
    multimodal_input: Option<bool>,
    context_window_tokens: Option<u64>,
    usage_reporting: Option<bool>,
}

fn process_model_capabilities_from_initialization(
    initialization_result: &Value,
    expected_registry_id: &str,
    expected_model: &str,
) -> Result<ModelCapabilityAdvertisement, ModelRuntimeFailure> {
    if initialization_result
        .get("ready")
        .and_then(Value::as_bool)
        .is_some_and(|ready| !ready)
    {
        return Err(ModelRuntimeFailure::new(format!(
            "model service `{expected_registry_id}` initialized but reported not ready"
        )));
    }
    if let Some(registry_id) = initialization_result
        .get("registry_id")
        .and_then(Value::as_str)
        && registry_id != expected_registry_id
    {
        return Err(ModelRuntimeFailure::new(format!(
            "model service initialized as `{registry_id}`, expected `{expected_registry_id}`"
        )));
    }
    if let Some(model) = initialization_result.get("model").and_then(Value::as_str)
        && model != expected_model
    {
        return Err(ModelRuntimeFailure::new(format!(
            "model service initialized model `{model}`, expected `{expected_model}`"
        )));
    }
    let capabilities_value = initialization_result
        .get("capabilities")
        .unwrap_or(initialization_result);
    if capabilities_value.is_null() {
        return Ok(ModelCapabilityAdvertisement::default());
    }
    let partial: PartialModelCapabilityAdvertisement =
        serde_json::from_value(capabilities_value.clone()).map_err(|err| {
            ModelRuntimeFailure::new(format!("invalid model service capabilities: {err}"))
        })?;
    let mut capabilities = ModelCapabilityAdvertisement::default();
    if let Some(semantic_actions) = partial.semantic_actions {
        capabilities.semantic_actions = semantic_actions;
    }
    if let Some(structured_output) = partial.structured_output {
        capabilities.structured_output = structured_output;
    }
    if let Some(multimodal_input) = partial.multimodal_input {
        capabilities.multimodal_input = multimodal_input;
    }
    if let Some(context_window_tokens) = partial.context_window_tokens {
        capabilities.context_window_tokens = Some(context_window_tokens);
    }
    if let Some(usage_reporting) = partial.usage_reporting {
        capabilities.usage_reporting = usage_reporting;
    }
    Ok(capabilities)
}

fn openai_context_window(model: &str) -> Option<u64> {
    if model.contains("gpt-4.1") || model.contains("gpt-5") {
        Some(1_000_000)
    } else if model.contains("gpt-4o") || model.contains("o3") || model.contains("o4") {
        Some(128_000)
    } else if model.contains("gpt-4") {
        Some(8_192)
    } else {
        None
    }
}

fn anthropic_context_window(model: &str) -> Option<u64> {
    if model.contains("claude") {
        Some(200_000)
    } else {
        None
    }
}

fn ollama_context_window(model: &str) -> Option<u64> {
    if model.contains("llama3.1") || model.contains("llama3.2") || model.contains("llama3.3") {
        Some(128_000)
    } else if model.contains("llama3") || model.contains("mistral") {
        Some(8_192)
    } else {
        None
    }
}

impl ModelRuntime for BuiltInModelRuntime {
    fn capabilities(&self) -> ModelCapabilityAdvertisement {
        self.capabilities.clone()
    }

    fn inspect_request(
        &self,
        request: &ModelRequest,
    ) -> Option<super::model::ModelRuntimeRequestSnapshot> {
        let provider_request = self.provider_request(request);
        let capability_catalog_in_prompt = provider_request.include_capability_catalog
            && request.prompt.has_capability_catalog_section();
        let mut diagnostics = request.prompt.diagnostics.clone();
        diagnostics.extend(provider_schema_fallback_diagnostics(request));
        Some(super::model::ModelRuntimeRequestSnapshot {
            runtime_kind: "built_in".into(),
            request_kind: "provider_wire_request".into(),
            provider: provider_request.selection.provider,
            model: provider_request.selection.model,
            action_descriptors: request.prompt.action_aliases.len(),
            structured_actions: Some(provider_request.actions.len()),
            capability_catalog_in_prompt,
            action_aliases: request.prompt.action_aliases.clone(),
            turn_strategy: provider_request.turn_strategy,
            ordered_turns: provider_request.turns,
            diagnostics,
            prompt: provider_request.prompt,
        })
    }

    fn generate(&mut self, request: ModelRequest) -> Result<ModelTurn, ModelRuntimeFailure> {
        if !self.capabilities.semantic_actions || !self.capabilities.structured_output {
            return Err(ModelRuntimeFailure::new(
                "selected model runtime does not advertise required Harness semantic action support",
            ));
        }
        let provider_request = self.provider_request(&request);
        let response = self.transport.send(provider_request)?;
        normalize_provider_response(response, &request.prompt.action_aliases)
    }
}

impl BuiltInModelRuntime {
    fn provider_request(&self, request: &ModelRequest) -> ProviderRequest {
        let selection = request
            .model
            .clone()
            .unwrap_or_else(|| self.selection.clone());
        let actions = provider_action_tools(request);
        let include_capability_catalog = actions.is_empty();
        let native_turns_available = !actions.is_empty();
        let turn_strategy = if native_turns_available {
            "native_action_result_turns"
        } else {
            "prompt_text_fallback"
        };
        let prompt = if native_turns_available {
            request
                .prompt
                .render_provider_text_with_native_turns(include_capability_catalog)
        } else {
            request
                .prompt
                .render_provider_text(include_capability_catalog)
        };
        ProviderRequest {
            selection,
            prompt,
            action_aliases: request
                .prompt
                .action_aliases
                .iter()
                .map(|alias| (alias.alias.clone(), alias.identity.clone()))
                .collect(),
            actions,
            include_capability_catalog,
            turn_strategy: turn_strategy.into(),
            turns: if native_turns_available {
                if request.ordered_turns.is_empty() {
                    vec![ModelRequestTurn::UserInput {
                        content: request.run_input.clone(),
                    }]
                } else {
                    request.ordered_turns.clone()
                }
            } else {
                Vec::new()
            },
        }
    }
}

fn provider_action_tools(request: &ModelRequest) -> Vec<ProviderActionTool> {
    request
        .prompt
        .action_aliases
        .iter()
        .filter_map(|alias| {
            let descriptor =
                request
                    .effective_phase
                    .capability_catalog
                    .iter()
                    .find(|descriptor| {
                        descriptor.action_kind == alias.action_kind
                            && descriptor.identity == alias.identity
                    })?;
            Some(ProviderActionTool {
                alias: alias.alias.clone(),
                action_kind: alias.action_kind.clone(),
                identity: alias.identity.clone(),
                description: provider_action_description(alias, descriptor, request),
                parameters: action_parameters_schema(alias, request),
            })
        })
        .collect()
}

fn provider_schema_fallback_diagnostics(request: &ModelRequest) -> Vec<String> {
    let mut diagnostics = BTreeSet::new();
    for memory in &request.effective_phase.active_memory {
        if memory
            .retrieval_modes
            .iter()
            .any(|mode| matches!(mode, MemoryRetrievalMode::Filter))
            && !memory_filter_shape_supported(memory)
            && request
                .effective_phase
                .capability_catalog
                .iter()
                .any(|descriptor| {
                    descriptor.action_kind == "memory_read"
                        && descriptor.identity == memory_identity(&memory.package, &memory.space)
                })
        {
            diagnostics.insert(format!(
                "provider-facing schema omission: Memory read `{}` filter mode has no declared content filter paths; filter action is not advertised.",
                memory_identity(&memory.package, &memory.space)
            ));
        }
    }
    for alias in &request.prompt.action_aliases {
        match alias.action_kind.as_str() {
            "agentpm_tool" => {
                if !request
                    .runtime
                    .tools
                    .iter()
                    .any(|tool| tool.name == alias.identity)
                {
                    diagnostics.insert(format!(
                        "provider-facing schema fallback for `{}`: Tool `{}` has no runtime input_schema metadata; arguments are advertised as an open object.",
                        alias.alias, alias.identity
                    ));
                }
            }
            "knowledge_request" => {
                let knowledge = request
                    .effective_phase
                    .active_knowledge
                    .iter()
                    .find(|knowledge| knowledge.name == alias.identity)
                    .or_else(|| {
                        request
                            .runtime
                            .knowledge
                            .iter()
                            .find(|knowledge| knowledge.name == alias.identity)
                    });
                match knowledge {
                    Some(knowledge)
                        if knowledge.mode == "context" && knowledge.documents.is_empty() =>
                    {
                        diagnostics.insert(format!(
                            "provider-facing schema fallback for `{}`: Knowledge package `{}` has no declared document metadata; document is advertised as an open string.",
                            alias.alias, alias.identity
                        ));
                    }
                    None => {
                        diagnostics.insert(format!(
                            "provider-facing schema fallback for `{}`: Knowledge package `{}` has no runtime metadata; request mode is advertised as an open fallback.",
                            alias.alias, alias.identity
                        ));
                    }
                    _ => {}
                }
            }
            "memory_write" => {
                let memory = request.effective_phase.active_memory.iter().find(|memory| {
                    memory_identity(&memory.package, &memory.space) == alias.identity
                });
                if memory.is_none_or(|memory| memory.record_types.is_empty()) {
                    diagnostics.insert(format!(
                        "provider-facing schema fallback for `{}`: Memory write `{}` has no record-type metadata; content is advertised as an open object.",
                        alias.alias, alias.identity
                    ));
                }
            }
            "memory_read" if memory_read_filter_paths_incomplete(alias, request) => {
                diagnostics.insert(format!(
                    "provider-facing schema fallback for `{}`: Memory read `{}` has recursive or oversized content filter paths; filter is advertised as an open object and Harness validation remains authoritative.",
                    alias.alias, alias.identity
                ));
            }
            "external_mcp_tool" => {
                if let Some(tool) = request
                    .effective_phase
                    .active_mcp_tools
                    .iter()
                    .find(|tool| tool.identity == alias.identity)
                {
                    match mcp_provider_schema_degradation(&tool.input_schema) {
                        Some(McpProviderSchemaDegradation::OpenFallback) => {
                            diagnostics.insert(format!(
                                "provider-facing schema fallback for `{}`: MCP Tool `{}` input schema uses unsupported composition or references that cannot be represented for the selected provider; arguments are advertised as an open object and Harness validates against the canonical MCP schema.",
                                alias.alias, alias.identity
                            ));
                        }
                        Some(McpProviderSchemaDegradation::Reduced) => {
                            diagnostics.insert(format!(
                                "provider-facing schema fallback for `{}`: MCP Tool `{}` input schema was reduced for provider compatibility; Harness validates against the canonical MCP schema.",
                                alias.alias, alias.identity
                            ));
                        }
                        None => {}
                    }
                }
            }
            _ => {}
        }
    }
    diagnostics.into_iter().collect()
}

fn provider_action_description(
    alias: &ActionAlias,
    descriptor: &super::model::CapabilityDescriptor,
    request: &ModelRequest,
) -> String {
    match alias.action_kind.as_str() {
        "phase_completion" => format!(
            "{} Use this action when the phase objective is satisfied. Other available actions do not need to be called just because they remain available.",
            descriptor.description
        ),
        "persistence_review_complete" => {
            "Complete the bounded persistence review when no additional authorized Memory read or write is needed. This does not change the pending phase outcome.".into()
        }
        "memory_read" | "memory_write" => {
            let descriptor_description =
                provider_memory_descriptor_description(alias, descriptor, request);
            let fixed_target = request
                .effective_phase
                .active_memory
                .iter()
                .find(|memory| memory_identity(&memory.package, &memory.space) == alias.identity)
                .map(|memory| {
                    let fixed_record_type = alias
                        .provider_shape
                        .as_deref()
                        .and_then(|provider_shape| {
                            memory_write_provider_shape(provider_shape)
                                .and_then(|shape| shape.record_type())
                        })
                        .or_else(|| {
                            memory
                                .record_types
                                .as_slice()
                                .first()
                                .filter(|_| memory.record_types.len() == 1)
                                .map(|record_type| record_type.name.as_str())
                        });
                    let record_type = if let Some(record_type) = fixed_record_type {
                        format!(", fixed record_type `{record_type}`")
                    } else {
                        ", record_type must be selected from this action's schema".into()
                    };
                    format!(
                        "Fixed Memory target: package `{}`, space `{}`{}.",
                        memory.package, memory.space, record_type
                    )
                })
                .unwrap_or_else(|| format!("Fixed Memory target identity: `{}`.", alias.identity));
            let read_shape = if alias.action_kind == "memory_read" {
                alias.provider_shape
                    .as_deref()
                    .and_then(|provider_shape| {
                        memory_read_shape_description(
                            provider_shape,
                            memory_filter_shape_supported_for_alias(alias, request),
                        )
                    })
                    .map(|description| format!(" Fixed read shape: {description}."))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            let write_shape = if alias.action_kind == "memory_write" {
                alias
                    .provider_shape
                    .as_deref()
                    .and_then(memory_write_shape_description)
                    .map(|description| format!(" Fixed write shape: {description}."))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            format!(
                "{} {}{}{} Choose this action only when that fixed target exactly matches the intended durable Memory surface.",
                descriptor_description, fixed_target, read_shape, write_shape
            )
        }
        "knowledge_request" => format!(
            "{} Fixed Knowledge target: package `{}`. Do not put a different package identity in the arguments.",
            descriptor.description, alias.identity
        ),
        "skill_resource_read" => format!(
            "{} Fixed Skill target: `{}`. Select only one declared resource id in the arguments.",
            descriptor.description, alias.identity
        ),
        "agentpm_tool" => format!(
            "{} Fixed AgentPM Tool target: `{}`. Put only this Tool's JSON arguments in the arguments field.",
            descriptor.description, alias.identity
        ),
        "external_mcp_tool" => format!(
            "{} Fixed MCP Tool target: `{}`. Put only this MCP Tool's JSON arguments in the arguments field.",
            descriptor.description, alias.identity
        ),
        _ => descriptor.description.clone(),
    }
}

fn provider_memory_descriptor_description(
    alias: &super::model::ActionAlias,
    descriptor: &CapabilityDescriptor,
    request: &ModelRequest,
) -> String {
    if !matches!(alias.action_kind.as_str(), "memory_read" | "memory_write")
        || alias.provider_shape.is_none()
    {
        return descriptor.description.clone();
    }
    request
        .effective_phase
        .active_memory
        .iter()
        .find(|memory| memory_identity(&memory.package, &memory.space) == alias.identity)
        .map(|memory| memory.description.clone())
        .unwrap_or_else(|| descriptor.description.clone())
}

fn action_parameters_schema(alias: &super::model::ActionAlias, request: &ModelRequest) -> Value {
    match alias.action_kind.as_str() {
        "phase_completion" => phase_completion_parameters_schema(request),
        "persistence_review_complete" => json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {}
        }),
        "agentpm_tool" => agentpm_tool_parameters_schema(alias, request),
        "external_mcp_tool" => external_mcp_tool_parameters_schema(alias, request),
        "skill_resource_read" => skill_resource_read_parameters_schema(alias, request),
        "knowledge_request" => knowledge_request_parameters_schema(alias, request),
        "memory_read" => memory_read_parameters_schema(alias, request),
        "memory_write" => memory_write_parameters_schema(alias, request),
        _ => json!({
            "type": "object",
            "additionalProperties": true
        }),
    }
}

fn memory_read_parameters_schema(
    alias: &super::model::ActionAlias,
    request: &ModelRequest,
) -> Value {
    if let Some(provider_shape) = alias.provider_shape.as_deref() {
        return memory_read_shape_parameters_schema(provider_shape, alias, request);
    }

    let modes = request
        .effective_phase
        .active_memory
        .iter()
        .find(|memory| memory_identity(&memory.package, &memory.space) == alias.identity)
        .map(|memory| {
            memory
                .retrieval_modes
                .iter()
                .map(memory_retrieval_mode_label)
                .collect::<Vec<_>>()
        })
        .filter(|modes| !modes.is_empty())
        .unwrap_or_else(|| {
            vec![
                "key".into(),
                "filter".into(),
                "chronological".into(),
                "full_text".into(),
            ]
        });
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "mode": {
                "type": "string",
                "enum": modes,
                "description": "Memory retrieval mode for this already-selected package/space."
            },
            "record_id": {
                "type": "string",
                "minLength": 1,
                "description": "Existing Memory record id returned by authorized Memory access."
            },
            "record_type": memory_record_type_schema(alias, request),
            "filter": {
                "type": "object",
                "additionalProperties": true,
                "description": "Conjunctive exact-match filter using dot-path keys over durable record content."
            },
            "query": {
                "type": "string",
                "minLength": 1,
                "description": "Text query for full_text or semantic Memory retrieval."
            },
            "limit": { "type": "integer", "minimum": 1 }
        },
        "required": ["mode"]
    })
}

fn memory_read_shape_parameters_schema(
    provider_shape: &str,
    alias: &super::model::ActionAlias,
    request: &ModelRequest,
) -> Value {
    let record_type = memory_record_type_schema(alias, request);
    match provider_shape {
        "key_document" => json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "record_type": record_type
            }
        }),
        "key_record" => json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "record_id": memory_record_id_schema(),
                "record_type": record_type
            },
            "required": ["record_id"]
        }),
        "chronological" => json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "record_type": record_type,
                "limit": memory_limit_schema()
            }
        }),
        "filter" => json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "record_type": record_type,
                "filter": memory_filter_schema(alias, request),
                "limit": memory_limit_schema()
            },
            "required": ["filter"]
        }),
        "full_text" => json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "record_type": record_type,
                "query": memory_query_schema("Text query for full_text Memory retrieval."),
                "limit": memory_limit_schema()
            },
            "required": ["query"]
        }),
        "semantic" => {
            let mut properties = Map::from_iter([
                ("record_type".into(), record_type),
                (
                    "query".into(),
                    memory_query_schema("Text query for semantic Memory retrieval."),
                ),
                ("limit".into(), memory_limit_schema()),
            ]);
            if memory_filter_shape_supported_for_alias(alias, request) {
                properties.insert("filter".into(), memory_filter_schema(alias, request));
            }
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": properties,
                "required": ["query"]
            })
        }
        _ => json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "mode": {
                    "type": "string",
                    "enum": ["key", "filter", "chronological", "full_text", "semantic"],
                    "description": "Memory retrieval mode for this already-selected package/space."
                },
                "record_id": memory_record_id_schema(),
                "record_type": record_type,
                "filter": memory_filter_schema(alias, request),
                "query": memory_query_schema("Text query for full_text or semantic Memory retrieval."),
                "limit": memory_limit_schema()
            },
            "required": ["mode"]
        }),
    }
}

fn memory_record_id_schema() -> Value {
    json!({
        "type": "string",
        "minLength": 1,
        "description": "Existing Memory record id returned by authorized Memory access."
    })
}

fn memory_filter_schema(alias: &super::model::ActionAlias, request: &ModelRequest) -> Value {
    let Some(memory) = request
        .effective_phase
        .active_memory
        .iter()
        .find(|memory| memory_identity(&memory.package, &memory.space) == alias.identity)
    else {
        return memory_open_filter_schema(
            "Conjunctive exact-match filter using dot-path keys over durable record content.",
        );
    };
    let enumerations = memory
        .record_types
        .iter()
        .map(|record_type| memory_content_filter_path_enumeration(&record_type.content_schema))
        .collect::<Vec<_>>();
    if enumerations.iter().any(|enumeration| !enumeration.complete) {
        return memory_open_filter_schema(
            "Conjunctive exact-match filter using dot-path keys over durable record content. Declared filter paths are recursive or too large to enumerate in provider-facing schema; Harness validation remains authoritative.",
        );
    }
    let mut paths = enumerations
        .into_iter()
        .flat_map(|enumeration| enumeration.paths)
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    let properties = paths
        .into_iter()
        .map(|path| {
            (
                path,
                json!({
                    "description": "Exact value to match at this declared durable content path."
                }),
            )
        })
        .collect::<Map<_, _>>();
    json!({
        "type": "object",
        "additionalProperties": false,
        "minProperties": 1,
        "properties": properties,
        "description": "Conjunctive exact-match filter using dot-path keys over durable record content."
    })
}

fn memory_open_filter_schema(description: &str) -> Value {
    json!({
        "type": "object",
        "additionalProperties": true,
        "minProperties": 1,
        "description": description
    })
}

fn memory_filter_shape_supported_for_alias(
    alias: &super::model::ActionAlias,
    request: &ModelRequest,
) -> bool {
    request
        .effective_phase
        .active_memory
        .iter()
        .find(|memory| memory_identity(&memory.package, &memory.space) == alias.identity)
        .is_none_or(memory_filter_shape_supported)
}

fn memory_read_filter_paths_incomplete(
    alias: &super::model::ActionAlias,
    request: &ModelRequest,
) -> bool {
    if !matches!(
        alias.provider_shape.as_deref(),
        Some("filter") | Some("semantic")
    ) {
        return false;
    }
    request
        .effective_phase
        .active_memory
        .iter()
        .find(|memory| memory_identity(&memory.package, &memory.space) == alias.identity)
        .is_some_and(|memory| {
            memory.record_types.iter().any(|record_type| {
                !memory_content_filter_path_enumeration(&record_type.content_schema).complete
            })
        })
}

fn memory_query_schema(description: &str) -> Value {
    json!({
        "type": "string",
        "minLength": 1,
        "description": description
    })
}

fn memory_limit_schema() -> Value {
    json!({ "type": "integer", "minimum": 1 })
}

fn memory_write_parameters_schema(
    alias: &super::model::ActionAlias,
    request: &ModelRequest,
) -> Value {
    if let Some(provider_shape) = alias.provider_shape.as_deref() {
        return memory_write_shape_parameters_schema(provider_shape, alias, request);
    }
    let content_schema = memory_write_content_schema(alias, request);
    let operations = memory_write_operation_values(alias, request);
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "operation": {
                "type": "string",
                "enum": operations,
                "description": "Direct Memory mutation intent for this already-selected package/space."
            },
            "record_type": memory_record_type_schema(alias, request),
            "record_id": {
                "type": "string",
                "minLength": 1,
                "description": "Existing Memory record id returned by authorized Memory access for update/delete/archive."
            },
            "content": content_schema
        },
        "required": ["operation", "record_type"]
    })
}

fn memory_write_shape_parameters_schema(
    provider_shape: &str,
    alias: &super::model::ActionAlias,
    request: &ModelRequest,
) -> Value {
    if let Some(parsed_shape) = memory_write_provider_shape(provider_shape) {
        if let Some(record_type) = parsed_shape.record_type() {
            let content_schema =
                memory_write_content_schema_for_record_type(alias, request, record_type);
            let record_type_schema = json!({
                "type": "string",
                "const": record_type
            });
            if matches!(
                parsed_shape,
                MemoryWriteProviderShape::Create { .. }
                    | MemoryWriteProviderShape::CreateOrUpsert { .. }
            ) {
                let operations = match parsed_shape {
                    MemoryWriteProviderShape::Create { .. } => json!(["create"]),
                    MemoryWriteProviderShape::CreateOrUpsert { .. } => json!(["create", "upsert"]),
                    _ => unreachable!("matched create shapes"),
                };
                return json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": operations,
                            "description": "Create or upsert Memory content for this fixed record type."
                        },
                        "record_type": record_type_schema,
                        "content": content_schema
                    },
                    "required": ["operation", "content"]
                });
            }
            if matches!(parsed_shape, MemoryWriteProviderShape::Update { .. }) {
                return json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "operation": {
                            "type": "string",
                            "const": "update",
                            "description": "Update an existing Memory record for this fixed record type."
                        },
                        "record_id": memory_record_id_schema(),
                        "record_type": record_type_schema,
                        "content": content_schema
                    },
                    "required": ["operation", "record_id", "content"]
                });
            }
        }
        if matches!(parsed_shape, MemoryWriteProviderShape::DeleteOrArchive) {
            return json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["delete", "archive"],
                        "description": "Delete or archive an existing Memory record in this fixed package/space."
                    },
                    "record_id": memory_record_id_schema(),
                    "record_type": memory_record_type_schema(alias, request)
                },
                "required": ["operation", "record_id", "record_type"]
            });
        }
    }
    memory_write_parameters_schema(
        &super::model::ActionAlias {
            provider_shape: None,
            ..alias.clone()
        },
        request,
    )
}

fn memory_write_operation_values(
    alias: &super::model::ActionAlias,
    request: &ModelRequest,
) -> Vec<&'static str> {
    let append_only = request
        .effective_phase
        .active_memory
        .iter()
        .find(|memory| memory_identity(&memory.package, &memory.space) == alias.identity)
        .map(|memory| memory.append_only)
        .unwrap_or(false);
    if append_only {
        vec!["create"]
    } else {
        vec!["create", "upsert", "update", "delete", "archive"]
    }
}

fn memory_record_type_schema(alias: &super::model::ActionAlias, request: &ModelRequest) -> Value {
    let record_types = request
        .effective_phase
        .active_memory
        .iter()
        .find(|memory| memory_identity(&memory.package, &memory.space) == alias.identity)
        .map(|memory| {
            memory
                .record_types
                .iter()
                .map(|record_type| record_type.name.clone())
                .collect::<Vec<_>>()
        })
        .filter(|record_types| !record_types.is_empty());
    match record_types {
        Some(record_types) => json!({
            "type": "string",
            "enum": record_types
        }),
        None => json!({
            "type": "string",
            "minLength": 1
        }),
    }
}

fn memory_write_content_schema(alias: &super::model::ActionAlias, request: &ModelRequest) -> Value {
    let schemas = request
        .effective_phase
        .active_memory
        .iter()
        .find(|memory| memory_identity(&memory.package, &memory.space) == alias.identity)
        .map(|memory| {
            memory
                .record_types
                .iter()
                .map(|record_type| record_type.content_schema.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    match schemas.as_slice() {
        [schema] => schema.clone(),
        [] => json!({
            "type": "object",
            "additionalProperties": true,
            "description": "Memory record content proposed by the model."
        }),
        _ => json!({
            "type": "object",
            "additionalProperties": true,
            "description": "Memory record content proposed by the model. Harness validates this against the selected record type contract before mutation."
        }),
    }
}

fn memory_write_content_schema_for_record_type(
    alias: &super::model::ActionAlias,
    request: &ModelRequest,
    record_type_name: &str,
) -> Value {
    request
        .effective_phase
        .active_memory
        .iter()
        .find(|memory| memory_identity(&memory.package, &memory.space) == alias.identity)
        .and_then(|memory| {
            memory
                .record_types
                .iter()
                .find(|record_type| record_type.name == record_type_name)
        })
        .map(|record_type| record_type.content_schema.clone())
        .unwrap_or_else(|| {
            json!({
                "type": "object",
                "additionalProperties": true,
                "description": "Memory record content proposed by the model."
            })
        })
}

fn memory_identity(package: &str, space: &str) -> String {
    format!("{package}/{space}")
}

fn memory_retrieval_mode_label(mode: &MemoryRetrievalMode) -> String {
    match mode {
        MemoryRetrievalMode::Key => "key",
        MemoryRetrievalMode::Filter => "filter",
        MemoryRetrievalMode::Chronological => "chronological",
        MemoryRetrievalMode::FullText => "full_text",
        MemoryRetrievalMode::Semantic => "semantic",
    }
    .into()
}

fn knowledge_request_parameters_schema(
    alias: &super::model::ActionAlias,
    request: &ModelRequest,
) -> Value {
    let knowledge = request
        .effective_phase
        .active_knowledge
        .iter()
        .find(|knowledge| knowledge.name == alias.identity)
        .or_else(|| {
            request
                .runtime
                .knowledge
                .iter()
                .find(|knowledge| knowledge.name == alias.identity)
        });

    match knowledge.map(|knowledge| knowledge.mode.as_str()) {
        Some("context") => {
            let documents = knowledge
                .map(|knowledge| {
                    knowledge
                        .documents
                        .iter()
                        .map(|document| document.path.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let document_schema = if documents.is_empty() {
                json!({
                    "type": "string",
                    "minLength": 1,
                    "description": "Declared document path inside this Knowledge package. Do not put the package identity here."
                })
            } else {
                json!({
                    "type": "string",
                    "enum": documents,
                    "description": "Declared document path inside this Knowledge package. Do not put the package identity here."
                })
            };
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "mode": {
                        "type": "string",
                        "enum": ["context_document"],
                        "description": "Use context_document for this context Knowledge surface."
                    },
                    "document": document_schema,
                    "return_citations": { "type": "boolean" }
                },
                "required": ["document"]
            })
        }
        Some("vector") => json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "mode": {
                    "type": "string",
                    "enum": ["vector_query"],
                    "description": "Use vector_query for this vector Knowledge surface."
                },
                "query": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Text query to search this vector Knowledge package. Do not include a document path or package identity here."
                },
                "top_k": { "type": "integer", "minimum": 1 },
                "score_threshold": { "type": "number" },
                "return_citations": { "type": "boolean" }
            },
            "required": ["query"]
        }),
        _ => json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "mode": {
                    "type": "string",
                    "enum": ["context_document", "vector_query"],
                    "description": "Required Knowledge request mode. This fallback schema is used only when Knowledge metadata is unavailable."
                },
                "document": { "type": "string", "minLength": 1 },
                "query": { "type": "string", "minLength": 1 },
                "top_k": { "type": "integer", "minimum": 1 },
                "score_threshold": { "type": "number" },
                "return_citations": { "type": "boolean" }
            },
            "required": ["mode"]
        }),
    }
}

fn agentpm_tool_parameters_schema(
    alias: &super::model::ActionAlias,
    request: &ModelRequest,
) -> Value {
    let input_schema = request
        .runtime
        .tools
        .iter()
        .find(|tool| tool.name == alias.identity)
        .map(|tool| tool.input_schema.clone())
        .unwrap_or_else(|| {
            json!({
                "type": "object",
                "additionalProperties": true
            })
        });
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "arguments": input_schema
        },
        "required": ["arguments"]
    })
}

fn external_mcp_tool_parameters_schema(
    alias: &super::model::ActionAlias,
    request: &ModelRequest,
) -> Value {
    let input_schema = request
        .effective_phase
        .active_mcp_tools
        .iter()
        .find(|tool| tool.identity == alias.identity)
        .map(|tool| provider_safe_mcp_input_schema(&tool.input_schema))
        .unwrap_or_else(|| {
            json!({
                "type": "object",
                "additionalProperties": true
            })
        });
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "arguments": input_schema
        },
        "required": ["arguments"]
    })
}

fn provider_safe_mcp_input_schema(schema: &Value) -> Value {
    match schema {
        Value::Object(object) => {
            let had_unsupported_wrapper = object.contains_key("$ref")
                || object.contains_key("allOf")
                || object.contains_key("anyOf")
                || object.contains_key("oneOf");
            let mut sanitized = serde_json::Map::new();
            for (key, value) in object {
                match key.as_str() {
                    "$defs"
                    | "$id"
                    | "$schema"
                    | "$ref"
                    | "allOf"
                    | "anyOf"
                    | "oneOf"
                    | "not"
                    | "patternProperties"
                    | "unevaluatedProperties" => {}
                    "properties" => {
                        if let Some(properties) = value.as_object() {
                            let properties = properties
                                .iter()
                                .map(|(name, schema)| {
                                    (name.clone(), provider_safe_mcp_input_schema(schema))
                                })
                                .collect();
                            sanitized.insert(key.clone(), Value::Object(properties));
                        }
                    }
                    "items" => {
                        sanitized.insert(key.clone(), provider_safe_mcp_input_schema(value));
                    }
                    "additionalProperties" => {
                        if value.is_boolean() {
                            sanitized.insert(key.clone(), value.clone());
                        } else {
                            sanitized.insert(key.clone(), Value::Bool(true));
                        }
                    }
                    _ => {
                        sanitized.insert(key.clone(), value.clone());
                    }
                }
            }
            if sanitized.is_empty() && had_unsupported_wrapper {
                json!({
                    "type": "object",
                    "additionalProperties": true
                })
            } else {
                Value::Object(sanitized)
            }
        }
        Value::Array(items) => {
            Value::Array(items.iter().map(provider_safe_mcp_input_schema).collect())
        }
        _ => schema.clone(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum McpProviderSchemaDegradation {
    Reduced,
    OpenFallback,
}

fn mcp_provider_schema_degradation(schema: &Value) -> Option<McpProviderSchemaDegradation> {
    let provider_schema = provider_safe_mcp_input_schema(schema);
    if provider_schema == *schema {
        return None;
    }
    if provider_schema
        == json!({
            "type": "object",
            "additionalProperties": true
        })
    {
        Some(McpProviderSchemaDegradation::OpenFallback)
    } else {
        Some(McpProviderSchemaDegradation::Reduced)
    }
}

fn skill_resource_read_parameters_schema(
    alias: &super::model::ActionAlias,
    request: &ModelRequest,
) -> Value {
    let resources = request
        .runtime
        .skills
        .iter()
        .find(|skill| skill.name == alias.identity)
        .map(|skill| {
            skill
                .resources
                .iter()
                .map(|resource| resource.id.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "resource": {
                "type": "string",
                "enum": resources,
            }
        },
        "required": ["resource"]
    })
}

fn phase_completion_parameters_schema(request: &ModelRequest) -> Value {
    let outcome_schema = if request.prompt.completion.implicit_complete {
        json!({ "type": "string", "const": "complete" })
    } else {
        json!({
            "type": "string",
            "enum": request.prompt.completion.explicit_outcomes
        })
    };
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "outcome": outcome_schema,
            "output": {
                "type": "object",
                "additionalProperties": true,
                "description": "Structured phase output to pass to later phases or terminal output."
            }
        },
        "required": ["outcome"]
    })
}

fn normalize_provider_response(
    response: ProviderResponse,
    aliases: &[super::model::ActionAlias],
) -> Result<ModelTurn, ModelRuntimeFailure> {
    if !response.action_calls.is_empty() {
        let actions = response
            .action_calls
            .iter()
            .enumerate()
            .map(|(index, call)| {
                let action = semantic_action_from_provider_call(call, aliases)?;
                let mut proposal =
                    SemanticActionProposal::new(format!("provider-action-{}", index + 1), action);
                proposal.provider_call_id.clone_from(&call.id);
                proposal.provider_alias = Some(call.alias.clone());
                proposal.provider_arguments = Some(call.arguments.clone());
                Ok(proposal)
            })
            .collect::<Result<Vec<_>, ModelRuntimeFailure>>()?;
        return Ok(ModelTurn {
            assistant_content: non_empty_text(&response.text),
            actions,
            usage: response.usage,
            finish_reason: response.finish_reason,
            provider_metadata: response.metadata,
        });
    }
    let trimmed = response.text.trim();
    let parsed = parse_json_response(trimmed);
    if let Some(value) = parsed {
        return turn_from_json_value(value, response);
    }
    Ok(ModelTurn {
        assistant_content: Some(response.text),
        actions: Vec::new(),
        usage: response.usage,
        finish_reason: response.finish_reason,
        provider_metadata: response.metadata,
    })
}

fn non_empty_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn parse_json_response(text: &str) -> Option<Value> {
    if let Ok(value) = serde_json::from_str(text) {
        return Some(value);
    }
    if let Some(stripped) = text
        .strip_prefix("```json")
        .and_then(|value| value.strip_suffix("```"))
        .or_else(|| {
            text.strip_prefix("```")
                .and_then(|value| value.strip_suffix("```"))
        })
    {
        return serde_json::from_str(stripped.trim()).ok();
    }
    None
}

fn turn_from_json_value(
    value: Value,
    response: ProviderResponse,
) -> Result<ModelTurn, ModelRuntimeFailure> {
    let assistant_content = value
        .get("assistant_content")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            value
                .get("content")
                .and_then(Value::as_str)
                .map(str::to_string)
        });
    let mut actions = Vec::new();
    if let Some(raw_actions) = value.get("actions").and_then(Value::as_array) {
        for (index, raw_action) in raw_actions.iter().enumerate() {
            actions.push(SemanticActionProposal::new(
                format!("provider-action-{}", index + 1),
                semantic_action_from_json(raw_action)?,
            ));
        }
    } else if let Some(outcome) = value.get("outcome").and_then(Value::as_str) {
        actions.push(SemanticActionProposal::new(
            "provider-action-1",
            SemanticAction::PhaseCompletion {
                outcome: Some(outcome.to_string()),
                output: value.get("output").cloned(),
            },
        ));
    }
    Ok(ModelTurn {
        assistant_content,
        actions,
        usage: response.usage,
        finish_reason: response.finish_reason,
        provider_metadata: response.metadata,
    })
}

fn semantic_action_from_json(value: &Value) -> Result<SemanticAction, ModelRuntimeFailure> {
    let action_type = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| ModelRuntimeFailure::new("provider action is missing type"))?;
    match action_type {
        "persistence_review_complete" => Ok(SemanticAction::PersistenceReviewComplete),
        "phase_completion" => Ok(SemanticAction::PhaseCompletion {
            outcome: value
                .get("outcome")
                .and_then(Value::as_str)
                .map(str::to_string),
            output: value.get("output").cloned(),
        }),
        "agentpm_tool" => Ok(SemanticAction::AgentPmTool {
            tool: required_string(value, "tool")?,
            arguments: value.get("arguments").cloned().unwrap_or_else(|| json!({})),
        }),
        "external_mcp_tool" => Ok(SemanticAction::ExternalMcpTool {
            server: required_string(value, "server")?,
            tool: required_string(value, "tool")?,
            arguments: value.get("arguments").cloned().unwrap_or_else(|| json!({})),
        }),
        "skill_resource_read" => Ok(SemanticAction::SkillResourceRead {
            skill: required_string(value, "skill")?,
            resource: required_string(value, "resource")?,
        }),
        "knowledge_request" => Ok(SemanticAction::KnowledgeRequest {
            package: required_string(value, "package")?,
            mode: parse_optional_knowledge_mode(value.get("mode"))?,
            document: value
                .get("document")
                .and_then(Value::as_str)
                .map(str::to_string),
            query: value
                .get("query")
                .and_then(Value::as_str)
                .map(str::to_string),
            top_k: value
                .get("top_k")
                .and_then(Value::as_u64)
                .map(|v| v as usize),
            score_threshold: value.get("score_threshold").and_then(Value::as_f64),
            return_citations: value.get("return_citations").and_then(Value::as_bool),
        }),
        "memory_read" => Ok(SemanticAction::MemoryRead {
            package: required_string(value, "package")?,
            space: required_string(value, "space")?,
            mode: parse_memory_read_mode(value.get("mode"))?,
            record_id: value
                .get("record_id")
                .and_then(Value::as_str)
                .map(str::to_string),
            record_type: value
                .get("record_type")
                .and_then(Value::as_str)
                .map(str::to_string),
            filter: value
                .get("filter")
                .and_then(Value::as_object)
                .map(|object| {
                    object
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect()
                })
                .unwrap_or_default(),
            query: value
                .get("query")
                .and_then(Value::as_str)
                .map(str::to_string),
            limit: value
                .get("limit")
                .and_then(Value::as_u64)
                .map(|limit| limit as usize),
        }),
        "memory_write" => Ok(SemanticAction::MemoryWrite {
            package: required_string(value, "package")?,
            space: required_string(value, "space")?,
            operation: parse_memory_write_operation(value.get("operation"))?,
            record_type: required_string(value, "record_type")?,
            record_id: value
                .get("record_id")
                .and_then(Value::as_str)
                .map(str::to_string),
            content: value.get("content").cloned(),
        }),
        other => Err(ModelRuntimeFailure::new(format!(
            "unsupported provider action type `{other}`"
        ))),
    }
}

fn semantic_action_from_provider_call(
    call: &ProviderActionCall,
    aliases: &[super::model::ActionAlias],
) -> Result<SemanticAction, ModelRuntimeFailure> {
    let alias = aliases
        .iter()
        .find(|alias| alias.alias == call.alias)
        .ok_or_else(|| {
            ModelRuntimeFailure::new(format!(
                "provider action alias `{}` is not recognized",
                call.alias
            ))
        })?;
    match alias.action_kind.as_str() {
        "persistence_review_complete" => Ok(SemanticAction::PersistenceReviewComplete),
        "phase_completion" => Ok(SemanticAction::PhaseCompletion {
            outcome: call
                .arguments
                .get("outcome")
                .and_then(Value::as_str)
                .map(str::to_string),
            output: call.arguments.get("output").cloned(),
        }),
        "agentpm_tool" => Ok(SemanticAction::AgentPmTool {
            tool: alias.identity.clone(),
            arguments: call
                .arguments
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({})),
        }),
        "external_mcp_tool" => {
            let server = alias.mcp_server.clone().ok_or_else(|| {
                ModelRuntimeFailure::new(format!(
                    "provider action alias `{}` is missing MCP server metadata",
                    alias.alias
                ))
            })?;
            let tool = alias.mcp_tool.clone().ok_or_else(|| {
                ModelRuntimeFailure::new(format!(
                    "provider action alias `{}` is missing MCP tool metadata",
                    alias.alias
                ))
            })?;
            Ok(SemanticAction::ExternalMcpTool {
                server,
                tool,
                arguments: call
                    .arguments
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({})),
            })
        }
        "skill_resource_read" => {
            let resource = required_string(&call.arguments, "resource")?;
            Ok(SemanticAction::SkillResourceRead {
                skill: alias.identity.clone(),
                resource,
            })
        }
        "knowledge_request" => Ok(SemanticAction::KnowledgeRequest {
            package: alias.identity.clone(),
            mode: parse_optional_knowledge_mode(call.arguments.get("mode"))?,
            document: call
                .arguments
                .get("document")
                .and_then(Value::as_str)
                .map(str::to_string),
            query: call
                .arguments
                .get("query")
                .and_then(Value::as_str)
                .map(str::to_string),
            top_k: call
                .arguments
                .get("top_k")
                .and_then(Value::as_u64)
                .map(|v| v as usize),
            score_threshold: call
                .arguments
                .get("score_threshold")
                .and_then(Value::as_f64),
            return_citations: call
                .arguments
                .get("return_citations")
                .and_then(Value::as_bool),
        }),
        "memory_read" => {
            let (package, space) = split_identity(&alias.identity)?;
            let fixed_mode = memory_read_mode_from_provider_shape(alias.provider_shape.as_deref())?;
            let mode = match (fixed_mode, call.arguments.get("mode")) {
                (Some(fixed_mode), Some(value)) => {
                    let proposed = parse_memory_read_mode(Some(value))?;
                    if proposed != fixed_mode {
                        return Err(ModelRuntimeFailure::new(format!(
                            "memory_read provider action `{}` fixes mode `{}` but arguments requested `{}`",
                            alias.alias,
                            memory_read_mode_name(fixed_mode),
                            memory_read_mode_name(proposed)
                        )));
                    }
                    fixed_mode
                }
                (Some(fixed_mode), None) => fixed_mode,
                (None, value) => parse_memory_read_mode(value)?,
            };
            Ok(SemanticAction::MemoryRead {
                package,
                space,
                mode,
                record_id: call
                    .arguments
                    .get("record_id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                record_type: call
                    .arguments
                    .get("record_type")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                filter: call
                    .arguments
                    .get("filter")
                    .and_then(Value::as_object)
                    .map(|object| {
                        object
                            .iter()
                            .map(|(key, value)| (key.clone(), value.clone()))
                            .collect()
                    })
                    .unwrap_or_default(),
                query: call
                    .arguments
                    .get("query")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                limit: call
                    .arguments
                    .get("limit")
                    .and_then(Value::as_u64)
                    .map(|limit| limit as usize),
            })
        }
        "memory_write" => {
            let (package, space) = split_identity(&alias.identity)?;
            let fixed_operations =
                memory_write_operations_from_provider_shape(alias.provider_shape.as_deref())?;
            let operation = match (fixed_operations.as_deref(), call.arguments.get("operation")) {
                (Some(operations), Some(value)) => {
                    let proposed = parse_memory_write_operation(Some(value))?;
                    if !operations.contains(&proposed) {
                        return Err(ModelRuntimeFailure::new(format!(
                            "memory_write provider action `{}` does not permit operation `{}`",
                            alias.alias,
                            memory_write_operation_name(proposed)
                        )));
                    }
                    proposed
                }
                (Some([operation]), None) => *operation,
                (Some(_), None) => {
                    return Err(ModelRuntimeFailure::new(
                        "memory_write operation is required",
                    ));
                }
                (None, value) => parse_memory_write_operation(value)?,
            };
            let fixed_record_type = alias.provider_shape.as_deref().and_then(|provider_shape| {
                memory_write_provider_shape(provider_shape).and_then(|shape| shape.record_type())
            });
            let record_type = match (fixed_record_type, call.arguments.get("record_type")) {
                (Some(fixed_record_type), Some(value)) => {
                    let proposed = value.as_str().ok_or_else(|| {
                        ModelRuntimeFailure::new("memory_write record_type must be a string")
                    })?;
                    if proposed != fixed_record_type {
                        return Err(ModelRuntimeFailure::new(format!(
                            "memory_write provider action `{}` fixes record_type `{}` but arguments requested `{}`",
                            alias.alias, fixed_record_type, proposed
                        )));
                    }
                    fixed_record_type.to_string()
                }
                (Some(fixed_record_type), None) => fixed_record_type.to_string(),
                (None, _) => required_string(&call.arguments, "record_type")?,
            };
            Ok(SemanticAction::MemoryWrite {
                package,
                space,
                operation,
                record_type,
                record_id: call
                    .arguments
                    .get("record_id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                content: call.arguments.get("content").cloned(),
            })
        }
        other => Err(ModelRuntimeFailure::new(format!(
            "unsupported provider action kind `{other}`"
        ))),
    }
}

fn memory_read_mode_from_provider_shape(
    provider_shape: Option<&str>,
) -> Result<Option<MemoryReadMode>, ModelRuntimeFailure> {
    let Some(provider_shape) = provider_shape else {
        return Ok(None);
    };
    match provider_shape {
        "key_document" | "key_record" => Ok(Some(MemoryReadMode::Key)),
        "filter" => Ok(Some(MemoryReadMode::Filter)),
        "chronological" => Ok(Some(MemoryReadMode::Chronological)),
        "full_text" => Ok(Some(MemoryReadMode::FullText)),
        "semantic" => Ok(Some(MemoryReadMode::Semantic)),
        other => Err(ModelRuntimeFailure::new(format!(
            "unsupported memory_read provider shape `{other}`"
        ))),
    }
}

fn memory_read_mode_name(mode: MemoryReadMode) -> &'static str {
    match mode {
        MemoryReadMode::Key => "key",
        MemoryReadMode::Filter => "filter",
        MemoryReadMode::Chronological => "chronological",
        MemoryReadMode::FullText => "full_text",
        MemoryReadMode::Semantic => "semantic",
    }
}

fn memory_read_shape_description(
    provider_shape: &str,
    filter_supported: bool,
) -> Option<&'static str> {
    match provider_shape {
        "key_document" => Some(
            "key read for the current scoped document; do not provide record_id, query, or filter",
        ),
        "key_record" => Some("key read by existing record_id; do not provide query or filter"),
        "chronological" => {
            Some("chronological list read; only record_type and limit may narrow results")
        }
        "filter" => Some("filter read; provide filter and do not provide query or record_id"),
        "full_text" => Some("full_text read; provide query and do not provide record_id or filter"),
        "semantic" if filter_supported => Some(
            "semantic read; provide query, optionally provide filter, and do not provide record_id",
        ),
        "semantic" => Some("semantic read; provide query and do not provide record_id or filter"),
        _ => None,
    }
}

fn memory_write_shape_description(provider_shape: &str) -> Option<&'static str> {
    match memory_write_provider_shape(provider_shape)? {
        MemoryWriteProviderShape::Create { .. } => {
            Some("create; provide content and do not provide record_id")
        }
        MemoryWriteProviderShape::CreateOrUpsert { .. } => {
            Some("create/upsert; provide content and do not provide record_id")
        }
        MemoryWriteProviderShape::Update { .. } => Some("update; provide record_id and content"),
        MemoryWriteProviderShape::DeleteOrArchive => {
            Some("delete/archive; provide record_id and record_type, and do not provide content")
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemoryWriteProviderShape<'a> {
    Create { record_type: &'a str },
    CreateOrUpsert { record_type: &'a str },
    Update { record_type: &'a str },
    DeleteOrArchive,
}

impl<'a> MemoryWriteProviderShape<'a> {
    fn record_type(self) -> Option<&'a str> {
        match self {
            Self::Create { record_type }
            | Self::CreateOrUpsert { record_type }
            | Self::Update { record_type } => Some(record_type),
            Self::DeleteOrArchive => None,
        }
    }

    fn operations(self) -> &'static [MemoryWriteOperation] {
        match self {
            Self::Create { .. } => &[MemoryWriteOperation::Create],
            Self::CreateOrUpsert { .. } => {
                &[MemoryWriteOperation::Create, MemoryWriteOperation::Upsert]
            }
            Self::Update { .. } => &[MemoryWriteOperation::Update],
            Self::DeleteOrArchive => &[MemoryWriteOperation::Delete, MemoryWriteOperation::Archive],
        }
    }
}

fn memory_write_provider_shape(provider_shape: &str) -> Option<MemoryWriteProviderShape<'_>> {
    if provider_shape == "delete_or_archive" {
        return Some(MemoryWriteProviderShape::DeleteOrArchive);
    }
    if let Some(record_type) = provider_shape.strip_prefix("create_or_upsert_") {
        return (!record_type.is_empty())
            .then_some(MemoryWriteProviderShape::CreateOrUpsert { record_type });
    }
    if let Some(record_type) = provider_shape.strip_prefix("create_only_") {
        return (!record_type.is_empty())
            .then_some(MemoryWriteProviderShape::Create { record_type });
    }
    if let Some(record_type) = provider_shape.strip_prefix("update_") {
        return (!record_type.is_empty())
            .then_some(MemoryWriteProviderShape::Update { record_type });
    }
    None
}

fn memory_write_operations_from_provider_shape(
    provider_shape: Option<&str>,
) -> Result<Option<Vec<MemoryWriteOperation>>, ModelRuntimeFailure> {
    let Some(provider_shape) = provider_shape else {
        return Ok(None);
    };
    memory_write_provider_shape(provider_shape)
        .map(|shape| Some(shape.operations().to_vec()))
        .ok_or_else(|| {
            ModelRuntimeFailure::new(format!(
                "unsupported memory_write provider shape `{provider_shape}`"
            ))
        })
}

fn split_identity(identity: &str) -> Result<(String, String), ModelRuntimeFailure> {
    identity
        .rsplit_once('/')
        .map(|(left, right)| (left.to_string(), right.to_string()))
        .ok_or_else(|| {
            ModelRuntimeFailure::new(format!(
                "provider action identity `{identity}` cannot be split"
            ))
        })
}

fn required_string(value: &Value, key: &str) -> Result<String, ModelRuntimeFailure> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ModelRuntimeFailure::new(format!("provider action is missing `{key}`")))
}

fn parse_optional_knowledge_mode(
    value: Option<&Value>,
) -> Result<Option<super::knowledge::KnowledgeRequestMode>, ModelRuntimeFailure> {
    let Some(value) = value else {
        return Ok(None);
    };
    let Some(mode) = value.as_str() else {
        return Err(ModelRuntimeFailure::new(
            "knowledge_request mode must be a string",
        ));
    };
    match mode {
        "context_document" => Ok(Some(
            super::knowledge::KnowledgeRequestMode::ContextDocument,
        )),
        "vector_query" => Ok(Some(super::knowledge::KnowledgeRequestMode::VectorQuery)),
        other => Err(ModelRuntimeFailure::new(format!(
            "unsupported knowledge_request mode `{other}`"
        ))),
    }
}

fn parse_memory_read_mode(value: Option<&Value>) -> Result<MemoryReadMode, ModelRuntimeFailure> {
    let Some(value) = value else {
        return Err(ModelRuntimeFailure::new("memory_read mode is required"));
    };
    let Some(mode) = value.as_str() else {
        return Err(ModelRuntimeFailure::new(
            "memory_read mode must be a string",
        ));
    };
    match mode {
        "key" => Ok(MemoryReadMode::Key),
        "filter" => Ok(MemoryReadMode::Filter),
        "chronological" => Ok(MemoryReadMode::Chronological),
        "full_text" => Ok(MemoryReadMode::FullText),
        "semantic" => Ok(MemoryReadMode::Semantic),
        other => Err(ModelRuntimeFailure::new(format!(
            "unsupported memory_read mode `{other}`"
        ))),
    }
}

fn parse_memory_write_operation(
    value: Option<&Value>,
) -> Result<MemoryWriteOperation, ModelRuntimeFailure> {
    let Some(value) = value else {
        return Err(ModelRuntimeFailure::new(
            "memory_write operation is required",
        ));
    };
    let Some(operation) = value.as_str() else {
        return Err(ModelRuntimeFailure::new(
            "memory_write operation must be a string",
        ));
    };
    match operation {
        "create" => Ok(MemoryWriteOperation::Create),
        "upsert" => Ok(MemoryWriteOperation::Upsert),
        "update" => Ok(MemoryWriteOperation::Update),
        "delete" => Ok(MemoryWriteOperation::Delete),
        "archive" => Ok(MemoryWriteOperation::Archive),
        other => Err(ModelRuntimeFailure::new(format!(
            "unsupported memory_write operation `{other}`"
        ))),
    }
}

fn memory_write_operation_name(operation: MemoryWriteOperation) -> &'static str {
    match operation {
        MemoryWriteOperation::Create => "create",
        MemoryWriteOperation::Upsert => "upsert",
        MemoryWriteOperation::Update => "update",
        MemoryWriteOperation::Delete => "delete",
        MemoryWriteOperation::Archive => "archive",
    }
}

#[derive(Default)]
pub struct OpenAiTransport {
    client: Client,
}

impl ModelProviderTransport for OpenAiTransport {
    fn send(&mut self, request: ProviderRequest) -> Result<ProviderResponse, ModelRuntimeFailure> {
        let api_key = env::var("OPENAI_API_KEY").map_err(|_| {
            ModelRuntimeFailure::new("OPENAI_API_KEY is required for provider `openai`")
        })?;
        let url = env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1/chat/completions".into());
        let body = openai_request_body(&request);
        let value: Value = self
            .client
            .post(url)
            .bearer_auth(api_key)
            .json(&Value::Object(body))
            .send()
            .and_then(|response| response.error_for_status())
            .and_then(|response| response.json())
            .map_err(|err| ModelRuntimeFailure::new(format!("openai request failed: {err}")))?;
        provider_response_from_openai(value)
    }
}

fn openai_request_body(request: &ProviderRequest) -> Map<String, Value> {
    let mut body = object_options(&request.selection.options);
    body.insert("model".into(), json!(request.selection.model));
    body.insert("messages".into(), json!(openai_messages(request)));
    if !request.actions.is_empty() {
        body.insert("tools".into(), openai_tool_definitions(&request.actions));
        body.entry("tool_choice").or_insert(json!("auto"));
    }
    body
}

fn openai_messages(request: &ProviderRequest) -> Vec<Value> {
    if request.turns.is_empty() {
        return vec![json!({ "role": "user", "content": request.prompt })];
    }
    let mut messages = vec![json!({ "role": "system", "content": request.prompt })];
    for turn in &request.turns {
        match turn {
            ModelRequestTurn::UserInput { content } => {
                messages.push(json!({ "role": "user", "content": content }));
            }
            ModelRequestTurn::AssistantContent { content } => {
                messages.push(json!({ "role": "assistant", "content": content }));
            }
            ModelRequestTurn::SemanticActionCall {
                provider_call_id: Some(provider_call_id),
                provider_alias: Some(provider_alias),
                arguments,
                ..
            } => {
                messages.push(json!({
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": provider_call_id,
                        "type": "function",
                        "function": {
                            "name": provider_alias,
                            "arguments": arguments.to_string()
                        }
                    }]
                }));
            }
            ModelRequestTurn::SemanticActionCall { .. } => {
                messages.push(json!({
                    "role": "user",
                    "content": native_turn_text(turn)
                }));
            }
            ModelRequestTurn::SemanticActionResult {
                provider_call_id: Some(provider_call_id),
                result,
                ..
            } => {
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": provider_call_id,
                    "content": result.to_string()
                }));
            }
            ModelRequestTurn::SemanticActionResult { .. } => {
                messages.push(json!({
                    "role": "user",
                    "content": native_turn_text(turn)
                }));
            }
            ModelRequestTurn::RepairFeedback { content } => {
                messages.push(json!({ "role": "user", "content": repair_feedback_turn(content) }));
            }
        }
    }
    messages
}

fn provider_response_from_openai(value: Value) -> Result<ProviderResponse, ModelRuntimeFailure> {
    let text = value
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let action_calls = value
        .pointer("/choices/0/message/tool_calls")
        .and_then(Value::as_array)
        .map(|calls| {
            calls
                .iter()
                .map(openai_tool_call)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    if text.is_empty() && action_calls.is_empty() {
        return Err(ModelRuntimeFailure::new(
            "openai response did not contain message content or tool calls",
        ));
    }
    let usage = usage_from_json(value.get("usage"));
    let finish_reason = value
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok(ProviderResponse {
        text,
        action_calls,
        usage,
        finish_reason,
        metadata: BTreeMap::from([("provider".into(), json!("openai"))]),
    })
}

#[derive(Default)]
pub struct AnthropicTransport {
    client: Client,
}

impl ModelProviderTransport for AnthropicTransport {
    fn send(&mut self, request: ProviderRequest) -> Result<ProviderResponse, ModelRuntimeFailure> {
        let api_key = env::var("ANTHROPIC_API_KEY").map_err(|_| {
            ModelRuntimeFailure::new("ANTHROPIC_API_KEY is required for provider `anthropic`")
        })?;
        let url = env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com/v1/messages".into());
        let body = anthropic_request_body(&request);
        let value: Value = self
            .client
            .post(url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&Value::Object(body))
            .send()
            .and_then(|response| response.error_for_status())
            .and_then(|response| response.json())
            .map_err(|err| ModelRuntimeFailure::new(format!("anthropic request failed: {err}")))?;
        provider_response_from_anthropic(value)
    }
}

fn anthropic_request_body(request: &ProviderRequest) -> Map<String, Value> {
    let mut body = object_options(&request.selection.options);
    body.insert("model".into(), json!(request.selection.model));
    body.entry("max_tokens").or_insert(json!(1024));
    body.insert("system".into(), json!(request.prompt));
    body.insert("messages".into(), json!(anthropic_messages(request)));
    if !request.actions.is_empty() {
        body.insert("tools".into(), anthropic_tool_definitions(&request.actions));
    }
    body
}

fn anthropic_messages(request: &ProviderRequest) -> Vec<Value> {
    if request.turns.is_empty() {
        return vec![json!({ "role": "user", "content": request.prompt })];
    }
    let mut messages = Vec::new();
    for turn in &request.turns {
        match turn {
            ModelRequestTurn::UserInput { content } => {
                messages.push(json!({ "role": "user", "content": content }));
            }
            ModelRequestTurn::AssistantContent { content } => {
                messages.push(json!({ "role": "assistant", "content": content }));
            }
            ModelRequestTurn::SemanticActionCall {
                provider_call_id: Some(provider_call_id),
                provider_alias: Some(provider_alias),
                arguments,
                ..
            } => {
                messages.push(json!({
                    "role": "assistant",
                    "content": [{
                        "type": "tool_use",
                        "id": provider_call_id,
                        "name": provider_alias,
                        "input": arguments
                    }]
                }));
            }
            ModelRequestTurn::SemanticActionCall { .. } => {
                messages.push(json!({
                    "role": "user",
                    "content": native_turn_text(turn)
                }));
            }
            ModelRequestTurn::SemanticActionResult {
                provider_call_id: Some(provider_call_id),
                result,
                ..
            } => {
                messages.push(json!({
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": provider_call_id,
                        "content": result.to_string()
                    }]
                }));
            }
            ModelRequestTurn::SemanticActionResult { .. } => {
                messages.push(json!({
                    "role": "user",
                    "content": native_turn_text(turn)
                }));
            }
            ModelRequestTurn::RepairFeedback { content } => {
                messages.push(json!({ "role": "user", "content": repair_feedback_turn(content) }));
            }
        }
    }
    messages
}

fn provider_response_from_anthropic(value: Value) -> Result<ProviderResponse, ModelRuntimeFailure> {
    let content_items = value
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| ModelRuntimeFailure::new("anthropic response did not contain content"))?;
    let text = content_items
        .iter()
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    let action_calls = content_items
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("tool_use"))
        .map(anthropic_tool_call)
        .collect::<Result<Vec<_>, _>>()?;
    if text.is_empty() && action_calls.is_empty() {
        return Err(ModelRuntimeFailure::new(
            "anthropic response did not contain text content or tool calls",
        ));
    }
    let usage = usage_from_json(value.get("usage"));
    let finish_reason = value
        .get("stop_reason")
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok(ProviderResponse {
        text,
        action_calls,
        usage,
        finish_reason,
        metadata: BTreeMap::from([("provider".into(), json!("anthropic"))]),
    })
}

#[derive(Default)]
pub struct OllamaTransport {
    client: Client,
}

impl ModelProviderTransport for OllamaTransport {
    fn send(&mut self, request: ProviderRequest) -> Result<ProviderResponse, ModelRuntimeFailure> {
        let base_url =
            env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| "http://localhost:11434".into());
        let url = format!("{}/api/chat", base_url.trim_end_matches('/'));
        let body = ollama_request_body(&request);
        let value: Value = self
            .client
            .post(url)
            .json(&Value::Object(body))
            .send()
            .and_then(|response| response.error_for_status())
            .and_then(|response| response.json())
            .map_err(|err| ModelRuntimeFailure::new(format!("ollama request failed: {err}")))?;
        provider_response_from_ollama(value)
    }
}

fn ollama_request_body(request: &ProviderRequest) -> Map<String, Value> {
    let mut body = object_options(&request.selection.options);
    body.insert("model".into(), json!(request.selection.model));
    body.insert("stream".into(), json!(false));
    body.insert("messages".into(), json!(ollama_messages(request)));
    if !request.actions.is_empty() {
        body.insert("tools".into(), openai_tool_definitions(&request.actions));
    }
    body
}

fn ollama_messages(request: &ProviderRequest) -> Vec<Value> {
    openai_messages(request)
}

fn native_turn_text(turn: &ModelRequestTurn) -> String {
    match turn {
        ModelRequestTurn::SemanticActionCall {
            action_kind,
            identity,
            arguments,
            ..
        } => format!(
            "Previous semantic action call [{action_kind} {identity}]: {}",
            arguments
        ),
        ModelRequestTurn::SemanticActionResult {
            action_kind,
            identity,
            result,
            action_succeeded,
            ..
        } => format!(
            "Previous semantic action result [{action_kind} {identity}] succeeded={}: {}",
            action_succeeded
                .map(|succeeded| succeeded.to_string())
                .unwrap_or_else(|| "unknown".into()),
            result
        ),
        other => serde_json::to_string(other).unwrap_or_else(|_| format!("{other:?}")),
    }
}

fn repair_feedback_turn(content: &str) -> String {
    format!("Repair feedback from previous turn: {content}")
}

fn provider_response_from_ollama(value: Value) -> Result<ProviderResponse, ModelRuntimeFailure> {
    let text = value
        .pointer("/message/content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let action_calls = value
        .pointer("/message/tool_calls")
        .and_then(Value::as_array)
        .map(|calls| {
            calls
                .iter()
                .map(ollama_tool_call)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    if text.is_empty() && action_calls.is_empty() {
        return Err(ModelRuntimeFailure::new(
            "ollama response did not contain message content or tool calls",
        ));
    }
    Ok(ProviderResponse {
        text,
        action_calls,
        usage: usage_from_json(Some(&value)),
        finish_reason: value
            .get("done_reason")
            .and_then(Value::as_str)
            .map(str::to_string),
        metadata: BTreeMap::from([("provider".into(), json!("ollama"))]),
    })
}

fn openai_tool_definitions(actions: &[ProviderActionTool]) -> Value {
    json!(
        actions
            .iter()
            .map(|action| {
                json!({
                    "type": "function",
                    "function": {
                        "name": action.alias,
                        "description": action.description,
                        "parameters": action.parameters
                    }
                })
            })
            .collect::<Vec<_>>()
    )
}

fn anthropic_tool_definitions(actions: &[ProviderActionTool]) -> Value {
    json!(
        actions
            .iter()
            .map(|action| {
                json!({
                    "name": action.alias,
                    "description": action.description,
                    "input_schema": action.parameters
                })
            })
            .collect::<Vec<_>>()
    )
}

fn openai_tool_call(value: &Value) -> Result<ProviderActionCall, ModelRuntimeFailure> {
    let function = value
        .get("function")
        .ok_or_else(|| ModelRuntimeFailure::new("openai tool call is missing function"))?;
    let alias = required_string(function, "name")?;
    let arguments = function
        .get("arguments")
        .and_then(Value::as_str)
        .map(parse_tool_arguments)
        .transpose()?
        .unwrap_or_else(|| json!({}));
    Ok(ProviderActionCall {
        id: value.get("id").and_then(Value::as_str).map(str::to_string),
        alias,
        arguments,
    })
}

fn anthropic_tool_call(value: &Value) -> Result<ProviderActionCall, ModelRuntimeFailure> {
    Ok(ProviderActionCall {
        id: value.get("id").and_then(Value::as_str).map(str::to_string),
        alias: required_string(value, "name")?,
        arguments: value.get("input").cloned().unwrap_or_else(|| json!({})),
    })
}

fn ollama_tool_call(value: &Value) -> Result<ProviderActionCall, ModelRuntimeFailure> {
    let function = value
        .get("function")
        .ok_or_else(|| ModelRuntimeFailure::new("ollama tool call is missing function"))?;
    let alias = required_string(function, "name")?;
    let arguments = match function.get("arguments") {
        Some(Value::String(raw)) => parse_tool_arguments(raw)?,
        Some(value) => value.clone(),
        None => json!({}),
    };
    Ok(ProviderActionCall {
        id: value.get("id").and_then(Value::as_str).map(str::to_string),
        alias,
        arguments,
    })
}

fn parse_tool_arguments(raw: &str) -> Result<Value, ModelRuntimeFailure> {
    serde_json::from_str(raw).map_err(|err| {
        ModelRuntimeFailure::new(format!(
            "provider tool call arguments are invalid JSON: {err}"
        ))
    })
}

fn object_options(options: &Value) -> Map<String, Value> {
    options.as_object().cloned().unwrap_or_default()
}

fn usage_from_json(value: Option<&Value>) -> RunUsage {
    let mut usage = RunUsage::default();
    if let Some(value) = value {
        usage.tokens.input_tokens = value
            .get("prompt_tokens")
            .or_else(|| value.get("prompt_eval_count"))
            .or_else(|| value.get("input_tokens"))
            .and_then(Value::as_u64);
        usage.tokens.output_tokens = value
            .get("completion_tokens")
            .or_else(|| value.get("eval_count"))
            .or_else(|| value.get("output_tokens"))
            .and_then(Value::as_u64);
        usage.tokens.total_tokens =
            value
                .get("total_tokens")
                .and_then(Value::as_u64)
                .or_else(|| {
                    usage
                        .tokens
                        .input_tokens
                        .zip(usage.tokens.output_tokens)
                        .map(|(input, output)| input + output)
                });
    }
    usage
}

#[cfg(test)]
pub struct MockModelTransport {
    pub responses: Vec<ProviderResponse>,
    pub requests: Vec<ProviderRequest>,
}

#[cfg(test)]
impl MockModelTransport {
    pub fn new(responses: Vec<ProviderResponse>) -> Self {
        Self {
            responses,
            requests: Vec::new(),
        }
    }
}

#[cfg(test)]
impl ModelProviderTransport for MockModelTransport {
    fn send(&mut self, request: ProviderRequest) -> Result<ProviderResponse, ModelRuntimeFailure> {
        self.requests.push(request);
        if self.responses.is_empty() {
            return Err(ModelRuntimeFailure::new("mock model transport exhausted"));
        }
        Ok(self.responses.remove(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness_engine::EffectivePhase;
    use crate::harness_runtime::hook::{
        BeforeModelRequestContextSection, BeforeModelRequestDecision,
        apply_before_model_request_decision,
    };
    use crate::harness_runtime::model::{
        ActionAlias, CapabilityDescriptor, CompletionContract, KnowledgeDocumentSnapshot,
        KnowledgeRetrievalSnapshot, KnowledgeRuntimeSnapshot, LogicalPrompt,
        MemoryRecordTypeRuntimeSnapshot, MemorySpaceRuntimeSnapshot, ModelRequestTurn,
        PromptSection, RuntimeSnapshot, SkillResourceSnapshot, SkillRuntimeSnapshot,
        ToolRuntimeSnapshot, TranscriptEntry, TranscriptEntryKind, model_request_turns,
    };
    use crate::manifest::MemorySpaceModel;
    use std::cell::RefCell;
    use std::fs;
    use std::rc::Rc;

    fn selection(provider: &str) -> ModelProviderSelection {
        ModelProviderSelection {
            provider: provider.into(),
            model: "test-model".into(),
            options: json!({}),
        }
    }

    #[test]
    fn provider_response_normalizes_phase_completion_action() {
        let response = ProviderResponse {
            text: json!({
                "assistant_content": "done",
                "actions": [
                    { "type": "phase_completion", "outcome": "ready", "output": { "ok": true } }
                ]
            })
            .to_string(),
            action_calls: Vec::new(),
            usage: RunUsage::default(),
            finish_reason: Some("stop".into()),
            metadata: BTreeMap::new(),
        };
        let turn = normalize_provider_response(response, &[]).unwrap();
        assert_eq!(turn.assistant_content.as_deref(), Some("done"));
        assert_eq!(turn.actions.len(), 1);
        assert!(matches!(
            turn.actions[0].action,
            SemanticAction::PhaseCompletion { .. }
        ));
        assert_eq!(turn.finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn provider_response_normalizes_persistence_review_complete_action() {
        let response = ProviderResponse {
            text: json!({
                "actions": [
                    { "type": "persistence_review_complete" }
                ]
            })
            .to_string(),
            action_calls: Vec::new(),
            usage: RunUsage::default(),
            finish_reason: Some("stop".into()),
            metadata: BTreeMap::new(),
        };
        let turn = normalize_provider_response(response, &[]).unwrap();
        assert_eq!(turn.actions.len(), 1);
        assert!(matches!(
            turn.actions[0].action,
            SemanticAction::PersistenceReviewComplete
        ));
    }

    #[test]
    fn provider_response_normalizes_native_action_calls_through_aliases() {
        let response = ProviderResponse {
            text: "I am completing this phase.".into(),
            action_calls: vec![ProviderActionCall {
                id: Some("call-openai-1".into()),
                alias: "phase_complete".into(),
                arguments: json!({
                    "outcome": "ready",
                    "output": { "ok": true }
                }),
            }],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            metadata: BTreeMap::new(),
        };
        let request = model_request();
        let turn = normalize_provider_response(response, &request.prompt.action_aliases).unwrap();
        assert_eq!(
            turn.assistant_content.as_deref(),
            Some("I am completing this phase.")
        );
        assert_eq!(turn.actions.len(), 1);
        assert!(matches!(
            &turn.actions[0].action,
            SemanticAction::PhaseCompletion {
                outcome: Some(outcome),
                ..
            } if outcome == "ready"
        ));
        assert_eq!(
            turn.actions[0].provider_call_id.as_deref(),
            Some("call-openai-1")
        );
        assert_eq!(
            turn.actions[0].provider_alias.as_deref(),
            Some("phase_complete")
        );
        assert_eq!(
            turn.actions[0].provider_arguments.as_ref().unwrap()["outcome"],
            json!("ready")
        );
    }

    #[test]
    fn provider_response_normalizes_native_persistence_review_complete_alias() {
        let response = ProviderResponse {
            text: "review complete".into(),
            action_calls: vec![ProviderActionCall {
                id: Some("call-review-1".into()),
                alias: "action_1".into(),
                arguments: json!({}),
            }],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            metadata: BTreeMap::new(),
        };
        let aliases = vec![ActionAlias {
            alias: "action_1".into(),
            action_kind: "persistence_review_complete".into(),
            identity: "harness/persistence_review".into(),
            provider_shape: None,
            mcp_server: None,
            mcp_tool: None,
        }];
        let turn = normalize_provider_response(response, &aliases).unwrap();
        assert_eq!(turn.actions.len(), 1);
        assert!(matches!(
            turn.actions[0].action,
            SemanticAction::PersistenceReviewComplete
        ));
    }

    #[test]
    fn provider_response_maps_duplicate_mcp_tool_names_through_structured_alias_metadata() {
        let response = ProviderResponse {
            text: String::new(),
            action_calls: vec![ProviderActionCall {
                id: Some("call-mcp-1".into()),
                alias: "mcp_tool_search_linear".into(),
                arguments: json!({ "arguments": { "query": "issue" } }),
            }],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            metadata: BTreeMap::new(),
        };
        let aliases = vec![
            ActionAlias {
                alias: "mcp_tool_search_github".into(),
                action_kind: "external_mcp_tool".into(),
                identity: "mcp:github/search".into(),
                provider_shape: None,
                mcp_server: Some("github".into()),
                mcp_tool: Some("search".into()),
            },
            ActionAlias {
                alias: "mcp_tool_search_linear".into(),
                action_kind: "external_mcp_tool".into(),
                identity: "mcp:linear/search".into(),
                provider_shape: None,
                mcp_server: Some("linear".into()),
                mcp_tool: Some("search".into()),
            },
        ];

        let turn = normalize_provider_response(response, &aliases).unwrap();

        assert_eq!(turn.actions.len(), 1);
        assert!(matches!(
            &turn.actions[0].action,
            SemanticAction::ExternalMcpTool {
                server,
                tool,
                arguments
            } if server == "linear"
                && tool == "search"
                && arguments == &json!({ "query": "issue" })
        ));
    }

    #[test]
    fn action_parameter_schemas_use_resolved_tool_and_skill_metadata() {
        let mut request = model_request();
        let guide = vector_knowledge_snapshot("@zack/guide");
        request.runtime.knowledge.push(guide.clone());
        request.effective_phase.active_knowledge.push(guide);
        let mcp_tool = crate::harness_runtime::McpImportRuntimeSnapshot {
            server_id: "incident-data".into(),
            tool_name: "search".into(),
            identity: "mcp:incident-data/search".into(),
            description: "Search incidents over MCP.".into(),
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "query": { "type": "string", "minLength": 1 }
                },
                "required": ["query"]
            }),
            transport: "http".into(),
            scopes: vec!["global".into()],
            endpoint: Some("https://mcp.example.com/mcp".into()),
            state: "available".into(),
            readiness_reason: None,
            source: "harness_config".into(),
        };
        request.effective_phase.active_mcp_tools.push(mcp_tool);
        let memory = MemorySpaceRuntimeSnapshot {
            package: "@zack/state".into(),
            package_version: "0.1.0".into(),
            space: "conversation_state".into(),
            model: MemorySpaceModel::Document,
            description: "Conversation state.".into(),
            root: None,
            runtime: "local".into(),
            source: "agent_binding".into(),
            state: "available".into(),
            readiness_reason: None,
            binding_scope: "global".into(),
            scope_keys: vec!["user".into()],
            retrieval_modes: vec![MemoryRetrievalMode::Key, MemoryRetrievalMode::Filter],
            semantic: None,
            append_only: false,
            record_types: vec![MemoryRecordTypeRuntimeSnapshot {
                name: "summary".into(),
                schema_version: "1.0.0".into(),
                content_schema: json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "summary": { "type": "string", "minLength": 1 }
                    },
                    "required": ["summary"]
                }),
            }],
        };
        request.effective_phase.active_memory.push(memory);
        let tool_schema = action_parameters_schema(
            &action_alias("action_2", "agentpm_tool", "@zack/search"),
            &request,
        );
        assert_eq!(
            tool_schema["properties"]["arguments"]["required"],
            json!(["q"])
        );
        assert_eq!(
            tool_schema["properties"]["arguments"]["properties"]["q"]["type"],
            json!("string")
        );

        let skill_schema = action_parameters_schema(
            &action_alias("action_3", "skill_resource_read", "@zack/handoff-skill"),
            &request,
        );
        assert_eq!(
            skill_schema["properties"]["resource"]["enum"],
            json!(["entrypoint", "references/architecture.md"])
        );

        let required_cases = [
            (
                action_alias("action_4", "external_mcp_tool", "mcp:incident-data/search"),
                "arguments",
            ),
            (
                action_alias("action_5", "knowledge_request", "@zack/guide"),
                "query",
            ),
            (
                action_alias("action_6", "memory_read", "@zack/state/conversation_state"),
                "mode",
            ),
        ];

        for (alias, required_field) in required_cases {
            let schema = action_parameters_schema(&alias, &request);
            assert_eq!(schema["type"], "object");
            assert_eq!(schema["additionalProperties"], false);
            if alias.action_kind == "knowledge_request" {
                assert!(schema.get("anyOf").is_none());
                assert!(schema["properties"].get(required_field).is_some());
                assert!(
                    schema["required"]
                        .as_array()
                        .expect("knowledge_request required array")
                        .contains(&json!(required_field)),
                    "knowledge_request should require {required_field}"
                );
            } else {
                assert!(
                    schema["required"]
                        .as_array()
                        .expect("schema required array")
                        .contains(&json!(required_field)),
                    "{} should require {required_field}",
                    alias.action_kind
                );
            }
        }
        let mcp_schema = action_parameters_schema(
            &action_alias("action_4", "external_mcp_tool", "mcp:incident-data/search"),
            &request,
        );
        assert_eq!(
            mcp_schema["properties"]["arguments"]["properties"]["query"]["type"],
            json!("string")
        );
        assert_eq!(
            mcp_schema["properties"]["arguments"]["required"],
            json!(["query"])
        );

        let memory_write_schema = action_parameters_schema(
            &action_alias("action_7", "memory_write", "@zack/state/conversation_state"),
            &request,
        );
        assert_eq!(memory_write_schema["type"], "object");
        assert_eq!(
            memory_write_schema["properties"]["record_type"]["enum"],
            json!(["summary"])
        );
        assert_eq!(
            memory_write_schema["properties"]["content"]["properties"]["summary"]["type"],
            json!("string")
        );
        assert_eq!(
            memory_write_schema["properties"]["operation"]["enum"],
            json!(["create", "upsert", "update", "delete", "archive"])
        );
        assert!(
            memory_write_schema["required"]
                .as_array()
                .expect("memory_write required array")
                .contains(&json!("operation"))
        );
        assert!(
            memory_write_schema["required"]
                .as_array()
                .expect("memory_write required array")
                .contains(&json!("record_type"))
        );

        request.effective_phase.active_memory[0].append_only = true;
        let append_only_memory_write_schema = action_parameters_schema(
            &action_alias("action_7", "memory_write", "@zack/state/conversation_state"),
            &request,
        );
        assert_eq!(
            append_only_memory_write_schema["properties"]["operation"]["enum"],
            json!(["create"])
        );
    }

    #[test]
    fn external_mcp_provider_schema_strips_unsupported_composition_without_changing_runtime_schema()
    {
        let mut request = model_request();
        let canonical_schema = json!({
            "$defs": {
                "Query": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" }
                    },
                    "required": ["query"]
                }
            },
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "query": {
                    "anyOf": [
                        { "type": "string" },
                        { "type": "number" }
                    ]
                },
                "options": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                }
            },
            "required": ["query"]
        });
        request.effective_phase.active_mcp_tools.push(
            crate::harness_runtime::McpImportRuntimeSnapshot {
                server_id: "incident-data".into(),
                tool_name: "search".into(),
                identity: "mcp:incident-data/search".into(),
                description: "Search incidents over MCP.".into(),
                input_schema: canonical_schema.clone(),
                transport: "http".into(),
                scopes: vec!["global".into()],
                endpoint: Some("https://mcp.example.com/mcp".into()),
                state: "available".into(),
                readiness_reason: None,
                source: "harness_config".into(),
            },
        );

        let schema = action_parameters_schema(
            &action_alias("action_4", "external_mcp_tool", "mcp:incident-data/search"),
            &request,
        );

        let provider_arguments = &schema["properties"]["arguments"];
        assert!(provider_arguments.get("$defs").is_none());
        assert!(
            provider_arguments["properties"]["query"]
                .get("anyOf")
                .is_none()
        );
        assert_eq!(
            provider_arguments["properties"]["options"]["additionalProperties"],
            json!(true)
        );
        assert_eq!(
            request.effective_phase.active_mcp_tools[0].input_schema,
            canonical_schema
        );
    }

    #[test]
    fn memory_write_provider_actions_advertise_flat_shape_schemas() {
        let mut request = model_request();
        request
            .effective_phase
            .capability_catalog
            .push(CapabilityDescriptor {
                action_kind: "memory_write".into(),
                identity: "@zack/state/conversation_state".into(),
                description: "Write conversation state.".into(),
                source: "agent_binding".into(),
            });
        request
            .effective_phase
            .active_memory
            .push(MemorySpaceRuntimeSnapshot {
                package: "@zack/state".into(),
                package_version: "0.1.0".into(),
                space: "conversation_state".into(),
                model: MemorySpaceModel::Collection,
                description: "Conversation state.".into(),
                root: None,
                runtime: "local".into(),
                source: "agent_binding".into(),
                state: "available".into(),
                readiness_reason: None,
                binding_scope: "global".into(),
                scope_keys: vec!["user".into()],
                retrieval_modes: vec![MemoryRetrievalMode::Key],
                semantic: None,
                append_only: false,
                record_types: vec![
                    MemoryRecordTypeRuntimeSnapshot {
                        name: "summary".into(),
                        schema_version: "1.0.0".into(),
                        content_schema: json!({
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "summary": { "type": "string", "minLength": 1 }
                            },
                            "required": ["summary"]
                        }),
                    },
                    MemoryRecordTypeRuntimeSnapshot {
                        name: "preference".into(),
                        schema_version: "1.0.0".into(),
                        content_schema: json!({
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "preference": { "type": "string", "minLength": 1 }
                            },
                            "required": ["preference"]
                        }),
                    },
                ],
            });
        request.prompt.action_aliases =
            crate::harness_runtime::model::provider_action_aliases(&request.effective_phase);

        let tools = provider_action_tools(&request);
        assert_eq!(
            tools
                .iter()
                .filter(|tool| tool.action_kind == "memory_write")
                .count(),
            5
        );
        let summary_create = tools
            .iter()
            .find(|tool| {
                tool.action_kind == "memory_write"
                    && tool.alias.contains("create_or_upsert_summary")
            })
            .expect("summary create/upsert tool");
        assert_eq!(
            summary_create.parameters["properties"]["operation"]["enum"],
            json!(["create", "upsert"])
        );
        assert_eq!(
            summary_create.parameters["properties"]["record_type"]["const"],
            json!("summary")
        );
        assert_eq!(
            summary_create.parameters["properties"]["content"]["properties"]["summary"]["type"],
            json!("string")
        );
        assert!(
            summary_create.parameters["properties"]["content"]["properties"]
                .get("preference")
                .is_none()
        );
        assert_eq!(
            summary_create.parameters["required"],
            json!(["operation", "content"])
        );
        assert!(
            summary_create.parameters["properties"]
                .get("record_id")
                .is_none()
        );

        let preference_update = tools
            .iter()
            .find(|tool| {
                tool.action_kind == "memory_write" && tool.alias.contains("update_preference")
            })
            .expect("preference update tool");
        assert_eq!(
            preference_update.parameters["properties"]["operation"]["const"],
            json!("update")
        );
        assert_eq!(
            preference_update.parameters["properties"]["record_type"]["const"],
            json!("preference")
        );
        assert_eq!(
            preference_update.parameters["required"],
            json!(["operation", "record_id", "content"])
        );
        assert_eq!(
            preference_update.parameters["properties"]["content"]["properties"]["preference"]["type"],
            json!("string")
        );

        let delete_archive = tools
            .iter()
            .find(|tool| {
                tool.action_kind == "memory_write" && tool.alias.contains("delete_or_archive")
            })
            .expect("delete/archive tool");
        assert_eq!(
            delete_archive.parameters["properties"]["operation"]["enum"],
            json!(["delete", "archive"])
        );
        assert_eq!(
            delete_archive.parameters["properties"]["record_type"]["enum"],
            json!(["summary", "preference"])
        );
        assert!(
            delete_archive.parameters["properties"]
                .get("content")
                .is_none()
        );
        assert_eq!(
            delete_archive.parameters["required"],
            json!(["operation", "record_id", "record_type"])
        );
    }

    #[test]
    fn append_only_create_schema_does_not_treat_record_type_prefix_as_upsert_shape() {
        let mut request = model_request();
        request
            .effective_phase
            .capability_catalog
            .push(CapabilityDescriptor {
                action_kind: "memory_write".into(),
                identity: "@zack/memory/notes".into(),
                description: "Write notes.".into(),
                source: "agent_binding".into(),
            });
        let mut memory = memory_snapshot(
            "@zack/memory",
            "notes",
            MemorySpaceModel::Collection,
            vec![MemoryRetrievalMode::Key],
        );
        memory.append_only = true;
        memory.record_types[0].name = "or_upsert_note".into();
        request.effective_phase.active_memory.push(memory);
        request.prompt.action_aliases =
            crate::harness_runtime::model::provider_action_aliases(&request.effective_phase);

        let tool = provider_action_tools(&request)
            .into_iter()
            .find(|tool| tool.action_kind == "memory_write")
            .expect("memory write provider action");

        assert!(tool.alias.contains("create_only_or_upsert_note"));
        assert_eq!(
            tool.parameters["properties"]["operation"]["enum"],
            json!(["create"])
        );
        assert_eq!(
            tool.parameters["properties"]["record_type"]["const"],
            json!("or_upsert_note")
        );
    }

    #[test]
    fn provider_call_decoding_covers_all_non_phase_action_kinds() {
        let action = decode_provider_call(
            action_alias("action_1", "agentpm_tool", "@zack/search"),
            json!({ "arguments": { "q": "incident" } }),
        );
        match action {
            SemanticAction::AgentPmTool { tool, arguments } => {
                assert_eq!(tool, "@zack/search");
                assert_eq!(arguments["q"], json!("incident"));
            }
            other => panic!("expected agentpm tool action, got {other:?}"),
        }

        let action = decode_provider_call(
            action_alias("action_1", "external_mcp_tool", "incident-data/search"),
            json!({ "arguments": { "state": "open" } }),
        );
        match action {
            SemanticAction::ExternalMcpTool {
                server,
                tool,
                arguments,
            } => {
                assert_eq!(server, "incident-data");
                assert_eq!(tool, "search");
                assert_eq!(arguments["state"], json!("open"));
            }
            other => panic!("expected external MCP tool action, got {other:?}"),
        }

        let action = decode_provider_call(
            action_alias("action_1", "skill_resource_read", "@zack/handoff-skill"),
            json!({ "resource": "references/architecture.md" }),
        );
        match action {
            SemanticAction::SkillResourceRead { skill, resource } => {
                assert_eq!(skill, "@zack/handoff-skill");
                assert_eq!(resource, "references/architecture.md");
            }
            other => panic!("expected skill resource read action, got {other:?}"),
        }

        let action = decode_provider_call(
            action_alias("action_1", "knowledge_request", "@zack/guide"),
            json!({ "query": "incident handoff" }),
        );
        match action {
            SemanticAction::KnowledgeRequest { package, query, .. } => {
                assert_eq!(package, "@zack/guide");
                assert_eq!(query.as_deref(), Some("incident handoff"));
            }
            other => panic!("expected knowledge request action, got {other:?}"),
        }

        let action = decode_provider_call(
            action_alias("action_1", "memory_read", "@zack/state"),
            json!({ "mode": "key" }),
        );
        match action {
            SemanticAction::MemoryRead {
                package,
                space,
                mode,
                ..
            } => {
                assert_eq!(package, "@zack");
                assert_eq!(space, "state");
                assert_eq!(mode, MemoryReadMode::Key);
            }
            other => panic!("expected memory read action, got {other:?}"),
        }

        let action = decode_provider_call(
            action_alias("action_1", "memory_write", "@zack/state"),
            json!({
                "operation": "upsert",
                "record_type": "summary",
                "content": { "summary": "updated" }
            }),
        );
        match action {
            SemanticAction::MemoryWrite {
                package,
                space,
                operation,
                record_type,
                content,
                ..
            } => {
                assert_eq!(package, "@zack");
                assert_eq!(space, "state");
                assert_eq!(operation, MemoryWriteOperation::Upsert);
                assert_eq!(record_type, "summary");
                assert_eq!(content.as_ref().unwrap()["summary"], json!("updated"));
            }
            other => panic!("expected memory write action, got {other:?}"),
        }
    }

    #[test]
    fn provider_response_preserves_plain_text_as_assistant_content() {
        let response = ProviderResponse {
            text: "plain final text".into(),
            action_calls: Vec::new(),
            usage: RunUsage::default(),
            finish_reason: None,
            metadata: BTreeMap::new(),
        };
        let turn = normalize_provider_response(response, &[]).unwrap();
        assert_eq!(turn.assistant_content.as_deref(), Some("plain final text"));
        assert!(turn.actions.is_empty());
    }

    #[test]
    fn built_in_runtime_rejects_custom_provider_until_service_transport_exists() {
        let Err(err) = BuiltInModelRuntime::from_selection(selection("company-model")) else {
            panic!("expected custom provider rejection");
        };
        assert!(err.message.contains("custom provider transport"));
    }

    #[test]
    fn built_in_runtime_capabilities_follow_selected_provider_model() {
        let runtime = BuiltInModelRuntime::new(
            ModelProviderSelection {
                provider: "openai".into(),
                model: "gpt-4o-mini".into(),
                options: json!({}),
            },
            Box::new(MockModelTransport::new(Vec::new())),
        );
        let capabilities = runtime.capabilities();
        assert!(capabilities.semantic_actions);
        assert!(capabilities.structured_output);
        assert_eq!(capabilities.context_window_tokens, Some(128_000));

        let runtime = BuiltInModelRuntime::new(
            ModelProviderSelection {
                provider: "anthropic".into(),
                model: "claude-3-5-sonnet-latest".into(),
                options: json!({}),
            },
            Box::new(MockModelTransport::new(Vec::new())),
        );
        assert_eq!(runtime.capabilities().context_window_tokens, Some(200_000));

        let runtime = BuiltInModelRuntime::new(
            ModelProviderSelection {
                provider: "ollama".into(),
                model: "custom-local-model".into(),
                options: json!({}),
            },
            Box::new(MockModelTransport::new(Vec::new())),
        );
        assert_eq!(runtime.capabilities().context_window_tokens, None);
    }

    #[test]
    fn process_model_initialization_rejects_mismatched_model_identity() {
        let err = process_model_capabilities_from_initialization(
            &json!({
                "registry_id": "custom-process",
                "model": "other-model",
                "ready": true,
                "capabilities": {
                    "semantic_actions": true,
                    "structured_output": true
                }
            }),
            "custom-process",
            "test-model",
        )
        .unwrap_err();

        assert!(err.message.contains("expected `test-model`"));
    }

    #[test]
    fn built_in_runtime_generate_preserves_ordered_actions_and_usage() {
        let response = ProviderResponse {
            text: json!({
                "assistant_content": "I will use a tool and then complete.",
                "actions": [
                    {
                        "type": "agentpm_tool",
                        "tool": "@zack/search",
                        "arguments": { "q": "incident" }
                    },
                    {
                        "type": "phase_completion",
                        "outcome": "ready",
                        "output": { "summary": "done" }
                    }
                ]
            })
            .to_string(),
            action_calls: Vec::new(),
            usage: usage_from_json(Some(&json!({
                "prompt_tokens": 10,
                "completion_tokens": 5
            }))),
            finish_reason: Some("stop".into()),
            metadata: BTreeMap::from([("provider".into(), json!("test"))]),
        };
        let mut runtime = BuiltInModelRuntime::new(
            selection("openai"),
            Box::new(MockModelTransport::new(vec![response])),
        );
        let turn = runtime.generate(model_request()).unwrap();
        assert_eq!(turn.actions.len(), 2);
        assert!(matches!(
            turn.actions[0].action,
            SemanticAction::AgentPmTool { .. }
        ));
        assert!(matches!(
            turn.actions[1].action,
            SemanticAction::PhaseCompletion { .. }
        ));
        assert_eq!(turn.usage.tokens.input_tokens, Some(10));
        assert_eq!(turn.usage.tokens.output_tokens, Some(5));
        assert_eq!(turn.usage.tokens.total_tokens, Some(15));
    }

    #[test]
    fn built_in_runtime_generate_passes_aliases_to_transport() {
        let diagnostic_prompt = model_request().prompt.render_text();
        assert!(diagnostic_prompt.contains("EFFECTIVE CAPABILITY CATALOG"));
        assert!(
            diagnostic_prompt.contains("- phase_complete [phase_completion] review/completion")
        );

        let transport = SharedMockTransport::new(vec![ProviderResponse {
            text: json!({ "outcome": "ready" }).to_string(),
            action_calls: Vec::new(),
            usage: RunUsage::default(),
            finish_reason: Some("stop".into()),
            metadata: BTreeMap::new(),
        }]);
        let requests = transport.requests.clone();
        let mut runtime = BuiltInModelRuntime::new(selection("openai"), Box::new(transport));
        let turn = runtime.generate(model_request()).unwrap();
        assert_eq!(turn.actions.len(), 1);

        let requests = requests.borrow();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].action_aliases.get("phase_complete"),
            Some(&"review/completion".to_string())
        );
        assert_eq!(requests[0].actions.len(), 1);
        assert_eq!(requests[0].actions[0].alias, "phase_complete");
        assert_eq!(requests[0].actions[0].action_kind, "phase_completion");
        assert_eq!(
            requests[0].actions[0].parameters["properties"]["outcome"]["enum"],
            json!(["ready"])
        );
        assert!(requests[0].prompt.contains("HARNESS CONTROL"));
        assert!(!requests[0].prompt.contains("EFFECTIVE CAPABILITY CATALOG"));
        assert!(
            !requests[0]
                .prompt
                .contains("- phase_complete [phase_completion] review/completion")
        );
    }

    #[test]
    fn provider_action_tools_use_semantic_aliases_and_fixed_target_descriptions() {
        let mut request = model_request();
        let memory = MemorySpaceRuntimeSnapshot {
            package: "@zack/m16-reference-memory".into(),
            package_version: "0.1.0".into(),
            space: "notes".into(),
            model: MemorySpaceModel::Collection,
            description: "Durable note collection.".into(),
            root: None,
            runtime: "local".into(),
            source: "agent_binding".into(),
            state: "available".into(),
            readiness_reason: None,
            binding_scope: "global".into(),
            scope_keys: vec!["user".into()],
            retrieval_modes: vec![MemoryRetrievalMode::Key],
            semantic: None,
            append_only: false,
            record_types: vec![MemoryRecordTypeRuntimeSnapshot {
                name: "note".into(),
                schema_version: "1.0.0".into(),
                content_schema: json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "body": { "type": "string" }
                    },
                    "required": ["body"]
                }),
            }],
        };
        request.effective_phase.active_memory.push(memory);
        request
            .effective_phase
            .capability_catalog
            .push(CapabilityDescriptor {
                action_kind: "memory_write".into(),
                identity: "@zack/m16-reference-memory/notes".into(),
                description: "Write durable notes.".into(),
                source: "agent_binding".into(),
            });
        request.prompt.action_aliases =
            crate::harness_runtime::model::provider_action_aliases(&request.effective_phase);

        let tools = provider_action_tools(&request);

        let phase_tool = tools
            .iter()
            .find(|tool| tool.action_kind == "phase_completion")
            .expect("phase completion tool");
        assert_eq!(phase_tool.alias, "phase_complete");
        assert_eq!(phase_tool.identity, "review/completion");
        assert!(
            phase_tool
                .description
                .contains("phase objective is satisfied")
        );
        let memory_tool = tools
            .iter()
            .find(|tool| {
                tool.action_kind == "memory_write" && tool.alias.contains("create_or_upsert_note")
            })
            .expect("memory write tool");
        assert!(
            memory_tool
                .alias
                .starts_with("memory_write_notes_create_or_upsert_note_")
        );
        assert_eq!(memory_tool.identity, "@zack/m16-reference-memory/notes");
        assert!(
            memory_tool
                .description
                .contains("package `@zack/m16-reference-memory`")
        );
        assert!(memory_tool.description.contains("space `notes`"));
        assert!(memory_tool.description.contains("fixed record_type `note`"));
        assert!(
            memory_tool
                .description
                .contains("Fixed write shape: create/upsert")
        );

        let transport = SharedMockTransport::new(vec![ProviderResponse {
            text: json!({ "outcome": "ready" }).to_string(),
            action_calls: Vec::new(),
            usage: RunUsage::default(),
            finish_reason: Some("stop".into()),
            metadata: BTreeMap::new(),
        }]);
        let requests = transport.requests.clone();
        let mut runtime = BuiltInModelRuntime::new(selection("openai"), Box::new(transport));
        runtime.generate(request).unwrap();
        let requests = requests.borrow();
        assert!(!requests[0].prompt.contains("EFFECTIVE CAPABILITY CATALOG"));
        assert!(!requests[0].prompt.contains(&memory_tool.alias));
        assert!(
            requests[0]
                .actions
                .iter()
                .any(|action| action.alias == memory_tool.alias
                    && action.action_kind == "memory_write"
                    && action.identity == "@zack/m16-reference-memory/notes")
        );
        assert_eq!(
            requests[0].action_aliases.get(&memory_tool.alias),
            Some(&"@zack/m16-reference-memory/notes".to_string())
        );
    }

    #[test]
    fn memory_provider_alias_round_trip_selects_the_exact_surface() {
        let mut request = model_request();
        for (space, model) in [
            ("current_note", MemorySpaceModel::Document),
            ("notes", MemorySpaceModel::Collection),
        ] {
            request
                .effective_phase
                .active_memory
                .push(MemorySpaceRuntimeSnapshot {
                    package: "@zack/m16-reference-memory".into(),
                    package_version: "0.1.0".into(),
                    space: space.into(),
                    model,
                    description: format!("{space} memory"),
                    root: None,
                    runtime: "local".into(),
                    source: "agent_binding".into(),
                    state: "available".into(),
                    readiness_reason: None,
                    binding_scope: "global".into(),
                    scope_keys: vec!["user".into()],
                    retrieval_modes: vec![MemoryRetrievalMode::Key],
                    semantic: None,
                    append_only: false,
                    record_types: vec![MemoryRecordTypeRuntimeSnapshot {
                        name: "note".into(),
                        schema_version: "1.0.0".into(),
                        content_schema: json!({ "type": "object", "additionalProperties": true }),
                    }],
                });
            request
                .effective_phase
                .capability_catalog
                .push(CapabilityDescriptor {
                    action_kind: "memory_write".into(),
                    identity: format!("@zack/m16-reference-memory/{space}"),
                    description: format!("Write {space}."),
                    source: "agent_binding".into(),
                });
        }
        request.prompt.action_aliases =
            crate::harness_runtime::model::provider_action_aliases(&request.effective_phase);
        let notes_alias = request
            .prompt
            .action_aliases
            .iter()
            .find(|alias| {
                alias.action_kind == "memory_write"
                    && alias.identity == "@zack/m16-reference-memory/notes"
                    && alias.provider_shape.as_deref() == Some("create_or_upsert_note")
            })
            .expect("notes alias");
        assert!(
            notes_alias
                .alias
                .starts_with("memory_write_notes_create_or_upsert_note_")
        );

        let action = semantic_action_from_provider_call(
            &ProviderActionCall {
                id: None,
                alias: notes_alias.alias.clone(),
                arguments: json!({
                    "operation": "create",
                    "record_type": "note",
                    "content": { "body": "launch readiness" }
                }),
            },
            &request.prompt.action_aliases,
        )
        .unwrap();

        match action {
            SemanticAction::MemoryWrite {
                package,
                space,
                record_type,
                ..
            } => {
                assert_eq!(package, "@zack/m16-reference-memory");
                assert_eq!(space, "notes");
                assert_eq!(record_type, "note");
            }
            other => panic!("expected memory write, got {other:?}"),
        }
    }

    #[test]
    fn memory_read_provider_actions_advertise_flat_shape_schemas() {
        let mut request = model_request();
        request.effective_phase.active_memory.push(memory_snapshot(
            "@zack/m16-reference-memory",
            "notes",
            MemorySpaceModel::Collection,
            vec![
                MemoryRetrievalMode::Key,
                MemoryRetrievalMode::Chronological,
                MemoryRetrievalMode::Filter,
                MemoryRetrievalMode::FullText,
                MemoryRetrievalMode::Semantic,
            ],
        ));
        request.effective_phase.active_memory[0].record_types[0].content_schema = json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "body": { "type": "string" },
                "tag": { "type": "string" },
                "tags": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "name": { "type": "string" }
                        }
                    }
                }
            }
        });
        request.effective_phase.active_memory.push(memory_snapshot(
            "@zack/m16-reference-memory",
            "current_note",
            MemorySpaceModel::Document,
            vec![MemoryRetrievalMode::Key],
        ));
        for identity in [
            "@zack/m16-reference-memory/notes",
            "@zack/m16-reference-memory/current_note",
        ] {
            request
                .effective_phase
                .capability_catalog
                .push(CapabilityDescriptor {
                    action_kind: "memory_read".into(),
                    identity: identity.into(),
                    description: format!("Read {identity}."),
                    source: "agent_binding".into(),
                });
        }
        request.prompt.action_aliases =
            crate::harness_runtime::model::provider_action_aliases(&request.effective_phase);

        let shape_schema = |shape: &str| {
            let alias = request
                .prompt
                .action_aliases
                .iter()
                .find(|alias| {
                    alias.action_kind == "memory_read"
                        && alias.identity == "@zack/m16-reference-memory/notes"
                        && alias.provider_shape.as_deref() == Some(shape)
                })
                .unwrap_or_else(|| panic!("missing {shape} alias"));
            action_parameters_schema(alias, &request)
        };

        let key_record = shape_schema("key_record");
        assert_eq!(key_record["required"], json!(["record_id"]));
        assert!(key_record["properties"].get("mode").is_none());
        assert!(key_record["properties"].get("query").is_none());
        assert!(key_record["properties"].get("filter").is_none());

        let chronological = shape_schema("chronological");
        assert!(chronological.get("required").is_none());
        assert!(chronological["properties"].get("limit").is_some());
        assert!(chronological["properties"].get("record_id").is_none());
        assert!(chronological["properties"].get("query").is_none());
        assert!(chronological["properties"].get("filter").is_none());

        let filter = shape_schema("filter");
        assert_eq!(filter["required"], json!(["filter"]));
        assert!(filter["properties"].get("filter").is_some());
        assert_eq!(
            filter["properties"]["filter"]["additionalProperties"],
            json!(false)
        );
        assert_eq!(
            filter["properties"]["filter"]["properties"]
                .as_object()
                .expect("filter properties")
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["body", "tag", "tags", "tags.name"]
        );
        assert!(
            filter["properties"]["filter"]["properties"]
                .get("status")
                .is_none()
        );
        assert!(filter["properties"].get("record_id").is_none());
        assert!(filter["properties"].get("query").is_none());

        let full_text = shape_schema("full_text");
        assert_eq!(full_text["required"], json!(["query"]));
        assert!(full_text["properties"].get("query").is_some());
        assert!(full_text["properties"].get("filter").is_none());
        assert!(full_text["properties"].get("record_id").is_none());

        let semantic = shape_schema("semantic");
        assert_eq!(semantic["required"], json!(["query"]));
        assert!(semantic["properties"].get("query").is_some());
        assert!(semantic["properties"].get("filter").is_some());
        assert_eq!(
            semantic["properties"]["filter"]["properties"]["tags.name"]["description"],
            json!("Exact value to match at this declared durable content path.")
        );
        assert!(semantic["properties"].get("record_id").is_none());

        let key_document_alias = request
            .prompt
            .action_aliases
            .iter()
            .find(|alias| {
                alias.identity == "@zack/m16-reference-memory/current_note"
                    && alias.provider_shape.as_deref() == Some("key_document")
            })
            .expect("document key alias");
        let key_document = action_parameters_schema(key_document_alias, &request);
        assert!(key_document.get("required").is_none());
        assert!(key_document["properties"].get("record_id").is_none());
        assert!(key_document["properties"].get("query").is_none());
        assert!(key_document["properties"].get("filter").is_none());

        let tools = provider_action_tools(&request);
        assert_eq!(
            tools
                .iter()
                .filter(|tool| tool.action_kind == "memory_read")
                .count(),
            6
        );
        assert!(tools.iter().any(|tool| {
            tool.alias == key_document_alias.alias
                && tool.description.contains("Fixed read shape: key read")
        }));
        let key_record_tool = tools
            .iter()
            .find(|tool| {
                tool.action_kind == "memory_read"
                    && tool.identity == "@zack/m16-reference-memory/notes"
                    && tool.alias.contains("key_record")
            })
            .expect("key record provider tool");
        assert!(key_record_tool.description.contains("notes memory"));
        assert!(
            key_record_tool
                .description
                .contains("Fixed read shape: key read by existing record_id")
        );
        assert!(!key_record_tool.description.contains("Modes:"));
        assert!(
            !key_record_tool
                .description
                .contains("key requires record_id")
        );
    }

    #[test]
    fn memory_filter_provider_schema_enumerates_only_declared_paths() {
        let mut request = model_request();
        request.effective_phase.active_memory.push(memory_snapshot(
            "@zack/memory",
            "notes",
            MemorySpaceModel::Collection,
            vec![MemoryRetrievalMode::Filter],
        ));
        request.effective_phase.active_memory[0].record_types[0].content_schema = json!({
            "type": "object",
            "$defs": {
                "Owner": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "team": { "type": "string" }
                    }
                }
            },
            "additionalProperties": false,
            "properties": {
                "body": { "type": "string" },
                "tags": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "name": { "type": "string" }
                        }
                    }
                },
                "owner": { "$ref": "#/$defs/Owner" }
            }
        });
        request
            .effective_phase
            .capability_catalog
            .push(CapabilityDescriptor {
                action_kind: "memory_read".into(),
                identity: "@zack/memory/notes".into(),
                description: "Read notes.".into(),
                source: "agent_binding".into(),
            });
        request.prompt.action_aliases =
            crate::harness_runtime::model::provider_action_aliases(&request.effective_phase);

        let filter_alias = request
            .prompt
            .action_aliases
            .iter()
            .find(|alias| {
                alias.action_kind == "memory_read"
                    && alias.identity == "@zack/memory/notes"
                    && alias.provider_shape.as_deref() == Some("filter")
            })
            .expect("filter alias");
        let schema = action_parameters_schema(filter_alias, &request);
        let filter_schema = &schema["properties"]["filter"];

        assert_eq!(filter_schema["additionalProperties"], json!(false));
        assert_eq!(filter_schema["minProperties"], json!(1));
        assert_eq!(
            filter_schema["properties"]
                .as_object()
                .expect("filter properties")
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["body", "owner", "owner.team", "tags", "tags.name"]
        );
        assert!(
            filter_schema["properties"]
                .as_object()
                .expect("filter properties")
                .get("status")
                .is_none(),
            "undeclared filter path should not be representable in the closed provider schema"
        );
    }

    #[test]
    fn memory_read_provider_shape_aliases_round_trip_to_canonical_actions() {
        let aliases = vec![
            ActionAlias {
                alias: "memory_read_notes_key_record_note_abcd1234".into(),
                action_kind: "memory_read".into(),
                identity: "@zack/memory/notes".into(),
                provider_shape: Some("key_record".into()),
                mcp_server: None,
                mcp_tool: None,
            },
            ActionAlias {
                alias: "memory_read_notes_semantic_note_abcd1234".into(),
                action_kind: "memory_read".into(),
                identity: "@zack/memory/notes".into(),
                provider_shape: Some("semantic".into()),
                mcp_server: None,
                mcp_tool: None,
            },
        ];

        let key_action = semantic_action_from_provider_call(
            &ProviderActionCall {
                id: None,
                alias: aliases[0].alias.clone(),
                arguments: json!({ "record_id": "mem-1", "record_type": "note" }),
            },
            &aliases,
        )
        .expect("key alias should decode");
        match key_action {
            SemanticAction::MemoryRead {
                package,
                space,
                mode,
                record_id,
                ..
            } => {
                assert_eq!(package, "@zack/memory");
                assert_eq!(space, "notes");
                assert_eq!(mode, MemoryReadMode::Key);
                assert_eq!(record_id.as_deref(), Some("mem-1"));
            }
            other => panic!("expected MemoryRead, got {other:?}"),
        }

        let semantic_action = semantic_action_from_provider_call(
            &ProviderActionCall {
                id: None,
                alias: aliases[1].alias.clone(),
                arguments: json!({
                    "query": "launch readiness",
                    "filter": { "tag": "release" },
                    "limit": 3
                }),
            },
            &aliases,
        )
        .expect("semantic alias should decode");
        match semantic_action {
            SemanticAction::MemoryRead {
                mode,
                query,
                filter,
                limit,
                ..
            } => {
                assert_eq!(mode, MemoryReadMode::Semantic);
                assert_eq!(query.as_deref(), Some("launch readiness"));
                assert_eq!(filter.get("tag"), Some(&json!("release")));
                assert_eq!(limit, Some(3));
            }
            other => panic!("expected MemoryRead, got {other:?}"),
        }
    }

    #[test]
    fn memory_write_provider_shape_aliases_round_trip_to_canonical_actions() {
        let aliases = vec![
            action_alias_with_shape(
                "memory_write_notes_create_or_upsert_note_abcd1234",
                "memory_write",
                "@zack/memory/notes",
                "create_or_upsert_note",
            ),
            action_alias_with_shape(
                "memory_write_notes_update_note_abcd1234",
                "memory_write",
                "@zack/memory/notes",
                "update_note",
            ),
            action_alias_with_shape(
                "memory_write_notes_delete_or_archive_abcd1234",
                "memory_write",
                "@zack/memory/notes",
                "delete_or_archive",
            ),
        ];

        let create = semantic_action_from_provider_call(
            &ProviderActionCall {
                id: None,
                alias: aliases[0].alias.clone(),
                arguments: json!({
                    "operation": "upsert",
                    "content": { "body": "launch readiness" }
                }),
            },
            &aliases,
        )
        .expect("create/upsert alias should decode");
        match create {
            SemanticAction::MemoryWrite {
                operation,
                record_type,
                record_id,
                content,
                ..
            } => {
                assert_eq!(operation, MemoryWriteOperation::Upsert);
                assert_eq!(record_type, "note");
                assert!(record_id.is_none());
                assert_eq!(content.unwrap()["body"], json!("launch readiness"));
            }
            other => panic!("expected MemoryWrite, got {other:?}"),
        }

        let update = semantic_action_from_provider_call(
            &ProviderActionCall {
                id: None,
                alias: aliases[1].alias.clone(),
                arguments: json!({
                    "operation": "update",
                    "record_id": "mem-1",
                    "record_type": "note",
                    "content": { "body": "updated" }
                }),
            },
            &aliases,
        )
        .expect("update alias should decode");
        match update {
            SemanticAction::MemoryWrite {
                operation,
                record_type,
                record_id,
                ..
            } => {
                assert_eq!(operation, MemoryWriteOperation::Update);
                assert_eq!(record_type, "note");
                assert_eq!(record_id.as_deref(), Some("mem-1"));
            }
            other => panic!("expected MemoryWrite, got {other:?}"),
        }

        let delete = semantic_action_from_provider_call(
            &ProviderActionCall {
                id: None,
                alias: aliases[2].alias.clone(),
                arguments: json!({
                    "operation": "archive",
                    "record_id": "mem-1",
                    "record_type": "summary"
                }),
            },
            &aliases,
        )
        .expect("delete/archive alias should decode");
        match delete {
            SemanticAction::MemoryWrite {
                operation,
                record_type,
                record_id,
                content,
                ..
            } => {
                assert_eq!(operation, MemoryWriteOperation::Archive);
                assert_eq!(record_type, "summary");
                assert_eq!(record_id.as_deref(), Some("mem-1"));
                assert!(content.is_none());
            }
            other => panic!("expected MemoryWrite, got {other:?}"),
        }

        let err = semantic_action_from_provider_call(
            &ProviderActionCall {
                id: None,
                alias: aliases[0].alias.clone(),
                arguments: json!({
                    "operation": "create",
                    "record_type": "summary",
                    "content": { "summary": "wrong type" }
                }),
            },
            &aliases,
        )
        .expect_err("contradictory fixed record_type should fail");
        assert!(err.message.contains("fixes record_type `note`"));

        let append_only_create = action_alias_with_shape(
            "memory_write_notes_create_only_or_upsert_note_abcd1234",
            "memory_write",
            "@zack/memory/notes",
            "create_only_or_upsert_note",
        );
        let action = semantic_action_from_provider_call(
            &ProviderActionCall {
                id: None,
                alias: append_only_create.alias.clone(),
                arguments: json!({
                    "operation": "create",
                    "content": { "body": "append-only" }
                }),
            },
            std::slice::from_ref(&append_only_create),
        )
        .expect("append-only create shape should decode");
        match action {
            SemanticAction::MemoryWrite {
                operation,
                record_type,
                ..
            } => {
                assert_eq!(operation, MemoryWriteOperation::Create);
                assert_eq!(record_type, "or_upsert_note");
            }
            other => panic!("expected MemoryWrite, got {other:?}"),
        }
        let err = semantic_action_from_provider_call(
            &ProviderActionCall {
                id: None,
                alias: append_only_create.alias.clone(),
                arguments: json!({
                    "operation": "upsert",
                    "content": { "body": "append-only" }
                }),
            },
            &[append_only_create],
        )
        .expect_err("append-only create shape should not permit upsert");
        assert!(err.message.contains("does not permit operation `upsert`"));
    }

    #[test]
    fn native_provider_prompt_omits_catalog_for_supported_built_in_providers() {
        for provider in ["openai", "anthropic", "ollama"] {
            let transport = SharedMockTransport::new(vec![ProviderResponse {
                text: json!({ "outcome": "ready" }).to_string(),
                action_calls: Vec::new(),
                usage: RunUsage::default(),
                finish_reason: Some("stop".into()),
                metadata: BTreeMap::new(),
            }]);
            let requests = transport.requests.clone();
            let mut runtime = BuiltInModelRuntime::new(selection(provider), Box::new(transport));
            let mut request = model_request();
            request.model = Some(selection(provider));
            let snapshot = runtime.inspect_request(&request).unwrap();
            assert_eq!(snapshot.runtime_kind, "built_in", "provider {provider}");
            assert_eq!(
                snapshot.request_kind, "provider_wire_request",
                "provider {provider}"
            );
            assert_eq!(snapshot.provider, provider, "provider {provider}");
            assert_eq!(snapshot.structured_actions, Some(1), "provider {provider}");
            assert!(
                !snapshot.capability_catalog_in_prompt,
                "provider {provider}"
            );
            assert!(
                !snapshot.prompt.contains("EFFECTIVE CAPABILITY CATALOG"),
                "provider {provider}"
            );

            runtime.generate(request).unwrap();

            let requests = requests.borrow();
            assert_eq!(requests.len(), 1, "provider {provider}");
            assert_eq!(requests[0].actions.len(), 1, "provider {provider}");
            assert!(
                requests[0].prompt.contains("HARNESS CONTROL"),
                "provider {provider}"
            );
            assert!(
                !requests[0].prompt.contains("EFFECTIVE CAPABILITY CATALOG"),
                "provider {provider}"
            );
            assert!(
                !requests[0]
                    .prompt
                    .contains("- phase_complete [phase_completion] review/completion"),
                "provider {provider}"
            );
        }
    }

    #[test]
    fn provider_request_snapshot_reports_open_schema_fallback_diagnostics() {
        let runtime = BuiltInModelRuntime::new(
            selection("openai"),
            Box::new(MockModelTransport::new(vec![])),
        );
        let mut request = model_request();
        request.model = Some(selection("openai"));
        request.prompt.action_aliases = vec![
            action_alias(
                "agentpm_tool_missing_schema_abcd1234",
                "agentpm_tool",
                "@zack/missing-tool",
            ),
            action_alias(
                "knowledge_request_empty_docs_abcd1234",
                "knowledge_request",
                "@zack/empty-context",
            ),
            action_alias(
                "memory_write_missing_record_types_abcd1234",
                "memory_write",
                "@zack/memory/notes",
            ),
            action_alias(
                "mcp_tool_lookup_ref_schema_abcd1234",
                "external_mcp_tool",
                "mcp:search/lookup",
            ),
        ];
        request
            .effective_phase
            .capability_catalog
            .push(CapabilityDescriptor {
                action_kind: "agentpm_tool".into(),
                identity: "@zack/missing-tool".into(),
                description: "Missing tool metadata.".into(),
                source: "agent_binding".into(),
            });
        request
            .effective_phase
            .capability_catalog
            .push(CapabilityDescriptor {
                action_kind: "knowledge_request".into(),
                identity: "@zack/empty-context".into(),
                description: "Empty context Knowledge.".into(),
                source: "agent_binding".into(),
            });
        request
            .effective_phase
            .capability_catalog
            .push(CapabilityDescriptor {
                action_kind: "memory_write".into(),
                identity: "@zack/memory/notes".into(),
                description: "Write notes.".into(),
                source: "agent_binding".into(),
            });
        request
            .effective_phase
            .capability_catalog
            .push(CapabilityDescriptor {
                action_kind: "external_mcp_tool".into(),
                identity: "mcp:search/lookup".into(),
                description: "Lookup over imported MCP.".into(),
                source: "harness_config".into(),
            });
        request.effective_phase.active_mcp_tools.push(
            crate::harness_runtime::McpImportRuntimeSnapshot {
                server_id: "search".into(),
                tool_name: "lookup".into(),
                identity: "mcp:search/lookup".into(),
                description: "Lookup over imported MCP.".into(),
                input_schema: json!({
                    "$ref": "#/$defs/LookupArgs",
                    "$defs": {
                        "LookupArgs": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "query": { "type": "string" }
                            },
                            "required": ["query"]
                        }
                    }
                }),
                transport: "http".into(),
                scopes: vec!["global".into()],
                endpoint: Some("https://mcp.example.com/mcp".into()),
                state: "available".into(),
                readiness_reason: None,
                source: "harness_config".into(),
            },
        );
        let mut empty_context = context_knowledge_snapshot("@zack/empty-context");
        empty_context.documents.clear();
        request.effective_phase.active_knowledge.push(empty_context);
        request
            .effective_phase
            .active_memory
            .push(MemorySpaceRuntimeSnapshot {
                package: "@zack/memory".into(),
                package_version: "0.1.0".into(),
                space: "notes".into(),
                model: MemorySpaceModel::Collection,
                description: "Notes.".into(),
                root: None,
                runtime: "local".into(),
                source: "agent_binding".into(),
                state: "available".into(),
                readiness_reason: None,
                binding_scope: "global".into(),
                scope_keys: vec!["user".into()],
                retrieval_modes: vec![MemoryRetrievalMode::Key],
                semantic: None,
                append_only: false,
                record_types: Vec::new(),
            });

        let snapshot = runtime.inspect_request(&request).expect("snapshot");

        assert!(
            snapshot
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains("Tool `@zack/missing-tool`"))
        );
        assert!(
            snapshot
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains("Knowledge package `@zack/empty-context`"))
        );
        assert!(
            snapshot
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains("Memory write `@zack/memory/notes`"))
        );
        assert!(snapshot.diagnostics.iter().any(|diagnostic| {
            diagnostic.contains("MCP Tool `mcp:search/lookup`")
                && diagnostic.contains("open object")
                && diagnostic.contains("canonical MCP schema")
        }));
    }

    #[test]
    fn recursive_memory_filter_paths_use_open_provider_schema_with_diagnostic() {
        let runtime = BuiltInModelRuntime::new(
            selection("openai"),
            Box::new(MockModelTransport::new(vec![])),
        );
        let mut request = model_request();
        request.model = Some(selection("openai"));
        request
            .effective_phase
            .capability_catalog
            .push(CapabilityDescriptor {
                action_kind: "memory_read".into(),
                identity: "@zack/memory/notes".into(),
                description: "Read notes.".into(),
                source: "agent_binding".into(),
            });
        let mut memory = memory_snapshot(
            "@zack/memory",
            "notes",
            MemorySpaceModel::Collection,
            vec![MemoryRetrievalMode::Filter],
        );
        memory.record_types[0].content_schema = json!({
            "type": "object",
            "$defs": {
                "node": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "left": { "$ref": "#/$defs/node" },
                        "right": { "$ref": "#/$defs/node" }
                    }
                }
            },
            "properties": {
                "root": { "$ref": "#/$defs/node" }
            }
        });
        request.effective_phase.active_memory.push(memory);
        request.prompt.action_aliases =
            crate::harness_runtime::model::provider_action_aliases(&request.effective_phase);
        let filter_alias = request
            .prompt
            .action_aliases
            .iter()
            .find(|alias| {
                alias.action_kind == "memory_read"
                    && alias.identity == "@zack/memory/notes"
                    && alias.provider_shape.as_deref() == Some("filter")
            })
            .expect("recursive contracts should keep filter action with open fallback schema");

        let schema = action_parameters_schema(filter_alias, &request);
        assert_eq!(
            schema["properties"]["filter"]["additionalProperties"],
            json!(true)
        );
        assert!(
            schema["properties"]["filter"]
                .get("properties")
                .is_none_or(Value::is_null)
        );

        let snapshot = runtime.inspect_request(&request).expect("snapshot");
        assert!(
            snapshot.diagnostics.iter().any(
                |diagnostic| diagnostic.contains("recursive or oversized content filter paths")
            )
        );
    }

    #[test]
    fn zero_declared_filter_paths_omit_filter_action_with_diagnostic() {
        let runtime = BuiltInModelRuntime::new(
            selection("openai"),
            Box::new(MockModelTransport::new(vec![])),
        );
        let mut request = model_request();
        request.model = Some(selection("openai"));
        request
            .effective_phase
            .capability_catalog
            .push(CapabilityDescriptor {
                action_kind: "memory_read".into(),
                identity: "@zack/memory/notes".into(),
                description: "Read notes.".into(),
                source: "agent_binding".into(),
            });
        let mut memory = memory_snapshot(
            "@zack/memory",
            "notes",
            MemorySpaceModel::Collection,
            vec![MemoryRetrievalMode::Filter, MemoryRetrievalMode::Semantic],
        );
        memory.record_types[0].content_schema = json!({
            "type": "object",
            "properties": {}
        });
        request.effective_phase.active_memory.push(memory);
        request.prompt.action_aliases =
            crate::harness_runtime::model::provider_action_aliases(&request.effective_phase);

        assert!(!request.prompt.action_aliases.iter().any(|alias| {
            alias.action_kind == "memory_read" && alias.provider_shape.as_deref() == Some("filter")
        }));
        let semantic_alias = request
            .prompt
            .action_aliases
            .iter()
            .find(|alias| {
                alias.action_kind == "memory_read"
                    && alias.provider_shape.as_deref() == Some("semantic")
            })
            .expect("semantic alias");
        let semantic_schema = action_parameters_schema(semantic_alias, &request);
        assert!(
            semantic_schema["properties"]
                .as_object()
                .expect("semantic properties")
                .get("filter")
                .is_none()
        );
        let semantic_tool = provider_action_tools(&request)
            .into_iter()
            .find(|tool| tool.alias == semantic_alias.alias)
            .expect("semantic provider tool");
        assert!(
            semantic_tool
                .description
                .contains("semantic read; provide query and do not provide record_id or filter")
        );
        assert!(
            !semantic_tool
                .description
                .contains("optionally provide filter")
        );

        let snapshot = runtime.inspect_request(&request).expect("snapshot");
        assert!(
            snapshot
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains("filter action is not advertised"))
        );
    }

    #[test]
    fn native_provider_request_omits_turn_backed_prompt_components() {
        let runtime = BuiltInModelRuntime::new(
            selection("openai"),
            Box::new(MockModelTransport::new(vec![])),
        );
        let mut request = model_request();
        request.prompt.sections = vec![
            PromptSection {
                number: 1,
                title: "HARNESS CONTROL".into(),
                content: "Harness authority: propose semantic actions only.\nRepair feedback from previous turn: add query".into(),
            },
            PromptSection {
                number: 3,
                title: crate::harness_runtime::model::CONSUMER_RUN_CONTEXT_SECTION_TITLE.into(),
                content: "Run input:\nWrite one note.\n\nConsumer Context snapshot:\n  state: NotConfigured".into(),
            },
            PromptSection {
                number: 5,
                title: crate::harness_runtime::model::EFFECTIVE_CAPABILITY_CATALOG_SECTION_TITLE.into(),
                content: "- phase_complete [phase_completion] review/completion".into(),
            },
            PromptSection {
                number: 6,
                title: crate::harness_runtime::model::CURRENT_PHASE_LOCAL_TRANSCRIPT_SECTION_TITLE.into(),
                content: "- UserInput: \"Write one note.\"".into(),
            },
        ];
        request.ordered_turns = vec![
            ModelRequestTurn::UserInput {
                content: "Write one note.".into(),
            },
            ModelRequestTurn::RepairFeedback {
                content: "add query".into(),
            },
        ];

        let provider_request = runtime.provider_request(&request);

        assert_eq!(provider_request.turn_strategy, "native_action_result_turns");
        assert_eq!(provider_request.turns, request.ordered_turns);
        assert!(!provider_request.prompt.contains("Write one note."));
        assert!(
            !provider_request
                .prompt
                .contains("CURRENT PHASE-LOCAL TRANSCRIPT")
        );
        assert!(
            !provider_request
                .prompt
                .contains("EFFECTIVE CAPABILITY CATALOG")
        );
        assert!(
            !provider_request
                .prompt
                .contains("Repair feedback from previous turn: add query")
        );
        assert!(
            provider_request
                .prompt
                .contains("Consumer Context snapshot:")
        );
        assert!(
            request
                .prompt
                .render_text()
                .contains("CURRENT PHASE-LOCAL TRANSCRIPT")
        );
        assert!(request.prompt.render_text().contains("Write one note."));
    }

    #[test]
    fn native_provider_request_preserves_before_model_request_hook_context() {
        let runtime = BuiltInModelRuntime::new(
            selection("openai"),
            Box::new(MockModelTransport::new(vec![])),
        );
        let mut request = model_request();
        request.prompt.sections = vec![
            PromptSection {
                number: 1,
                title: "HARNESS CONTROL".into(),
                content: "Harness authority: propose semantic actions only.\nRepair feedback from previous turn: add query".into(),
            },
            PromptSection {
                number: 3,
                title: crate::harness_runtime::model::CONSUMER_RUN_CONTEXT_SECTION_TITLE.into(),
                content: "Run input:\nWrite one note.\n\nConsumer Context snapshot:\n  state: NotConfigured".into(),
            },
            PromptSection {
                number: 5,
                title: crate::harness_runtime::model::EFFECTIVE_CAPABILITY_CATALOG_SECTION_TITLE
                    .into(),
                content: "- phase_complete [phase_completion] review/completion".into(),
            },
            PromptSection {
                number: 6,
                title: crate::harness_runtime::model::CURRENT_PHASE_LOCAL_TRANSCRIPT_SECTION_TITLE
                    .into(),
                content: "- UserInput: \"Write one note.\"\n- RepairFeedback: \"add query\"".into(),
            },
        ];
        request.ordered_turns = vec![
            ModelRequestTurn::UserInput {
                content: "Write one note.".into(),
            },
            ModelRequestTurn::RepairFeedback {
                content: "add query".into(),
            },
        ];
        apply_before_model_request_decision(
            &mut request,
            BeforeModelRequestDecision {
                context_sections: vec![BeforeModelRequestContextSection {
                    title: "Policy Note".into(),
                    content: "Prefer the safest concise answer.".into(),
                }],
                ..BeforeModelRequestDecision::default()
            },
        )
        .unwrap();

        let provider_request = runtime.provider_request(&request);
        let openai_body = Value::Object(openai_request_body(&provider_request));
        let openai_messages = openai_body["messages"].as_array().unwrap();

        assert_eq!(provider_request.turn_strategy, "native_action_result_turns");
        assert_eq!(provider_request.turns, request.ordered_turns);
        assert!(provider_request.prompt.contains("Hook Context:"));
        assert!(provider_request.prompt.contains("Policy Note:"));
        assert!(
            provider_request
                .prompt
                .contains("Prefer the safest concise answer.")
        );
        assert!(
            !provider_request
                .prompt
                .contains("Run input:\nWrite one note.")
        );
        assert!(
            !provider_request
                .prompt
                .contains("CURRENT PHASE-LOCAL TRANSCRIPT")
        );
        assert!(
            !provider_request
                .prompt
                .contains("Repair feedback from previous turn: add query")
        );
        assert_eq!(openai_messages[0]["role"], "system");
        assert!(
            openai_messages[0]["content"]
                .as_str()
                .unwrap()
                .contains("Prefer the safest concise answer.")
        );
        assert_eq!(openai_messages[1]["role"], "user");
        assert_eq!(openai_messages[1]["content"], "Write one note.");
        assert_eq!(openai_messages[2]["role"], "user");
        assert_eq!(
            openai_messages[2]["content"],
            "Repair feedback from previous turn: add query"
        );
    }

    #[test]
    fn provider_request_retains_prompt_history_when_native_turns_are_unavailable() {
        let runtime = BuiltInModelRuntime::new(
            selection("openai"),
            Box::new(MockModelTransport::new(vec![])),
        );
        let mut request = model_request();
        request.prompt.action_aliases.clear();
        request.effective_phase.capability_catalog.clear();
        request.prompt.sections.push(PromptSection {
            number: 3,
            title: crate::harness_runtime::model::CONSUMER_RUN_CONTEXT_SECTION_TITLE.into(),
            content: "Run input:\nFallback input.".into(),
        });
        request.prompt.sections.push(PromptSection {
            number: 6,
            title: crate::harness_runtime::model::CURRENT_PHASE_LOCAL_TRANSCRIPT_SECTION_TITLE
                .into(),
            content: "- RepairFeedback: \"retry\"".into(),
        });
        request.ordered_turns = vec![ModelRequestTurn::UserInput {
            content: "Fallback input.".into(),
        }];

        let provider_request = runtime.provider_request(&request);

        assert_eq!(provider_request.turn_strategy, "prompt_text_fallback");
        assert!(provider_request.turns.is_empty());
        assert!(
            provider_request
                .prompt
                .contains("Run input:\nFallback input.")
        );
        assert!(
            provider_request
                .prompt
                .contains("CURRENT PHASE-LOCAL TRANSCRIPT")
        );
    }

    #[test]
    fn built_in_provider_bodies_preserve_native_action_result_correlation() {
        let request = correlated_provider_request("openai");
        let openai_body = Value::Object(openai_request_body(&request));
        let openai_messages = openai_body["messages"].as_array().unwrap();
        assert_eq!(openai_messages[0]["role"], "system");
        assert_eq!(openai_messages[1]["role"], "user");
        assert_eq!(openai_messages[1]["content"], "Write one note.");
        assert_eq!(openai_messages[3]["tool_calls"][0]["id"], "call_1");
        assert_eq!(
            openai_messages[3]["tool_calls"][0]["function"]["name"],
            "memory_write_notes_note_abcd1234"
        );
        assert_eq!(openai_messages[4]["role"], "tool");
        assert_eq!(openai_messages[4]["tool_call_id"], "call_1");
        assert_eq!(
            openai_messages[5]["content"],
            "Repair feedback from previous turn: retry with record_type"
        );

        let request = correlated_provider_request("anthropic");
        let anthropic_body = Value::Object(anthropic_request_body(&request));
        assert_eq!(anthropic_body["system"], "Provider control.");
        let anthropic_messages = anthropic_body["messages"].as_array().unwrap();
        assert_eq!(anthropic_messages[0]["role"], "user");
        assert_eq!(anthropic_messages[2]["content"][0]["type"], "tool_use");
        assert_eq!(anthropic_messages[2]["content"][0]["id"], "call_1");
        assert_eq!(
            anthropic_messages[2]["content"][0]["name"],
            "memory_write_notes_note_abcd1234"
        );
        assert_eq!(anthropic_messages[3]["content"][0]["type"], "tool_result");
        assert_eq!(anthropic_messages[3]["content"][0]["tool_use_id"], "call_1");

        let request = correlated_provider_request("ollama");
        let ollama_body = Value::Object(ollama_request_body(&request));
        let ollama_messages = ollama_body["messages"].as_array().unwrap();
        assert_eq!(ollama_messages[3]["tool_calls"][0]["id"], "call_1");
        assert_eq!(ollama_messages[4]["tool_call_id"], "call_1");
    }

    #[test]
    fn split_memory_read_aliases_preserve_native_action_result_correlation() {
        let request = correlated_memory_read_provider_request("openai");
        let openai_body = Value::Object(openai_request_body(&request));
        let openai_messages = openai_body["messages"].as_array().unwrap();
        assert_eq!(openai_messages[2]["tool_calls"][0]["id"], "call_read_1");
        assert_eq!(
            openai_messages[2]["tool_calls"][0]["function"]["name"],
            "memory_read_notes_chronological_note_abcd1234"
        );
        assert_eq!(openai_messages[3]["role"], "tool");
        assert_eq!(openai_messages[3]["tool_call_id"], "call_read_1");

        let request = correlated_memory_read_provider_request("anthropic");
        let anthropic_body = Value::Object(anthropic_request_body(&request));
        let anthropic_messages = anthropic_body["messages"].as_array().unwrap();
        assert_eq!(anthropic_messages[1]["content"][0]["type"], "tool_use");
        assert_eq!(anthropic_messages[1]["content"][0]["id"], "call_read_1");
        assert_eq!(
            anthropic_messages[1]["content"][0]["name"],
            "memory_read_notes_chronological_note_abcd1234"
        );
        assert_eq!(anthropic_messages[2]["content"][0]["type"], "tool_result");
        assert_eq!(
            anthropic_messages[2]["content"][0]["tool_use_id"],
            "call_read_1"
        );
    }

    #[test]
    fn built_in_provider_body_degrades_orphaned_action_result_to_text() {
        let runtime = BuiltInModelRuntime::new(
            selection("openai"),
            Box::new(MockModelTransport::new(vec![])),
        );
        let mut request = model_request();
        request.ordered_turns = model_request_turns(&[TranscriptEntry {
            kind: TranscriptEntryKind::ActionResult,
            content: json!({
                "action_kind": "agentpm_tool",
                "identity": "@zack/search",
                "provider_call_id": "call_orphan",
                "result": { "ok": true }
            }),
            action_succeeded: Some(true),
        }]);

        let provider_request = runtime.provider_request(&request);
        let openai_body = Value::Object(openai_request_body(&provider_request));
        let openai_messages = openai_body["messages"].as_array().unwrap();

        assert!(
            openai_messages
                .iter()
                .all(|message| message["role"] != "tool")
        );
        assert!(openai_messages.iter().any(|message| {
            message["role"] == "user"
                && message["content"].as_str().is_some_and(|content| {
                    content.contains("Previous semantic action result [agentpm_tool @zack/search]")
                })
        }));
    }

    #[test]
    fn ollama_does_not_serialize_tools_without_structured_action_capability() {
        let transport = SharedMockTransport::new(vec![ProviderResponse {
            text: json!({ "outcome": "ready" }).to_string(),
            action_calls: Vec::new(),
            usage: RunUsage::default(),
            finish_reason: Some("stop".into()),
            metadata: BTreeMap::new(),
        }]);
        let requests = transport.requests.clone();
        let mut runtime = BuiltInModelRuntime {
            selection: selection("ollama"),
            capabilities: ModelCapabilityAdvertisement {
                semantic_actions: false,
                structured_output: true,
                ..ModelCapabilityAdvertisement::default()
            },
            transport: Box::new(transport),
        };
        let mut request = model_request();
        request.model = Some(selection("ollama"));

        let err = runtime.generate(request).unwrap_err();

        assert!(err.message.contains("does not advertise"));
        assert!(requests.borrow().is_empty());
    }

    #[test]
    fn built_in_runtime_generate_surfaces_malformed_action_and_transport_failure() {
        let mut runtime = BuiltInModelRuntime::new(
            selection("openai"),
            Box::new(MockModelTransport::new(vec![ProviderResponse {
                text: json!({ "actions": [{ "tool": "@zack/search" }] }).to_string(),
                action_calls: Vec::new(),
                usage: RunUsage::default(),
                finish_reason: None,
                metadata: BTreeMap::new(),
            }])),
        );
        let err = runtime.generate(model_request()).unwrap_err();
        assert!(err.message.contains("missing type"));

        let mut runtime = BuiltInModelRuntime::new(
            selection("openai"),
            Box::new(MockModelTransport::new(Vec::new())),
        );
        let err = runtime.generate(model_request()).unwrap_err();
        assert!(err.message.contains("mock model transport exhausted"));
    }

    #[test]
    fn provider_response_adapters_parse_provider_shapes_and_usage() {
        let openai = provider_response_from_openai(json!({
            "choices": [
                {
                    "message": { "content": "{\"outcome\":\"complete\"}" },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 11,
                "completion_tokens": 7,
                "total_tokens": 18
            }
        }))
        .unwrap();
        assert_eq!(openai.text, "{\"outcome\":\"complete\"}");
        assert_eq!(openai.finish_reason.as_deref(), Some("stop"));
        assert_eq!(openai.usage.tokens.total_tokens, Some(18));
        assert_eq!(openai.metadata["provider"], json!("openai"));

        let anthropic = provider_response_from_anthropic(json!({
            "content": [
                { "type": "text", "text": "first" },
                { "type": "text", "text": "second" }
            ],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 12, "output_tokens": 8 }
        }))
        .unwrap();
        assert_eq!(anthropic.text, "first\nsecond");
        assert_eq!(anthropic.finish_reason.as_deref(), Some("end_turn"));
        assert_eq!(anthropic.usage.tokens.total_tokens, Some(20));
        assert_eq!(anthropic.metadata["provider"], json!("anthropic"));

        let ollama = provider_response_from_ollama(json!({
            "message": { "content": "local response" },
            "done_reason": "stop",
            "prompt_eval_count": 9,
            "eval_count": 4
        }))
        .unwrap();
        assert_eq!(ollama.text, "local response");
        assert_eq!(ollama.finish_reason.as_deref(), Some("stop"));
        assert_eq!(ollama.usage.tokens.input_tokens, Some(9));
        assert_eq!(ollama.usage.tokens.output_tokens, Some(4));
        assert_eq!(ollama.usage.tokens.total_tokens, Some(13));
        assert_eq!(ollama.metadata["provider"], json!("ollama"));
    }

    #[test]
    fn provider_adapters_translate_native_tool_definitions_and_calls() {
        let request = model_request();
        let tools = provider_action_tools(&request);
        assert_eq!(tools.len(), 1);

        let openai_tools = openai_tool_definitions(&tools);
        assert_eq!(openai_tools[0]["type"], "function");
        assert_eq!(openai_tools[0]["function"]["name"], "phase_complete");
        assert_eq!(
            openai_tools[0]["function"]["parameters"]["properties"]["outcome"]["enum"],
            json!(["ready"])
        );

        let anthropic_tools = anthropic_tool_definitions(&tools);
        assert_eq!(anthropic_tools[0]["name"], "phase_complete");
        assert_eq!(
            anthropic_tools[0]["input_schema"]["properties"]["outcome"]["enum"],
            json!(["ready"])
        );

        let openai = provider_response_from_openai(json!({
            "choices": [
                {
                    "message": {
                        "content": null,
                        "tool_calls": [
                            {
                                "type": "function",
                                "function": {
                                    "name": "action_1",
                                    "arguments": "{\"outcome\":\"ready\",\"output\":{\"ok\":true}}"
                                }
                            }
                        ]
                    },
                    "finish_reason": "tool_calls"
                }
            ],
            "usage": { "prompt_tokens": 1, "completion_tokens": 2, "total_tokens": 3 }
        }))
        .unwrap();
        assert_eq!(openai.action_calls.len(), 1);
        assert_eq!(openai.action_calls[0].alias, "action_1");

        let anthropic = provider_response_from_anthropic(json!({
            "content": [
                {
                    "type": "tool_use",
                    "id": "toolu_1",
                    "name": "action_1",
                    "input": { "outcome": "ready", "output": { "ok": true } }
                }
            ],
            "stop_reason": "tool_use",
            "usage": { "input_tokens": 1, "output_tokens": 2 }
        }))
        .unwrap();
        assert_eq!(anthropic.action_calls.len(), 1);
        assert_eq!(anthropic.action_calls[0].alias, "action_1");

        let ollama = provider_response_from_ollama(json!({
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [
                    {
                        "function": {
                            "name": "action_1",
                            "arguments": { "outcome": "ready", "output": { "ok": true } }
                        }
                    }
                ]
            },
            "done_reason": "stop"
        }))
        .unwrap();
        assert_eq!(ollama.action_calls.len(), 1);
        assert_eq!(ollama.action_calls[0].alias, "action_1");
    }

    #[test]
    fn knowledge_request_provider_schema_omits_top_level_any_of_for_openai_tools() {
        let mut request = model_request();
        let context = context_knowledge_snapshot("@zack/manual-context");
        let vector = vector_knowledge_snapshot("@zack/manual-vector");
        request.runtime.knowledge.push(context.clone());
        request.runtime.knowledge.push(vector.clone());
        request.effective_phase.active_knowledge.push(context);
        request.effective_phase.active_knowledge.push(vector);
        request.prompt.action_aliases.push(action_alias(
            "action_2",
            "knowledge_request",
            "@zack/manual-context",
        ));
        request.prompt.action_aliases.push(action_alias(
            "action_3",
            "knowledge_request",
            "@zack/manual-vector",
        ));
        request
            .effective_phase
            .capability_catalog
            .push(CapabilityDescriptor {
                action_kind: "knowledge_request".into(),
                identity: "@zack/manual-context".into(),
                description: "Manual context Knowledge.".into(),
                source: "agent_binding".into(),
            });
        request
            .effective_phase
            .capability_catalog
            .push(CapabilityDescriptor {
                action_kind: "knowledge_request".into(),
                identity: "@zack/manual-vector".into(),
                description: "Manual vector Knowledge.".into(),
                source: "agent_binding".into(),
            });

        let tools = provider_action_tools(&request);
        let openai_tools = openai_tool_definitions(&tools);
        let context_schema = openai_tools
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["function"]["name"] == "action_2")
            .unwrap()
            .pointer("/function/parameters")
            .unwrap();
        let vector_schema = openai_tools
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["function"]["name"] == "action_3")
            .unwrap()
            .pointer("/function/parameters")
            .unwrap();

        assert_eq!(context_schema["type"], "object");
        assert!(context_schema.get("anyOf").is_none());
        assert!(context_schema["properties"].get("document").is_some());
        assert!(context_schema["properties"].get("query").is_none());
        assert_eq!(
            context_schema["properties"]["document"]["enum"],
            json!(["knowledge/docs/overview.md"])
        );
        assert!(
            context_schema["required"]
                .as_array()
                .expect("context required array")
                .contains(&json!("document"))
        );

        assert_eq!(vector_schema["type"], "object");
        assert!(vector_schema.get("anyOf").is_none());
        assert!(vector_schema["properties"].get("query").is_some());
        assert!(vector_schema["properties"].get("document").is_none());
        assert!(
            vector_schema["required"]
                .as_array()
                .expect("vector required array")
                .contains(&json!("query"))
        );
    }

    #[test]
    fn unresolved_knowledge_request_fallback_schema_rejects_empty_arguments() {
        let request = model_request();
        let schema = action_parameters_schema(
            &action_alias("action_2", "knowledge_request", "@zack/missing-knowledge"),
            &request,
        );

        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false);
        assert!(schema.get("anyOf").is_none());
        assert_eq!(
            schema["properties"]["mode"]["enum"],
            json!(["context_document", "vector_query"])
        );
        assert!(
            schema["required"]
                .as_array()
                .expect("fallback required array")
                .contains(&json!("mode")),
            "fallback Knowledge schema should not permit empty arguments"
        );
    }

    #[test]
    fn process_model_runtime_uses_agentpm_service_semantic_contract() {
        let temp = std::env::temp_dir().join(format!(
            "agentpm-process-model-runtime-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&temp);
        let script = temp.join("model_service.py");
        fs::write(
            &script,
            r#"
import json, sys
for line in sys.stdin:
    msg = json.loads(line)
    if msg["kind"] == "initialize":
        assert msg["payload"]["model"] == "test-model"
        result = {
            "registry_id": "custom-process",
            "model": "test-model",
            "ready": True,
            "capabilities": {
                "semantic_actions": True,
                "structured_output": True,
                "multimodal_input": True,
                "context_window_tokens": 32768,
                "usage_reporting": False
            }
        }
        kind = "initialized"
    else:
        assert msg["service"] == "model"
        assert msg["method"] == "generate"
        assert msg["payload"]["request"]["phase_id"] == "review"
        result = {
            "assistant_content": "service model completed",
            "actions": [],
            "usage": {
                "model_calls": 0,
                "tokens": {"input_tokens": 1, "output_tokens": 2, "total_tokens": 3},
                "accepted_semantic_actions": 0,
                "tool_calls": 0,
                "tool_retries": 0,
                "knowledge_requests": 0,
                "memory_requests": 0,
                "embedding_requests": 0,
                "duration_ms": None,
                "cost": {"amount": None, "currency": None}
            },
            "finish_reason": "stop",
            "provider_metadata": {}
        }
        kind = "response"
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
        let mut runtime = ProcessModelRuntime::start(
            selection("custom-process"),
            HarnessImplementation::Process {
                command: "python3".into(),
                args: vec![script.display().to_string()],
                cwd: None,
                env: Vec::new(),
                startup_timeout_ms: 1_000,
                request_timeout_ms: 1_000,
                restart: crate::harness_config::HarnessRestartPolicy::default(),
            },
            temp,
            None,
        )
        .unwrap();
        assert!(runtime.capabilities().multimodal_input);
        assert_eq!(runtime.capabilities().context_window_tokens, Some(32768));
        assert!(!runtime.capabilities().usage_reporting);

        let turn = runtime.generate(model_request()).unwrap();

        assert_eq!(
            turn.assistant_content.as_deref(),
            Some("service model completed")
        );
        assert_eq!(turn.finish_reason.as_deref(), Some("stop"));
        assert_eq!(turn.usage.tokens.total_tokens, Some(3));
    }

    fn model_request() -> ModelRequest {
        let capability = CapabilityDescriptor {
            action_kind: "phase_completion".into(),
            identity: "review/completion".into(),
            description: "Complete with ready.".into(),
            source: "loop".into(),
        };
        let mut runtime = RuntimeSnapshot::empty("session-test".into());
        runtime.tools.push(ToolRuntimeSnapshot {
            name: "@zack/search".into(),
            version: "0.1.0".into(),
            description: "Search incidents.".into(),
            root: None,
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "q": { "type": "string" }
                },
                "required": ["q"]
            }),
            state: "available".into(),
            source: "agent_binding".into(),
        });
        runtime.skills.push(SkillRuntimeSnapshot {
            name: "@zack/handoff-skill".into(),
            version: "0.1.0".into(),
            description: "Read handoff resources.".into(),
            root: None,
            resources: vec![
                SkillResourceSnapshot {
                    id: "entrypoint".into(),
                    path: "SKILL.md".into(),
                    kind: "entrypoint".into(),
                },
                SkillResourceSnapshot {
                    id: "references/architecture.md".into(),
                    path: "references/architecture.md".into(),
                    kind: "reference".into(),
                },
            ],
            state: "available".into(),
            source: "agent_binding".into(),
        });
        ModelRequest {
            runtime,
            model: Some(selection("openai")),
            prompt: LogicalPrompt {
                sections: vec![
                    PromptSection {
                        number: 1,
                        title: "HARNESS CONTROL".into(),
                        content: "Harness authority: propose semantic actions only.".into(),
                    },
                    PromptSection {
                        number: 5,
                        title: "EFFECTIVE CAPABILITY CATALOG".into(),
                        content: "- phase_complete [phase_completion] review/completion".into(),
                    },
                ],
                action_aliases: vec![crate::harness_runtime::model::ActionAlias {
                    alias: "phase_complete".into(),
                    action_kind: capability.action_kind.clone(),
                    identity: capability.identity.clone(),
                    provider_shape: None,
                    mcp_server: None,
                    mcp_tool: None,
                }],
                completion: CompletionContract {
                    phase_id: "review".into(),
                    explicit_outcomes: vec!["ready".into()],
                    implicit_complete: false,
                },
                diagnostics: Vec::new(),
            },
            ordered_turns: Vec::new(),
            run_id: "run-test".into(),
            phase_execution_id: "phase-exec-test".into(),
            phase_id: "review".into(),
            phase_objective: "Review the work.".into(),
            run_input: "input".into(),
            prior_phase_results: Vec::new(),
            transcript: Vec::new(),
            effective_phase: EffectivePhase {
                phase_id: "review".into(),
                tools_allowed: None,
                knowledge_allowed: None,
                memory_read_allowed: None,
                memory_write_allowed: None,
                authored_profile_candidates: Vec::new(),
                active_profiles: Vec::new(),
                active_tools: Vec::new(),
                active_mcp_tools: Vec::new(),
                active_skills: Vec::new(),
                active_knowledge: Vec::new(),
                active_memory: Vec::new(),
                active_memory_operations: Vec::new(),
                capability_catalog: vec![capability],
                suppressed_capabilities: Vec::new(),
            },
            repair_feedback: None,
        }
    }

    fn context_knowledge_snapshot(name: &str) -> KnowledgeRuntimeSnapshot {
        KnowledgeRuntimeSnapshot {
            name: name.into(),
            version: "0.1.0".into(),
            mode: "context".into(),
            description: "Manual context Knowledge.".into(),
            root: None,
            source: "agent_binding".into(),
            state: "available".into(),
            runtime: "local".into(),
            readiness_reason: None,
            documents: vec![KnowledgeDocumentSnapshot {
                path: "knowledge/docs/overview.md".into(),
                content_type: Some("text/markdown".into()),
                role: Some("context".into()),
                description: Some("Overview.".into()),
                bytes: Some(32),
                sha256: Some("sha256:test".into()),
            }],
            embedding: None,
            retrieval: None,
        }
    }

    fn vector_knowledge_snapshot(name: &str) -> KnowledgeRuntimeSnapshot {
        KnowledgeRuntimeSnapshot {
            name: name.into(),
            version: "0.1.0".into(),
            mode: "vector".into(),
            description: "Manual vector Knowledge.".into(),
            root: None,
            source: "agent_binding".into(),
            state: "available".into(),
            runtime: "local".into(),
            readiness_reason: None,
            documents: Vec::new(),
            embedding: None,
            retrieval: Some(KnowledgeRetrievalSnapshot {
                strategy: Some("vector".into()),
                default_top_k: Some(2),
                default_score_threshold: None,
                return_citations: Some(true),
            }),
        }
    }

    fn action_alias(alias: &str, action_kind: &str, identity: &str) -> ActionAlias {
        let (mcp_server, mcp_tool) = if action_kind == "external_mcp_tool" {
            identity
                .strip_prefix("mcp:")
                .unwrap_or(identity)
                .split_once('/')
                .map(|(server, tool)| (Some(server.to_string()), Some(tool.to_string())))
                .unwrap_or((None, None))
        } else {
            (None, None)
        };
        ActionAlias {
            alias: alias.into(),
            action_kind: action_kind.into(),
            identity: identity.into(),
            provider_shape: None,
            mcp_server,
            mcp_tool,
        }
    }

    fn action_alias_with_shape(
        alias: &str,
        action_kind: &str,
        identity: &str,
        provider_shape: &str,
    ) -> ActionAlias {
        ActionAlias {
            alias: alias.into(),
            action_kind: action_kind.into(),
            identity: identity.into(),
            provider_shape: Some(provider_shape.into()),
            mcp_server: None,
            mcp_tool: None,
        }
    }

    fn memory_snapshot(
        package: &str,
        space: &str,
        model: MemorySpaceModel,
        retrieval_modes: Vec<MemoryRetrievalMode>,
    ) -> MemorySpaceRuntimeSnapshot {
        MemorySpaceRuntimeSnapshot {
            package: package.into(),
            package_version: "0.1.0".into(),
            space: space.into(),
            model,
            description: format!("{space} memory"),
            root: None,
            runtime: "local".into(),
            source: "agent_binding".into(),
            state: "available".into(),
            readiness_reason: None,
            binding_scope: "global".into(),
            scope_keys: vec!["user".into()],
            retrieval_modes,
            semantic: None,
            append_only: false,
            record_types: vec![MemoryRecordTypeRuntimeSnapshot {
                name: "note".into(),
                schema_version: "1.0.0".into(),
                content_schema: json!({
                    "type": "object",
                    "additionalProperties": true
                }),
            }],
        }
    }

    fn decode_provider_call(alias: ActionAlias, arguments: Value) -> SemanticAction {
        semantic_action_from_provider_call(
            &ProviderActionCall {
                id: None,
                alias: alias.alias.clone(),
                arguments,
            },
            &[alias],
        )
        .expect("provider action call should decode")
    }

    fn correlated_provider_request(provider: &str) -> ProviderRequest {
        ProviderRequest {
            selection: selection(provider),
            prompt: "Provider control.".into(),
            include_capability_catalog: false,
            turn_strategy: "native_action_result_turns".into(),
            turns: vec![
                ModelRequestTurn::UserInput {
                    content: "Write one note.".into(),
                },
                ModelRequestTurn::AssistantContent {
                    content: "Writing now.".into(),
                },
                ModelRequestTurn::SemanticActionCall {
                    provider_call_id: Some("call_1".into()),
                    provider_alias: Some("memory_write_notes_note_abcd1234".into()),
                    action_kind: "memory_write".into(),
                    identity: "@zack/memory/notes".into(),
                    arguments: json!({
                        "operation": "create",
                        "record_type": "note",
                        "content": { "body": "launch" }
                    }),
                },
                ModelRequestTurn::SemanticActionResult {
                    provider_call_id: Some("call_1".into()),
                    action_kind: "memory_write".into(),
                    identity: "@zack/memory/notes".into(),
                    result: json!({ "ok": true, "record_id": "mem-1" }),
                    action_succeeded: Some(true),
                },
                ModelRequestTurn::RepairFeedback {
                    content: "retry with record_type".into(),
                },
            ],
            action_aliases: BTreeMap::from([(
                "memory_write_notes_note_abcd1234".into(),
                "@zack/memory/notes".into(),
            )]),
            actions: vec![ProviderActionTool {
                alias: "memory_write_notes_note_abcd1234".into(),
                action_kind: "memory_write".into(),
                identity: "@zack/memory/notes".into(),
                description: "Write notes.".into(),
                parameters: json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "operation": { "type": "string" },
                        "record_type": { "type": "string" },
                        "content": { "type": "object" }
                    },
                    "required": ["operation", "record_type"]
                }),
            }],
        }
    }

    struct SharedMockTransport {
        responses: Vec<ProviderResponse>,
        requests: Rc<RefCell<Vec<ProviderRequest>>>,
    }

    impl SharedMockTransport {
        fn new(responses: Vec<ProviderResponse>) -> Self {
            Self {
                responses,
                requests: Rc::new(RefCell::new(Vec::new())),
            }
        }
    }

    impl ModelProviderTransport for SharedMockTransport {
        fn send(
            &mut self,
            request: ProviderRequest,
        ) -> Result<ProviderResponse, ModelRuntimeFailure> {
            self.requests.borrow_mut().push(request);
            if self.responses.is_empty() {
                return Err(ModelRuntimeFailure::new("shared mock exhausted"));
            }
            Ok(self.responses.remove(0))
        }
    }

    fn correlated_memory_read_provider_request(provider: &str) -> ProviderRequest {
        ProviderRequest {
            selection: selection(provider),
            prompt: "Provider control.".into(),
            include_capability_catalog: false,
            turn_strategy: "native_action_result_turns".into(),
            turns: vec![
                ModelRequestTurn::UserInput {
                    content: "Check notes.".into(),
                },
                ModelRequestTurn::SemanticActionCall {
                    provider_call_id: Some("call_read_1".into()),
                    provider_alias: Some("memory_read_notes_chronological_note_abcd1234".into()),
                    action_kind: "memory_read".into(),
                    identity: "@zack/memory/notes".into(),
                    arguments: json!({
                        "limit": 5,
                        "record_type": "note"
                    }),
                },
                ModelRequestTurn::SemanticActionResult {
                    provider_call_id: Some("call_read_1".into()),
                    action_kind: "memory_read".into(),
                    identity: "@zack/memory/notes".into(),
                    result: json!({
                        "ok": true,
                        "count": 0,
                        "records": []
                    }),
                    action_succeeded: Some(true),
                },
            ],
            action_aliases: BTreeMap::from([(
                "memory_read_notes_chronological_note_abcd1234".into(),
                "@zack/memory/notes".into(),
            )]),
            actions: vec![ProviderActionTool {
                alias: "memory_read_notes_chronological_note_abcd1234".into(),
                action_kind: "memory_read".into(),
                identity: "@zack/memory/notes".into(),
                description: "Chronological Memory read.".into(),
                parameters: json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "limit": { "type": "integer", "minimum": 1 },
                        "record_type": {
                            "type": "string",
                            "enum": ["note"]
                        }
                    }
                }),
            }],
        }
    }
}
