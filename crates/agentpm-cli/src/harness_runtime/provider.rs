#![allow(dead_code)]

use super::action::{SemanticAction, SemanticActionProposal};
use super::model::{
    ModelCapabilityAdvertisement, ModelProviderSelection, ModelRequest, ModelRuntime,
    ModelRuntimeFailure, ModelTurn,
};
use crate::harness_observability::RunUsage;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::env;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderRequest {
    pub selection: ModelProviderSelection,
    pub prompt: String,
    // Reserved for provider-native function/tool alias mapping. The current
    // text-mode contract still expects canonical Harness action identities.
    pub action_aliases: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderResponse {
    pub text: String,
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

fn built_in_capabilities(selection: &ModelProviderSelection) -> ModelCapabilityAdvertisement {
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
        let provider_request = ProviderRequest {
            selection: self.selection.clone(),
            prompt: request.prompt.render_text(),
            // Keep aliases on the provider boundary now so native function/tool
            // adapters can round-trip provider-safe names without changing the
            // ModelRuntime contract later.
            action_aliases: request
                .prompt
                .action_aliases
                .iter()
                .map(|alias| (alias.alias.clone(), alias.identity.clone()))
                .collect(),
        };
        let response = self.transport.send(provider_request)?;
        normalize_provider_response(response)
    }
}

fn normalize_provider_response(
    response: ProviderResponse,
) -> Result<ModelTurn, ModelRuntimeFailure> {
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
            query: required_string(value, "query")?,
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

fn required_string(value: &Value, key: &str) -> Result<String, ModelRuntimeFailure> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ModelRuntimeFailure::new(format!("provider action is missing `{key}`")))
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
        .ok_or_else(|| ModelRuntimeFailure::new("openai response did not contain message content"))?
        .to_string();
    let usage = usage_from_json(value.get("usage"));
    let finish_reason = value
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok(ProviderResponse {
        text,
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
    let text = value
        .get("content")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|text| !text.is_empty())
        .ok_or_else(|| {
            ModelRuntimeFailure::new("anthropic response did not contain text content")
        })?;
    let usage = usage_from_json(value.get("usage"));
    let finish_reason = value
        .get("stop_reason")
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok(ProviderResponse {
        text,
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
        .ok_or_else(|| ModelRuntimeFailure::new("ollama response did not contain message content"))?
        .to_string();
    Ok(ProviderResponse {
        text,
        usage: usage_from_json(Some(&value)),
        finish_reason: value
            .get("done_reason")
            .and_then(Value::as_str)
            .map(str::to_string),
        metadata: BTreeMap::from([("provider".into(), json!("ollama"))]),
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
        CapabilityDescriptor, CompletionContract, LogicalPrompt, PromptSection, RuntimeSnapshot,
    };
    use std::cell::RefCell;
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
            usage: RunUsage::default(),
            finish_reason: Some("stop".into()),
            metadata: BTreeMap::new(),
        };
        let turn = normalize_provider_response(response).unwrap();
        assert_eq!(turn.assistant_content.as_deref(), Some("done"));
        assert_eq!(turn.actions.len(), 1);
        assert!(matches!(
            turn.actions[0].action,
            SemanticAction::PhaseCompletion { .. }
        ));
        assert_eq!(turn.finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn provider_response_preserves_plain_text_as_assistant_content() {
        let response = ProviderResponse {
            text: "plain final text".into(),
            usage: RunUsage::default(),
            finish_reason: None,
            metadata: BTreeMap::new(),
        };
        let turn = normalize_provider_response(response).unwrap();
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
            Some(&"outcome:ready".to_string())
        );
        assert!(requests[0].prompt.contains("EFFECTIVE CAPABILITY CATALOG"));
    }

    #[test]
    fn built_in_runtime_generate_surfaces_malformed_action_and_transport_failure() {
        let mut runtime = BuiltInModelRuntime::new(
            selection("openai"),
            Box::new(MockModelTransport::new(vec![ProviderResponse {
                text: json!({ "actions": [{ "tool": "@zack/search" }] }).to_string(),
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

    fn model_request() -> ModelRequest {
        let capability = CapabilityDescriptor {
            action_kind: "phase_completion".into(),
            identity: "outcome:ready".into(),
            description: "Complete with ready.".into(),
            source: "loop".into(),
        };
        ModelRequest {
            runtime: RuntimeSnapshot::empty("session-test".into()),
            model: Some(selection("openai")),
            prompt: LogicalPrompt {
                sections: vec![PromptSection {
                    number: 5,
                    title: "EFFECTIVE CAPABILITY CATALOG".into(),
                    content: "- action_1 [phase_completion] outcome:ready".into(),
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
                capability_catalog: vec![capability],
            },
            repair_feedback: None,
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
}
