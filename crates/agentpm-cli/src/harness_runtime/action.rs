#![allow(dead_code)]

use super::knowledge::KnowledgeRequestMode;
use crate::harness_observability::{HarnessTerminalStatus, RunUsage};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticActionProposal {
    pub id: String,
    pub action: SemanticAction,
}

impl SemanticActionProposal {
    pub fn new(id: impl Into<String>, action: SemanticAction) -> Self {
        Self {
            id: id.into(),
            action,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryReadMode {
    Key,
    Filter,
    Chronological,
    FullText,
    Semantic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryWriteOperation {
    Create,
    Upsert,
    Update,
    Delete,
    Archive,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SemanticAction {
    AgentPmTool {
        tool: String,
        arguments: Value,
    },
    ExternalMcpTool {
        server: String,
        tool: String,
        arguments: Value,
    },
    SkillResourceRead {
        skill: String,
        resource: String,
    },
    KnowledgeRequest {
        package: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        mode: Option<KnowledgeRequestMode>,
        #[serde(skip_serializing_if = "Option::is_none")]
        document: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        query: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        top_k: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        score_threshold: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        return_citations: Option<bool>,
    },
    MemoryRead {
        package: String,
        space: String,
        mode: MemoryReadMode,
        #[serde(skip_serializing_if = "Option::is_none")]
        record_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        record_type: Option<String>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        filter: BTreeMap<String, Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        query: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        limit: Option<usize>,
    },
    MemoryWrite {
        package: String,
        space: String,
        operation: MemoryWriteOperation,
        record_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        record_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<Value>,
    },
    PhaseCompletion {
        outcome: Option<String>,
        output: Option<Value>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionFailureCategory {
    Resolution,
    Runtime,
    Schema,
    Timeout,
    OutputLimit,
    MalformedOutput,
    SubprocessFailure,
    Other,
}

impl ActionFailureCategory {
    pub fn from_machine_category(value: &str) -> Option<Self> {
        match value {
            "resolution" => Some(Self::Resolution),
            "runtime" => Some(Self::Runtime),
            "schema" => Some(Self::Schema),
            "timeout" => Some(Self::Timeout),
            "output_limit" => Some(Self::OutputLimit),
            "malformed_output" => Some(Self::MalformedOutput),
            "subprocess_failure" => Some(Self::SubprocessFailure),
            "other" => Some(Self::Other),
            _ => None,
        }
    }

    pub fn is_retryable_tool_failure(self) -> bool {
        matches!(self, Self::Timeout | Self::SubprocessFailure)
    }
}

impl SemanticAction {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::AgentPmTool { .. } => "agentpm_tool",
            Self::ExternalMcpTool { .. } => "external_mcp_tool",
            Self::SkillResourceRead { .. } => "skill_resource_read",
            Self::KnowledgeRequest { .. } => "knowledge_request",
            Self::MemoryRead { .. } => "memory_read",
            Self::MemoryWrite { .. } => "memory_write",
            Self::PhaseCompletion { .. } => "phase_completion",
        }
    }

    pub fn identity(&self) -> String {
        match self {
            Self::AgentPmTool { tool, .. } => tool.clone(),
            Self::ExternalMcpTool { server, tool, .. } => format!("{server}/{tool}"),
            Self::SkillResourceRead { skill, resource } => format!("{skill}/{resource}"),
            Self::KnowledgeRequest { package, .. } => package.clone(),
            Self::MemoryRead { package, space, .. } | Self::MemoryWrite { package, space, .. } => {
                format!("{package}/{space}")
            }
            Self::PhaseCompletion { outcome, .. } => {
                outcome.clone().unwrap_or_else(|| "complete".to_string())
            }
        }
    }

    pub fn is_tool_call(&self) -> bool {
        matches!(
            self,
            Self::AgentPmTool { .. } | Self::ExternalMcpTool { .. }
        )
    }

    pub fn is_completion(&self) -> bool {
        matches!(self, Self::PhaseCompletion { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionDispatchResult {
    pub ok: bool,
    pub output: Value,
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_category: Option<ActionFailureCategory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_status: Option<HarnessTerminalStatus>,
    #[serde(default, skip_serializing_if = "is_default_usage")]
    pub usage: RunUsage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_request_duration_ms: Option<u64>,
}

impl ActionDispatchResult {
    pub fn success(output: Value) -> Self {
        Self {
            ok: true,
            output,
            error: None,
            failure_category: None,
            terminal_status: None,
            usage: RunUsage::default(),
            embedding_request_duration_ms: None,
        }
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            output: Value::Null,
            error: Some(message.into()),
            failure_category: None,
            terminal_status: None,
            usage: RunUsage::default(),
            embedding_request_duration_ms: None,
        }
    }

    pub fn failure_with_category(
        category: ActionFailureCategory,
        message: impl Into<String>,
    ) -> Self {
        Self {
            ok: false,
            output: Value::Null,
            error: Some(message.into()),
            failure_category: Some(category),
            terminal_status: None,
            usage: RunUsage::default(),
            embedding_request_duration_ms: None,
        }
    }

    pub fn terminal_failure(status: HarnessTerminalStatus, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            output: json!({ "terminal_status": status }),
            error: Some(message.into()),
            failure_category: None,
            terminal_status: Some(status),
            usage: RunUsage::default(),
            embedding_request_duration_ms: None,
        }
    }

    pub fn with_usage(mut self, usage: RunUsage) -> Self {
        self.usage = usage;
        self
    }

    pub fn with_embedding_request_duration_ms(mut self, duration_ms: u64) -> Self {
        self.embedding_request_duration_ms = Some(duration_ms);
        self
    }
}

fn is_default_usage(usage: &RunUsage) -> bool {
    usage == &RunUsage::default()
}

pub trait ActionDispatcher {
    fn dispatch(&mut self, action: &SemanticAction) -> ActionDispatchResult;
}

#[derive(Default)]
pub struct ScriptedActionDispatcher {
    results: BTreeMap<String, VecDeque<ActionDispatchResult>>,
    pub dispatched: Vec<SemanticAction>,
}

impl ScriptedActionDispatcher {
    pub fn push_result(&mut self, identity: impl Into<String>, result: ActionDispatchResult) {
        self.results
            .entry(identity.into())
            .or_default()
            .push_back(result);
    }
}

impl ActionDispatcher for ScriptedActionDispatcher {
    fn dispatch(&mut self, action: &SemanticAction) -> ActionDispatchResult {
        self.dispatched.push(action.clone());
        self.results
            .get_mut(&action.identity())
            .and_then(VecDeque::pop_front)
            .unwrap_or_else(|| ActionDispatchResult::success(json!({ "ok": true })))
    }
}
