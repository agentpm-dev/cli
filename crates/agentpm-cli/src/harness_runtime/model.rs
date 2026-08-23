#![allow(dead_code)]

use super::action::SemanticActionProposal;
use crate::harness_engine::{EffectivePhase, PhaseResult};
use crate::harness_observability::RunUsage;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptEntryKind {
    UserInput,
    Assistant,
    ActionResult,
    RepairFeedback,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscriptEntry {
    pub kind: TranscriptEntryKind,
    pub content: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelRequest {
    pub run_id: String,
    pub phase_execution_id: String,
    pub phase_id: String,
    pub phase_objective: String,
    pub run_input: String,
    pub prior_phase_results: Vec<PhaseResult>,
    pub transcript: Vec<TranscriptEntry>,
    pub effective_phase: EffectivePhase,
    pub repair_feedback: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelTurn {
    pub assistant_content: Option<String>,
    pub actions: Vec<SemanticActionProposal>,
    pub usage: RunUsage,
}

pub trait ModelRuntime {
    fn generate(&mut self, request: ModelRequest) -> Result<ModelTurn, ModelRuntimeFailure>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRuntimeFailure {
    pub message: String,
}

impl ModelRuntimeFailure {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Default)]
pub struct ScriptedModelRuntime {
    turns: VecDeque<Result<ModelTurn, ModelRuntimeFailure>>,
    pub requests: Vec<ModelRequest>,
}

impl ScriptedModelRuntime {
    pub fn new(turns: impl IntoIterator<Item = ModelTurn>) -> Self {
        Self {
            turns: turns.into_iter().map(Ok).collect(),
            requests: Vec::new(),
        }
    }

    pub fn with_results(
        turns: impl IntoIterator<Item = Result<ModelTurn, ModelRuntimeFailure>>,
    ) -> Self {
        Self {
            turns: turns.into_iter().collect(),
            requests: Vec::new(),
        }
    }
}

impl ModelRuntime for ScriptedModelRuntime {
    fn generate(&mut self, request: ModelRequest) -> Result<ModelTurn, ModelRuntimeFailure> {
        self.requests.push(request);
        self.turns
            .pop_front()
            .unwrap_or_else(|| Err(ModelRuntimeFailure::new("scripted model exhausted")))
    }
}
