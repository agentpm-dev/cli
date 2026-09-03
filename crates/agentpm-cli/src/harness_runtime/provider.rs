#![allow(dead_code)]

use super::action::{SemanticAction, SemanticActionProposal};
use super::model::{
    ModelCapabilityAdvertisement, ModelProviderSelection, ModelRequest, ModelRuntime,
    ModelRuntimeFailure, ModelTurn,
};
use super::service::{ProcessServiceClient, ProcessServiceConfig, ServiceLifecycleEmitter};
use crate::harness_config::HarnessImplementation;
use crate::harness_observability::RunUsage;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderRequest {
    pub selection: ModelProviderSelection,
    pub prompt: String,
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

    fn generate(&mut self, request: ModelRequest) -> Result<ModelTurn, ModelRuntimeFailure> {
        if !self.capabilities.semantic_actions || !self.capabilities.structured_output {
            return Err(ModelRuntimeFailure::new(
                "selected model runtime does not advertise required Harness semantic action support",
            ));
        }
        let selection = request
            .model
            .clone()
            .unwrap_or_else(|| self.selection.clone());
        let actions = provider_action_tools(&request);
        let provider_request = ProviderRequest {
            selection,
            prompt: request.prompt.render_text(),
            action_aliases: request
                .prompt
                .action_aliases
                .iter()
                .map(|alias| (alias.alias.clone(), alias.identity.clone()))
                .collect(),
            actions,
        };
        let response = self.transport.send(provider_request)?;
        normalize_provider_response(response, &request.prompt.action_aliases)
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
                description: descriptor.description.clone(),
                parameters: action_parameters_schema(alias, request),
            })
        })
        .collect()
}

fn action_parameters_schema(alias: &super::model::ActionAlias, request: &ModelRequest) -> Value {
    match alias.action_kind.as_str() {
        "phase_completion" => phase_completion_parameters_schema(request),
        "agentpm_tool" => agentpm_tool_parameters_schema(alias, request),
        "external_mcp_tool" => json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "arguments": {
                    "type": "object",
                    "description": "JSON arguments for the selected Tool capability."
                }
            },
            "required": ["arguments"]
        }),
        "skill_resource_read" => skill_resource_read_parameters_schema(alias, request),
        "knowledge_request" => knowledge_request_parameters_schema(alias, request),
        "memory_read" => json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "space": { "type": "string", "minLength": 1 }
            },
            "required": ["space"]
        }),
        "memory_write" => json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "space": { "type": "string", "minLength": 1 },
                "content": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Memory record content proposed by the model."
                }
            },
            "required": ["space", "content"]
        }),
        _ => json!({
            "type": "object",
            "additionalProperties": true
        }),
    }
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
                Ok(SemanticActionProposal::new(
                    format!("provider-action-{}", index + 1),
                    semantic_action_from_provider_call(call, aliases)?,
                ))
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
        }),
        "memory_write" => Ok(SemanticAction::MemoryWrite {
            package: required_string(value, "package")?,
            space: required_string(value, "space")?,
            content: value.get("content").cloned().unwrap_or(Value::Null),
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
            let (server, tool) = split_identity(&alias.identity)?;
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
        "memory_read" => Ok(SemanticAction::MemoryRead {
            package: alias.identity.clone(),
            space: required_string(&call.arguments, "space")?,
        }),
        "memory_write" => Ok(SemanticAction::MemoryWrite {
            package: alias.identity.clone(),
            space: required_string(&call.arguments, "space")?,
            content: call
                .arguments
                .get("content")
                .cloned()
                .unwrap_or(Value::Null),
        }),
        other => Err(ModelRuntimeFailure::new(format!(
            "unsupported provider action kind `{other}`"
        ))),
    }
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
        let mut body = object_options(&request.selection.options);
        body.insert("model".into(), json!(request.selection.model));
        body.insert(
            "messages".into(),
            json!([{ "role": "user", "content": request.prompt }]),
        );
        if !request.actions.is_empty() {
            body.insert("tools".into(), openai_tool_definitions(&request.actions));
            body.entry("tool_choice").or_insert(json!("auto"));
        }
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
        let mut body = object_options(&request.selection.options);
        body.insert("model".into(), json!(request.selection.model));
        body.entry("max_tokens").or_insert(json!(1024));
        body.insert(
            "messages".into(),
            json!([{ "role": "user", "content": request.prompt }]),
        );
        if !request.actions.is_empty() {
            body.insert("tools".into(), anthropic_tool_definitions(&request.actions));
        }
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
        let mut body = object_options(&request.selection.options);
        body.insert("model".into(), json!(request.selection.model));
        body.insert("stream".into(), json!(false));
        body.insert(
            "messages".into(),
            json!([{ "role": "user", "content": request.prompt }]),
        );
        if !request.actions.is_empty() {
            body.insert("tools".into(), openai_tool_definitions(&request.actions));
        }
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
    Ok(ProviderActionCall { alias, arguments })
}

fn anthropic_tool_call(value: &Value) -> Result<ProviderActionCall, ModelRuntimeFailure> {
    Ok(ProviderActionCall {
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
    Ok(ProviderActionCall { alias, arguments })
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
    use crate::harness_runtime::model::{
        ActionAlias, CapabilityDescriptor, CompletionContract, KnowledgeDocumentSnapshot,
        KnowledgeRetrievalSnapshot, KnowledgeRuntimeSnapshot, LogicalPrompt, PromptSection,
        RuntimeSnapshot, SkillResourceSnapshot, SkillRuntimeSnapshot, ToolRuntimeSnapshot,
    };
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
    fn provider_response_normalizes_native_action_calls_through_aliases() {
        let response = ProviderResponse {
            text: "I am completing this phase.".into(),
            action_calls: vec![ProviderActionCall {
                alias: "action_1".into(),
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
    }

    #[test]
    fn action_parameter_schemas_use_resolved_tool_and_skill_metadata() {
        let mut request = model_request();
        let guide = vector_knowledge_snapshot("@zack/guide");
        request.runtime.knowledge.push(guide.clone());
        request.effective_phase.active_knowledge.push(guide);
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
                action_alias("action_4", "external_mcp_tool", "incident-data/search"),
                "arguments",
            ),
            (
                action_alias("action_5", "knowledge_request", "@zack/guide"),
                "query",
            ),
            (
                action_alias("action_6", "memory_read", "@zack/state"),
                "space",
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

        let memory_write_schema = action_parameters_schema(
            &action_alias("action_7", "memory_write", "@zack/state"),
            &request,
        );
        assert_eq!(memory_write_schema["type"], "object");
        assert!(
            memory_write_schema["required"]
                .as_array()
                .expect("memory_write required array")
                .contains(&json!("space"))
        );
        assert!(
            memory_write_schema["required"]
                .as_array()
                .expect("memory_write required array")
                .contains(&json!("content"))
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
            json!({ "space": "conversation_state" }),
        );
        match action {
            SemanticAction::MemoryRead { package, space } => {
                assert_eq!(package, "@zack/state");
                assert_eq!(space, "conversation_state");
            }
            other => panic!("expected memory read action, got {other:?}"),
        }

        let action = decode_provider_call(
            action_alias("action_1", "memory_write", "@zack/state"),
            json!({
                "space": "conversation_state",
                "content": { "summary": "updated" }
            }),
        );
        match action {
            SemanticAction::MemoryWrite {
                package,
                space,
                content,
            } => {
                assert_eq!(package, "@zack/state");
                assert_eq!(space, "conversation_state");
                assert_eq!(content["summary"], json!("updated"));
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
            requests[0].action_aliases.get("action_1"),
            Some(&"review/completion".to_string())
        );
        assert_eq!(requests[0].actions.len(), 1);
        assert_eq!(requests[0].actions[0].alias, "action_1");
        assert_eq!(requests[0].actions[0].action_kind, "phase_completion");
        assert_eq!(
            requests[0].actions[0].parameters["properties"]["outcome"]["enum"],
            json!(["ready"])
        );
        assert!(requests[0].prompt.contains("EFFECTIVE CAPABILITY CATALOG"));
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
        assert_eq!(openai_tools[0]["function"]["name"], "action_1");
        assert_eq!(
            openai_tools[0]["function"]["parameters"]["properties"]["outcome"]["enum"],
            json!(["ready"])
        );

        let anthropic_tools = anthropic_tool_definitions(&tools);
        assert_eq!(anthropic_tools[0]["name"], "action_1");
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
                sections: vec![PromptSection {
                    number: 5,
                    title: "EFFECTIVE CAPABILITY CATALOG".into(),
                    content: "- action_1 [phase_completion] review/completion".into(),
                }],
                action_aliases: vec![crate::harness_runtime::model::ActionAlias {
                    alias: "action_1".into(),
                    action_kind: capability.action_kind.clone(),
                    identity: capability.identity.clone(),
                }],
                completion: CompletionContract {
                    phase_id: "review".into(),
                    explicit_outcomes: vec!["ready".into()],
                    implicit_complete: false,
                },
                diagnostics: Vec::new(),
            },
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
                active_skills: Vec::new(),
                active_knowledge: Vec::new(),
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
        ActionAlias {
            alias: alias.into(),
            action_kind: action_kind.into(),
            identity: identity.into(),
        }
    }

    fn decode_provider_call(alias: ActionAlias, arguments: Value) -> SemanticAction {
        semantic_action_from_provider_call(
            &ProviderActionCall {
                alias: alias.alias.clone(),
                arguments,
            },
            &[alias],
        )
        .expect("provider action call should decode")
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
}
