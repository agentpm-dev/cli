#![allow(dead_code)]

use super::action::SemanticActionProposal;
use crate::harness_engine::{EffectivePhase, PhaseResult};
use crate::harness_observability::RunUsage;
use crate::manifest::{
    MemoryRetrievalMode, MemorySpaceModel, ProfileConstraintStrength, ProfileMetadata,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;

pub const CONSUMER_RUN_CONTEXT_SECTION_TITLE: &str = "CONSUMER / RUN CONTEXT";
pub const EFFECTIVE_CAPABILITY_CATALOG_SECTION_TITLE: &str = "EFFECTIVE CAPABILITY CATALOG";
pub const CURRENT_PHASE_LOCAL_TRANSCRIPT_SECTION_TITLE: &str = "CURRENT PHASE-LOCAL TRANSCRIPT";
pub(crate) const SUCCESSFUL_ACTION_RESULT_CONTROL: &str = "If the phase-local transcript already contains successful ActionResults for all requested executable actions, do not propose any of those actions again; propose phase_completion next. For repeated actions, compare action kind, identity, and arguments.";
pub(crate) const SUCCESSFUL_REVIEW_ACTION_RESULT_CONTROL: &str = "If the review transcript already contains successful ActionResults for all requested Memory actions, do not propose any of those actions again; propose persistence_review_complete next. For repeated actions, compare action kind, identity, and arguments.";
pub(crate) const PERSISTENCE_REVIEW_TARGET_SELECTION_CONTROL: &str = "Choose the Memory action whose fixed package, space, and record-type semantics match the intended durable target exactly. Use persistence_review_complete when no further Memory work is needed.";

const PROVIDER_ACTION_ALIAS_MAX_LEN: usize = 64;
const PROVIDER_ACTION_HASH_LEN: usize = 8;

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_succeeded: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageSnapshot {
    pub kind: String,
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsumerContextSnapshot {
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approximate_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelProviderSelection {
    pub provider: String,
    pub model: String,
    pub options: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceReadinessSnapshot {
    pub kind: String,
    pub identity: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolRuntimeSnapshot {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<PathBuf>,
    pub input_schema: Value,
    pub state: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillResourceSnapshot {
    pub id: String,
    pub path: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillRuntimeSnapshot {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<PathBuf>,
    pub resources: Vec<SkillResourceSnapshot>,
    pub state: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeDocumentSnapshot {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeEmbeddingSnapshot {
    pub id: String,
    pub provider: String,
    pub model: String,
    pub dimensions: u64,
    pub metric: String,
    pub normalized: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeRetrievalSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_top_k: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_score_threshold: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_citations: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeRuntimeSnapshot {
    pub name: String,
    pub version: String,
    pub mode: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<PathBuf>,
    pub source: String,
    pub state: String,
    pub runtime: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readiness_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub documents: Vec<KnowledgeDocumentSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<KnowledgeEmbeddingSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retrieval: Option<KnowledgeRetrievalSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryRecordTypeRuntimeSnapshot {
    pub name: String,
    pub schema_version: String,
    pub content_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemorySpaceRuntimeSnapshot {
    pub package: String,
    pub package_version: String,
    pub space: String,
    pub model: MemorySpaceModel,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<PathBuf>,
    pub runtime: String,
    pub source: String,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readiness_reason: Option<String>,
    pub binding_scope: String,
    pub scope_keys: Vec<String>,
    pub retrieval_modes: Vec<MemoryRetrievalMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic: Option<KnowledgeEmbeddingSnapshot>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub append_only: bool,
    pub record_types: Vec<MemoryRecordTypeRuntimeSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryOperationRefRuntimeSnapshot {
    pub space: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryOperationRuntimeSnapshot {
    pub package: String,
    pub package_version: String,
    pub operation: String,
    pub operation_type: String,
    pub description: String,
    pub trigger: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<MemoryOperationRefRuntimeSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<MemoryOperationRefRuntimeSnapshot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<MemoryOperationRefRuntimeSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_handling: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preserve_provenance: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cascade_derived_records: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub referenced_spaces: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<PathBuf>,
    pub runtime: String,
    pub source: String,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readiness_reason: Option<String>,
    pub binding_scope: String,
    pub scope_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCapabilitySnapshot {
    pub kind: String,
    pub identity: String,
    pub scope: String,
    pub source: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileSnapshot {
    pub name: String,
    pub version: String,
    pub profile: ProfileMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProfileBindingSnapshot {
    pub global: Vec<String>,
    pub phases: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeSnapshot {
    pub session_id: String,
    pub workspace_root: PathBuf,
    pub state_dir: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<PackageSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_package: Option<PackageSnapshot>,
    pub package_graph: Vec<PackageSnapshot>,
    pub runtime_config_sources: BTreeMap<String, String>,
    pub runtime_scopes: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumer_context: Option<ConsumerContextSnapshot>,
    pub services: Vec<ServiceReadinessSnapshot>,
    pub hook_registrations: Vec<String>,
    pub profiles: Vec<ProfileSnapshot>,
    pub profile_bindings: ProfileBindingSnapshot,
    pub tools: Vec<ToolRuntimeSnapshot>,
    pub skills: Vec<SkillRuntimeSnapshot>,
    pub knowledge: Vec<KnowledgeRuntimeSnapshot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memory: Vec<MemorySpaceRuntimeSnapshot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memory_operations: Vec<MemoryOperationRuntimeSnapshot>,
    pub capability_candidates: Vec<RuntimeCapabilitySnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelProviderSelection>,
}

impl RuntimeSnapshot {
    pub fn empty(session_id: String) -> Self {
        Self {
            session_id,
            workspace_root: PathBuf::new(),
            state_dir: PathBuf::new(),
            agent: None,
            loop_package: None,
            package_graph: Vec::new(),
            runtime_config_sources: BTreeMap::new(),
            runtime_scopes: BTreeMap::new(),
            consumer_context: None,
            services: Vec::new(),
            hook_registrations: Vec::new(),
            profiles: Vec::new(),
            profile_bindings: ProfileBindingSnapshot::default(),
            tools: Vec::new(),
            skills: Vec::new(),
            knowledge: Vec::new(),
            memory: Vec::new(),
            memory_operations: Vec::new(),
            capability_candidates: Vec::new(),
            model: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityDescriptor {
    pub action_kind: String,
    pub identity: String,
    pub description: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionAlias {
    pub alias: String,
    pub action_kind: String,
    pub identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionContract {
    pub phase_id: String,
    pub explicit_outcomes: Vec<String>,
    pub implicit_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptSection {
    pub number: u8,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogicalPrompt {
    pub sections: Vec<PromptSection>,
    pub action_aliases: Vec<ActionAlias>,
    pub completion: CompletionContract,
    pub diagnostics: Vec<String>,
}

impl LogicalPrompt {
    pub fn render_text(&self) -> String {
        self.render_text_with_options(LogicalPromptRenderOptions::default())
    }

    pub fn render_provider_text(&self, include_capability_catalog: bool) -> String {
        self.render_text_with_options(LogicalPromptRenderOptions {
            include_capability_catalog,
            ..LogicalPromptRenderOptions::default()
        })
    }

    pub fn render_provider_text_with_native_turns(
        &self,
        include_capability_catalog: bool,
    ) -> String {
        self.render_text_with_options(LogicalPromptRenderOptions {
            include_capability_catalog,
            include_run_input: false,
            include_transcript: false,
            include_repair_feedback_control: false,
        })
    }

    pub fn has_capability_catalog_section(&self) -> bool {
        self.sections
            .iter()
            .any(|section| section.title == EFFECTIVE_CAPABILITY_CATALOG_SECTION_TITLE)
    }

    fn render_text_with_options(&self, options: LogicalPromptRenderOptions) -> String {
        let mut rendered = String::new();
        for section in &self.sections {
            if !options.include_capability_catalog
                && section.title == EFFECTIVE_CAPABILITY_CATALOG_SECTION_TITLE
            {
                continue;
            }
            if !options.include_transcript
                && section.title == CURRENT_PHASE_LOCAL_TRANSCRIPT_SECTION_TITLE
            {
                continue;
            }
            if !rendered.is_empty() {
                rendered.push_str("\n\n");
            }
            rendered.push_str(&format!("{}. {}\n", section.number, section.title));
            rendered.push_str(&render_prompt_section_content(section, options));
        }
        rendered
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LogicalPromptRenderOptions {
    include_capability_catalog: bool,
    include_run_input: bool,
    include_transcript: bool,
    include_repair_feedback_control: bool,
}

impl Default for LogicalPromptRenderOptions {
    fn default() -> Self {
        Self {
            include_capability_catalog: true,
            include_run_input: true,
            include_transcript: true,
            include_repair_feedback_control: true,
        }
    }
}

fn render_prompt_section_content(
    section: &PromptSection,
    options: LogicalPromptRenderOptions,
) -> String {
    let mut content = section.content.clone();
    if section.title == CONSUMER_RUN_CONTEXT_SECTION_TITLE && !options.include_run_input {
        content = omit_run_input_from_context_section(&content);
    }
    if section.title == "HARNESS CONTROL" && !options.include_repair_feedback_control {
        content = omit_repair_feedback_control(&content);
    }
    content
}

fn omit_run_input_from_context_section(content: &str) -> String {
    let remaining = content
        .strip_prefix("Run input:\n")
        .and_then(|rest| rest.split_once("\n\nConsumer Context snapshot:"))
        .map(|(_, context)| format!("Consumer Context snapshot:{context}"))
        .unwrap_or_default();
    if remaining.trim().is_empty() {
        "Run input is carried as the leading provider-native user turn.".into()
    } else {
        remaining
    }
}

fn omit_repair_feedback_control(content: &str) -> String {
    content
        .lines()
        .filter(|line| !line.starts_with("Repair feedback from previous turn: "))
        .collect::<Vec<_>>()
        .join("\n")
}

pub struct PromptAssemblyInput<'a> {
    pub purpose: PromptAssemblyPurpose<'a>,
    pub phase_id: &'a str,
    pub phase_objective: &'a str,
    pub explicit_outcomes: &'a [String],
    pub run_input: &'a str,
    pub consumer_context: Option<&'a ConsumerContextSnapshot>,
    pub prior_phase_results: &'a [PhaseResult],
    pub effective_phase: &'a EffectivePhase,
    pub transcript: &'a [TranscriptEntry],
    pub repair_feedback: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptAssemblyPurpose<'a> {
    Phase,
    MemoryWriteReview {
        point: &'a str,
        pending_outcome: &'a str,
    },
}

pub fn assemble_logical_prompt(input: PromptAssemblyInput<'_>) -> LogicalPrompt {
    let implicit_complete = input.explicit_outcomes.is_empty();
    let completion = CompletionContract {
        phase_id: input.phase_id.to_string(),
        explicit_outcomes: input.explicit_outcomes.to_vec(),
        implicit_complete,
    };
    let mut diagnostics = Vec::new();
    let action_aliases = provider_action_aliases(input.effective_phase);

    let outcome_contract = match input.purpose {
        PromptAssemblyPurpose::Phase if implicit_complete => {
            "This phase has implicit outcome `complete`; final assistant text with no action may complete the phase.".to_string()
        }
        PromptAssemblyPurpose::Phase => format!(
            "This phase must complete with exactly one authored outcome: {}.",
            input.explicit_outcomes.join(", ")
        ),
        PromptAssemblyPurpose::MemoryWriteReview {
            point,
            pending_outcome,
        } => format!(
            "This is a bounded Memory write review at `{point}` for pending phase outcome `{pending_outcome}`. Use only authorized Memory actions if useful, then propose persistence_review_complete. {PERSISTENCE_REVIEW_TARGET_SELECTION_CONTROL} Do not propose phase_completion."
        ),
    };
    let mut control = format!(
        "Harness authority: propose semantic actions only; Harness validates and executes them.\nCurrent phase: {}\n{}",
        input.phase_id, outcome_contract
    );
    if let Some(feedback) = input.repair_feedback {
        control.push_str(&format!("\nRepair feedback from previous turn: {feedback}"));
    }
    if transcript_has_successful_action_result(input.transcript) {
        control.push('\n');
        match input.purpose {
            PromptAssemblyPurpose::Phase => control.push_str(SUCCESSFUL_ACTION_RESULT_CONTROL),
            PromptAssemblyPurpose::MemoryWriteReview { .. } => {
                control.push_str(SUCCESSFUL_REVIEW_ACTION_RESULT_CONTROL)
            }
        }
    }

    let mut authored = format!("Phase objective:\n  {}", input.phase_objective);
    let profiles = render_active_profiles(input.effective_phase);
    if !profiles.is_empty() {
        authored.push_str("\n\n");
        authored.push_str(&profiles);
    }
    let loaded_skill_resources = render_loaded_skill_resources(input.transcript);
    if !loaded_skill_resources.is_empty() {
        authored.push_str("\n\n");
        authored.push_str(&loaded_skill_resources);
    }

    let mut context = format!("Run input:\n{}", input.run_input);
    if let Some(consumer_context) = input.consumer_context {
        context.push_str("\n\nConsumer Context snapshot:");
        context.push_str(&format!("\n  state: {}", consumer_context.state));
        if let Some(file) = &consumer_context.file {
            context.push_str(&format!("\n  file: {file}"));
        }
        if let Some(content) = &consumer_context.content {
            context.push_str("\n\n");
            context.push_str(content);
        } else if consumer_context.file.is_some() {
            diagnostics.push("consumer context content is not loaded for this Run".into());
        }
    }

    let cross_phase = if input.prior_phase_results.is_empty() {
        "No prior PhaseResults.".to_string()
    } else {
        input
            .prior_phase_results
            .iter()
            .map(|result| {
                format!(
                    "- step {} phase `{}` outcome `{}` output: {}",
                    result.loop_step_number,
                    result.phase_id,
                    result.outcome,
                    result
                        .output
                        .as_ref()
                        .map(Value::to_string)
                        .unwrap_or_else(|| "null".into())
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let capability_catalog = if input.effective_phase.capability_catalog.is_empty() {
        "No executable capability descriptors are available for this phase.".to_string()
    } else {
        input
            .effective_phase
            .capability_catalog
            .iter()
            .zip(action_aliases.iter())
            .map(|(descriptor, alias)| {
                format!(
                    "- {} [{}] {} — {}",
                    alias.alias,
                    descriptor.action_kind,
                    descriptor.identity,
                    descriptor.description
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let transcript = if input.transcript.is_empty() {
        "No current phase-local transcript entries.".to_string()
    } else {
        input
            .transcript
            .iter()
            .map(render_transcript_entry)
            .collect::<Vec<_>>()
            .join("\n")
    };

    LogicalPrompt {
        sections: vec![
            PromptSection {
                number: 1,
                title: "HARNESS CONTROL".into(),
                content: control,
            },
            PromptSection {
                number: 2,
                title: "AUTHORED PHASE + BEHAVIOR".into(),
                content: authored,
            },
            PromptSection {
                number: 3,
                title: CONSUMER_RUN_CONTEXT_SECTION_TITLE.into(),
                content: context,
            },
            PromptSection {
                number: 4,
                title: "CROSS-PHASE STATE".into(),
                content: cross_phase,
            },
            PromptSection {
                number: 5,
                title: EFFECTIVE_CAPABILITY_CATALOG_SECTION_TITLE.into(),
                content: capability_catalog,
            },
            PromptSection {
                number: 6,
                title: CURRENT_PHASE_LOCAL_TRANSCRIPT_SECTION_TITLE.into(),
                content: transcript,
            },
        ],
        action_aliases,
        completion,
        diagnostics,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModelRequestTurn {
    UserInput {
        content: String,
    },
    AssistantContent {
        content: String,
    },
    SemanticActionCall {
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_call_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_alias: Option<String>,
        action_kind: String,
        identity: String,
        arguments: Value,
    },
    SemanticActionResult {
        #[serde(skip_serializing_if = "Option::is_none")]
        provider_call_id: Option<String>,
        action_kind: String,
        identity: String,
        result: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        action_succeeded: Option<bool>,
    },
    RepairFeedback {
        content: String,
    },
}

pub fn model_request_turns(transcript: &[TranscriptEntry]) -> Vec<ModelRequestTurn> {
    let mut turns = Vec::new();
    for entry in transcript {
        match entry.kind {
            TranscriptEntryKind::UserInput => {
                if let Some(content) = entry.content.as_str() {
                    turns.push(ModelRequestTurn::UserInput {
                        content: content.to_string(),
                    });
                } else {
                    turns.push(ModelRequestTurn::UserInput {
                        content: entry.content.to_string(),
                    });
                }
            }
            TranscriptEntryKind::Assistant => {
                if let Some(content) = entry.content.as_str() {
                    turns.push(ModelRequestTurn::AssistantContent {
                        content: content.to_string(),
                    });
                } else {
                    turns.push(ModelRequestTurn::AssistantContent {
                        content: entry.content.to_string(),
                    });
                }
            }
            TranscriptEntryKind::RepairFeedback => {
                if let Some(content) = entry.content.as_str() {
                    turns.push(ModelRequestTurn::RepairFeedback {
                        content: content.to_string(),
                    });
                } else {
                    turns.push(ModelRequestTurn::RepairFeedback {
                        content: entry.content.to_string(),
                    });
                }
            }
            TranscriptEntryKind::ActionResult => {
                let action_kind = entry
                    .content
                    .get("action_kind")
                    .and_then(Value::as_str)
                    .unwrap_or("semantic_action")
                    .to_string();
                let identity = entry
                    .content
                    .get("identity")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                let result = entry
                    .content
                    .get("result")
                    .cloned()
                    .unwrap_or_else(|| entry.content.clone());
                let provider_call_id = entry
                    .content
                    .get("provider_call_id")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let provider_alias = entry
                    .content
                    .get("provider_alias")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let native_call_id = match (provider_call_id, provider_alias) {
                    (Some(provider_call_id), Some(provider_alias)) => {
                        turns.push(ModelRequestTurn::SemanticActionCall {
                            provider_call_id: Some(provider_call_id.clone()),
                            provider_alias: Some(provider_alias),
                            action_kind: action_kind.clone(),
                            identity: identity.clone(),
                            arguments: entry
                                .content
                                .get("provider_arguments")
                                .cloned()
                                .unwrap_or_else(|| json!({})),
                        });
                        Some(provider_call_id)
                    }
                    _ => None,
                };
                turns.push(ModelRequestTurn::SemanticActionResult {
                    provider_call_id: native_call_id,
                    action_kind,
                    identity,
                    result,
                    action_succeeded: entry.action_succeeded,
                });
            }
        }
    }
    turns
}

pub(crate) fn provider_action_aliases(effective_phase: &EffectivePhase) -> Vec<ActionAlias> {
    effective_phase
        .capability_catalog
        .iter()
        .map(|descriptor| ActionAlias {
            alias: provider_action_alias(descriptor, effective_phase),
            action_kind: descriptor.action_kind.clone(),
            identity: descriptor.identity.clone(),
        })
        .collect()
}

fn provider_action_alias(
    descriptor: &CapabilityDescriptor,
    effective_phase: &EffectivePhase,
) -> String {
    match descriptor.action_kind.as_str() {
        "phase_completion" => "phase_complete".into(),
        "persistence_review_complete" => "persistence_review_complete".into(),
        "memory_read" | "memory_write" => memory_provider_action_alias(descriptor, effective_phase),
        "agentpm_tool" => identity_provider_action_alias("agentpm_tool", descriptor),
        "external_mcp_tool" => identity_provider_action_alias("mcp_tool", descriptor),
        "skill_resource_read" => identity_provider_action_alias("skill_resource", descriptor),
        "knowledge_request" => identity_provider_action_alias("knowledge_request", descriptor),
        other => identity_provider_action_alias(other, descriptor),
    }
}

fn memory_provider_action_alias(
    descriptor: &CapabilityDescriptor,
    effective_phase: &EffectivePhase,
) -> String {
    let memory = effective_phase
        .active_memory
        .iter()
        .find(|memory| memory_identity(&memory.package, &memory.space) == descriptor.identity);
    if let Some(memory) = memory {
        return provider_memory_alias_with_hash(
            &descriptor.action_kind,
            &memory.space,
            memory
                .record_types
                .as_slice()
                .first()
                .filter(|_| memory.record_types.len() == 1)
                .map(|record_type| record_type.name.as_str()),
            Some(&package_signal(&memory.package)),
            descriptor,
        );
    } else if let Some((package, space)) = split_memory_identity(&descriptor.identity) {
        return provider_memory_alias_with_hash(
            &descriptor.action_kind,
            space,
            None,
            Some(&package_signal(package)),
            descriptor,
        );
    } else {
        let parts = vec![
            provider_safe_component(&descriptor.action_kind),
            provider_safe_component(&descriptor.identity),
        ];
        provider_alias_with_hash(parts, descriptor)
    }
}

fn provider_memory_alias_with_hash(
    action_kind: &str,
    space: &str,
    fixed_record_type: Option<&str>,
    package_signal: Option<&str>,
    descriptor: &CapabilityDescriptor,
) -> String {
    let (suffix, base_budget) = provider_alias_suffix_and_budget(descriptor);

    let mut parts = vec![
        provider_safe_component(action_kind),
        provider_safe_component(space),
    ];
    if let Some(record_type) = fixed_record_type {
        parts.push(provider_safe_component(record_type));
    }
    if let Some(package_signal) = package_signal {
        parts.push(provider_safe_component(package_signal));
    }

    let mut cleaned = parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if cleaned.is_empty() {
        cleaned.push("memory".into());
    }

    if fixed_record_type.is_some() && cleaned.len() >= 3 {
        let required = cleaned[..3].to_vec();
        if required.join("_").len() > base_budget {
            let action = required[0].clone();
            let space = required[1].clone();
            let record_type = required[2].clone();
            if let Some(space_budget) = base_budget
                .checked_sub(action.len())
                .and_then(|remaining| remaining.checked_sub(record_type.len()))
                .and_then(|remaining| remaining.checked_sub(2))
            {
                let truncated_space = truncate_provider_alias_component(&space, space_budget);
                if !truncated_space.is_empty() {
                    let base = [action, truncated_space, record_type].join("_");
                    return format!("{base}_{suffix}");
                }
            }
        }
    }

    let mut truncated = truncate_provider_alias_parts(cleaned, base_budget);
    truncated = truncated.trim_matches('_').to_string();
    if truncated.is_empty() {
        truncated = "memory".into();
    }
    format!("{truncated}_{suffix}")
}

fn identity_provider_action_alias(prefix: &str, descriptor: &CapabilityDescriptor) -> String {
    provider_alias_with_hash(
        vec![
            provider_safe_component(prefix),
            provider_safe_component(&identity_signal(&descriptor.identity)),
        ],
        descriptor,
    )
}

fn provider_alias_with_hash(parts: Vec<String>, descriptor: &CapabilityDescriptor) -> String {
    let (suffix, base_budget) = provider_alias_suffix_and_budget(descriptor);
    let mut cleaned = parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if cleaned.is_empty() {
        cleaned.push("agentpm_action".into());
    }
    let mut truncated = truncate_provider_alias_parts(cleaned, base_budget);
    truncated = truncated.trim_matches('_').to_string();
    if truncated.is_empty() {
        truncated = "agentpm_action".into();
    }
    format!("{truncated}_{suffix}")
}

fn provider_alias_suffix_and_budget(descriptor: &CapabilityDescriptor) -> (String, usize) {
    let suffix_source = format!("{}:{}", descriptor.action_kind, descriptor.identity);
    let suffix = stable_hash_hex(&suffix_source, PROVIDER_ACTION_HASH_LEN);
    let suffix_len = suffix.len() + 1;
    let base_budget = PROVIDER_ACTION_ALIAS_MAX_LEN.saturating_sub(suffix_len);
    (suffix, base_budget)
}

fn provider_safe_component(raw: &str) -> String {
    let mut out = String::new();
    let mut previous_underscore = false;
    for byte in raw.bytes() {
        let ch = match byte {
            b'a'..=b'z' => byte as char,
            b'A'..=b'Z' => (byte as char).to_ascii_lowercase(),
            b'0'..=b'9' => byte as char,
            _ => '_',
        };
        if ch == '_' {
            if !previous_underscore && !out.is_empty() {
                out.push('_');
            }
            previous_underscore = true;
        } else {
            out.push(ch);
            previous_underscore = false;
        }
    }
    let trimmed = out.trim_matches('_').to_string();
    if trimmed
        .as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_digit())
    {
        format!("a_{trimmed}")
    } else {
        trimmed
    }
}

fn truncate_provider_alias_parts(mut parts: Vec<String>, max_len: usize) -> String {
    if parts.join("_").len() <= max_len {
        return parts.join("_");
    }
    while parts.len() > 1 {
        let fixed_len = parts[..parts.len() - 1].join("_").len() + 1;
        if fixed_len < max_len {
            let tail_budget = max_len - fixed_len;
            let tail = truncate_provider_alias_component(
                parts.last().expect("alias parts are not empty"),
                tail_budget,
            );
            if !tail.is_empty() {
                let mut candidate = parts[..parts.len() - 1].to_vec();
                candidate.push(tail);
                return candidate.join("_");
            }
        }
        parts.pop();
        if parts.join("_").len() <= max_len {
            return parts.join("_");
        }
    }
    truncate_provider_alias_component(
        parts
            .first()
            .map(String::as_str)
            .unwrap_or("agentpm_action"),
        max_len,
    )
}

fn truncate_provider_alias_component(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        return value.to_string();
    }
    if max_len == 0 {
        return String::new();
    }
    let prefix = value
        .bytes()
        .take(max_len)
        .map(char::from)
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    if prefix.is_empty() || value.as_bytes().get(max_len) == Some(&b'_') {
        return prefix;
    }
    prefix
        .rsplit_once('_')
        .map(|(head, _)| head.trim_matches('_').to_string())
        .filter(|head| !head.is_empty())
        .unwrap_or(prefix)
}

fn stable_hash_hex(value: &str, len: usize) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}").chars().take(len).collect()
}

fn identity_signal(identity: &str) -> String {
    identity
        .rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or(identity)
        .to_string()
}

fn package_signal(package: &str) -> String {
    package
        .rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or(package)
        .to_string()
}

fn split_memory_identity(identity: &str) -> Option<(&str, &str)> {
    identity.rsplit_once('/')
}

fn memory_identity(package: &str, space: &str) -> String {
    format!("{package}/{space}")
}

pub(crate) fn is_provider_safe_action_alias(alias: &str) -> bool {
    !alias.is_empty()
        && alias.len() <= PROVIDER_ACTION_ALIAS_MAX_LEN
        && alias
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && alias
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_lowercase())
}

fn transcript_has_successful_action_result(transcript: &[TranscriptEntry]) -> bool {
    transcript.iter().any(|entry| {
        entry.kind == TranscriptEntryKind::ActionResult && entry.action_succeeded == Some(true)
    })
}

fn render_transcript_entry(entry: &TranscriptEntry) -> String {
    if entry.kind == TranscriptEntryKind::ActionResult
        && let (Some(action_kind), Some(identity), Some(result)) = (
            entry.content.get("action_kind").and_then(Value::as_str),
            entry.content.get("identity").and_then(Value::as_str),
            entry.content.get("result"),
        )
    {
        return format!("- ActionResult [{action_kind} {identity}]: {result}");
    }
    format!("- {:?}: {}", entry.kind, entry.content)
}

fn render_loaded_skill_resources(transcript: &[TranscriptEntry]) -> String {
    let mut grouped: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for entry in transcript
        .iter()
        .filter(|entry| entry.kind == TranscriptEntryKind::ActionResult)
    {
        if entry.content.get("action_kind").and_then(Value::as_str) != Some("skill_resource_read") {
            continue;
        }
        let result = entry.content.get("result").unwrap_or(&entry.content);
        if result.get("ok").and_then(Value::as_bool) != Some(true) {
            continue;
        }
        let Some(skill) = result.get("skill").and_then(Value::as_str) else {
            continue;
        };
        let Some(resource) = result.get("resource").and_then(Value::as_str) else {
            continue;
        };
        let Some(content) = result.get("content").and_then(Value::as_str) else {
            continue;
        };
        if let Some((_, resources)) = grouped.iter_mut().find(|(candidate, _)| candidate == skill) {
            resources.push((resource.into(), content.into()));
        } else {
            grouped.push((skill.into(), vec![(resource.into(), content.into())]));
        }
    }

    grouped
        .into_iter()
        .map(|(skill, resources)| {
            let loaded_resources = resources
                .into_iter()
                .map(|(resource, content)| {
                    format!(
                        "  Loaded resource: {resource}\n\n{}",
                        indent_skill_content(&content)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            format!("Skill: {skill}\n\n{loaded_resources}")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn indent_skill_content(content: &str) -> String {
    content
        .lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_active_profiles(effective_phase: &EffectivePhase) -> String {
    effective_phase
        .active_profiles
        .iter()
        .map(|profile| {
            let mut block = format!("Profile: {}@{}", profile.name, profile.version);
            block.push_str(&render_profile_metadata(&profile.profile));
            block
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_profile_metadata(profile: &ProfileMetadata) -> String {
    let mut lines = Vec::new();
    lines.push(format!("\n  Identity role: {}", profile.identity.role));
    if let Some(description) = &profile.identity.description {
        lines.push(format!("  Identity description: {description}"));
    }
    push_list(&mut lines, "  Expertise", &profile.identity.expertise);
    push_list(&mut lines, "  Objectives", &profile.objectives);
    push_list(&mut lines, "  Principles", &profile.principles);
    if let Some(audience) = &profile.audience {
        if let Some(description) = &audience.description {
            lines.push(format!("  Audience: {description}"));
        }
        if let Some(assumed_knowledge) = &audience.assumed_knowledge {
            lines.push(format!("  Audience assumed knowledge: {assumed_knowledge}"));
        }
        push_list(&mut lines, "  Audience adaptation", &audience.adaptation);
    }
    push_list(&mut lines, "  Tone", &profile.communication.tone);
    lines.push(format!(
        "  Verbosity: {:?}",
        profile.communication.verbosity
    ));
    push_list(
        &mut lines,
        "  Communication guidelines",
        &profile.communication.guidelines,
    );
    push_list(
        &mut lines,
        "  Formatting",
        &profile.communication.formatting,
    );
    if let Some(vocabulary) = &profile.communication.vocabulary {
        push_list(&mut lines, "  Preferred vocabulary", &vocabulary.prefer);
        push_list(&mut lines, "  Avoided vocabulary", &vocabulary.avoid);
    }
    push_list(&mut lines, "  Boundaries", &profile.boundaries);
    if !profile.constraints.is_empty() {
        lines.push("  Constraints:".into());
        for constraint in &profile.constraints {
            let strength = match constraint.strength {
                ProfileConstraintStrength::Required => "required",
                ProfileConstraintStrength::Preferred => "preferred",
            };
            lines.push(format!(
                "  - [{strength}] {}: {}",
                constraint.id, constraint.instruction
            ));
        }
    }
    lines.join("\n")
}

fn push_list(lines: &mut Vec<String>, label: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    lines.push(format!("{label}:"));
    for value in values {
        lines.push(format!("  - {value}"));
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelRequest {
    pub runtime: RuntimeSnapshot,
    pub model: Option<ModelProviderSelection>,
    pub prompt: LogicalPrompt,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ordered_turns: Vec<ModelRequestTurn>,
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
pub struct ModelRuntimeRequestSnapshot {
    pub runtime_kind: String,
    pub request_kind: String,
    pub provider: String,
    pub model: String,
    pub action_descriptors: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_actions: Option<usize>,
    pub capability_catalog_in_prompt: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_aliases: Vec<ActionAlias>,
    pub turn_strategy: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ordered_turns: Vec<ModelRequestTurn>,
    pub prompt: String,
}

impl ModelRuntimeRequestSnapshot {
    pub fn into_trace_fields(self) -> Result<BTreeMap<String, Value>> {
        let value =
            serde_json::to_value(self).context("serializing model runtime request snapshot")?;
        let Value::Object(map) = value else {
            return Ok(BTreeMap::new());
        };
        Ok(map.into_iter().collect())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelTurn {
    pub assistant_content: Option<String>,
    pub actions: Vec<SemanticActionProposal>,
    #[serde(default)]
    pub usage: RunUsage,
    #[serde(default)]
    pub finish_reason: Option<String>,
    #[serde(default)]
    pub provider_metadata: BTreeMap<String, Value>,
}

pub trait ModelRuntime {
    fn capabilities(&self) -> ModelCapabilityAdvertisement {
        ModelCapabilityAdvertisement::default()
    }

    fn inspect_request(&self, _request: &ModelRequest) -> Option<ModelRuntimeRequestSnapshot> {
        None
    }

    fn generate(&mut self, request: ModelRequest) -> Result<ModelTurn, ModelRuntimeFailure>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCapabilityAdvertisement {
    pub semantic_actions: bool,
    pub structured_output: bool,
    pub multimodal_input: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window_tokens: Option<u64>,
    pub usage_reporting: bool,
}

impl Default for ModelCapabilityAdvertisement {
    fn default() -> Self {
        Self {
            semantic_actions: true,
            structured_output: true,
            multimodal_input: false,
            context_window_tokens: None,
            usage_reporting: true,
        }
    }
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
    fn inspect_request(&self, request: &ModelRequest) -> Option<ModelRuntimeRequestSnapshot> {
        let selection = request
            .model
            .clone()
            .unwrap_or_else(|| ModelProviderSelection {
                provider: "scripted".into(),
                model: "scripted".into(),
                options: Value::Object(Default::default()),
            });
        let prompt = request.prompt.render_text();
        Some(ModelRuntimeRequestSnapshot {
            runtime_kind: "scripted".into(),
            request_kind: "canonical_model_request".into(),
            provider: selection.provider,
            model: selection.model,
            action_descriptors: request.prompt.action_aliases.len(),
            structured_actions: None,
            capability_catalog_in_prompt: request.prompt.has_capability_catalog_section(),
            action_aliases: request.prompt.action_aliases.clone(),
            turn_strategy: "canonical_request".into(),
            ordered_turns: request.ordered_turns.clone(),
            prompt,
        })
    }

    fn generate(&mut self, request: ModelRequest) -> Result<ModelTurn, ModelRuntimeFailure> {
        self.requests.push(request);
        self.turns
            .pop_front()
            .unwrap_or_else(|| Err(ModelRuntimeFailure::new("scripted model exhausted")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(action_kind: &str, identity: &str) -> CapabilityDescriptor {
        CapabilityDescriptor {
            action_kind: action_kind.into(),
            identity: identity.into(),
            description: format!("{action_kind} descriptor"),
            source: "test".into(),
        }
    }

    fn memory_space(
        package: &str,
        space: &str,
        record_types: impl IntoIterator<Item = &'static str>,
    ) -> MemorySpaceRuntimeSnapshot {
        MemorySpaceRuntimeSnapshot {
            package: package.into(),
            package_version: "0.1.0".into(),
            space: space.into(),
            model: MemorySpaceModel::Collection,
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
            record_types: record_types
                .into_iter()
                .map(|name| MemoryRecordTypeRuntimeSnapshot {
                    name: name.into(),
                    schema_version: "1.0.0".into(),
                    content_schema: Value::Object(Default::default()),
                })
                .collect(),
        }
    }

    fn phase_with(
        capability_catalog: Vec<CapabilityDescriptor>,
        active_memory: Vec<MemorySpaceRuntimeSnapshot>,
    ) -> EffectivePhase {
        EffectivePhase {
            phase_id: "remember".into(),
            tools_allowed: None,
            knowledge_allowed: None,
            memory_read_allowed: None,
            memory_write_allowed: None,
            authored_profile_candidates: Vec::new(),
            active_profiles: Vec::new(),
            active_tools: Vec::new(),
            active_skills: Vec::new(),
            active_knowledge: Vec::new(),
            active_memory,
            active_memory_operations: Vec::new(),
            capability_catalog,
            suppressed_capabilities: Vec::new(),
        }
    }

    #[test]
    fn provider_action_aliases_are_semantic_provider_safe_and_stable() {
        let single_memory = memory_space("@zack/m16-reference-memory", "notes", ["note"]);
        let multi_memory = memory_space(
            "@zack/m16-reference-memory",
            "mixed_notes",
            ["summary", "preference"],
        );
        let capability_catalog = vec![
            descriptor("phase_completion", "remember/completion"),
            descriptor("agentpm_tool", "@zack/search"),
            descriptor("agentpm_tool", "@other/search"),
            descriptor("external_mcp_tool", "incident-data/search"),
            descriptor("skill_resource_read", "@zack/handoff-skill"),
            descriptor("knowledge_request", "@zack/guide"),
            descriptor("memory_read", "@zack/m16-reference-memory/notes"),
            descriptor("memory_write", "@zack/m16-reference-memory/notes"),
            descriptor("memory_write", "@zack/m16-reference-memory/mixed_notes"),
            descriptor("persistence_review_complete", "harness/persistence_review"),
        ];
        let phase = phase_with(
            capability_catalog,
            vec![single_memory.clone(), multi_memory.clone()],
        );

        let aliases = provider_action_aliases(&phase);

        assert_eq!(aliases.len(), phase.capability_catalog.len());
        for alias in &aliases {
            assert!(
                is_provider_safe_action_alias(&alias.alias),
                "alias `{}` should be provider safe",
                alias.alias
            );
            assert!(
                !alias.alias.starts_with("action_"),
                "alias `{}` should not be positional",
                alias.alias
            );
        }
        assert!(aliases.iter().any(|alias| {
            alias.action_kind == "phase_completion" && alias.alias == "phase_complete"
        }));
        assert!(aliases.iter().any(|alias| {
            alias.action_kind == "persistence_review_complete"
                && alias.alias == "persistence_review_complete"
        }));
        let search_aliases = aliases
            .iter()
            .filter(|alias| alias.action_kind == "agentpm_tool")
            .map(|alias| alias.alias.as_str())
            .collect::<Vec<_>>();
        assert_eq!(search_aliases.len(), 2);
        assert_ne!(search_aliases[0], search_aliases[1]);
        assert!(
            search_aliases
                .iter()
                .all(|alias| alias.starts_with("agentpm_tool_search_"))
        );
        let mcp_tool = aliases
            .iter()
            .find(|alias| alias.action_kind == "external_mcp_tool")
            .expect("mcp tool alias");
        assert_eq!(mcp_tool.identity, "incident-data/search");
        assert!(mcp_tool.alias.starts_with("mcp_tool_search_"));
        assert_eq!(
            mcp_tool
                .alias
                .rsplit_once('_')
                .map(|(_, suffix)| suffix.len()),
            Some(PROVIDER_ACTION_HASH_LEN)
        );
        let skill_resource = aliases
            .iter()
            .find(|alias| alias.action_kind == "skill_resource_read")
            .expect("skill resource alias");
        assert_eq!(skill_resource.identity, "@zack/handoff-skill");
        assert!(
            skill_resource
                .alias
                .starts_with("skill_resource_handoff_skill_")
        );
        assert_eq!(
            skill_resource
                .alias
                .rsplit_once('_')
                .map(|(_, suffix)| suffix.len()),
            Some(PROVIDER_ACTION_HASH_LEN)
        );
        let knowledge = aliases
            .iter()
            .find(|alias| alias.action_kind == "knowledge_request")
            .expect("knowledge alias");
        assert_eq!(knowledge.identity, "@zack/guide");
        assert!(knowledge.alias.starts_with("knowledge_request_guide_"));
        assert_eq!(
            knowledge
                .alias
                .rsplit_once('_')
                .map(|(_, suffix)| suffix.len()),
            Some(PROVIDER_ACTION_HASH_LEN)
        );
        let notes_write = aliases
            .iter()
            .find(|alias| {
                alias.action_kind == "memory_write"
                    && alias.identity == "@zack/m16-reference-memory/notes"
            })
            .expect("notes write alias");
        assert!(notes_write.alias.starts_with("memory_write_notes_note_"));
        let mixed_write = aliases
            .iter()
            .find(|alias| {
                alias.action_kind == "memory_write"
                    && alias.identity == "@zack/m16-reference-memory/mixed_notes"
            })
            .expect("mixed write alias");
        assert!(mixed_write.alias.starts_with("memory_write_mixed_notes_"));
        assert!(!mixed_write.alias.contains("summary"));
        assert!(!mixed_write.alias.contains("preference"));

        let mut expanded_phase = phase_with(
            vec![
                descriptor("agentpm_tool", "@zack/unrelated"),
                descriptor("memory_write", "@zack/m16-reference-memory/notes"),
                descriptor("phase_completion", "remember/completion"),
            ],
            vec![single_memory],
        );
        let stable_alias = provider_action_aliases(&expanded_phase)
            .into_iter()
            .find(|alias| {
                alias.action_kind == "memory_write"
                    && alias.identity == "@zack/m16-reference-memory/notes"
            })
            .expect("expanded notes write alias");
        assert_eq!(stable_alias.alias, notes_write.alias);
        expanded_phase.capability_catalog.reverse();
        let reordered_alias = provider_action_aliases(&expanded_phase)
            .into_iter()
            .find(|alias| {
                alias.action_kind == "memory_write"
                    && alias.identity == "@zack/m16-reference-memory/notes"
            })
            .expect("reordered notes write alias");
        assert_eq!(reordered_alias.alias, notes_write.alias);
    }

    #[test]
    fn memory_alias_truncation_preserves_space_and_fixed_record_type() {
        let memory = memory_space(
            "@very-long-scope/very-long-package-name-that-should-not-dominate",
            "launch_readiness_notes",
            ["note"],
        );
        let phase = phase_with(
            vec![descriptor(
                "memory_write",
                "@very-long-scope/very-long-package-name-that-should-not-dominate/launch_readiness_notes",
            )],
            vec![memory],
        );

        let alias = &provider_action_aliases(&phase)[0].alias;

        assert!(is_provider_safe_action_alias(alias));
        assert!(alias.len() <= 64);
        assert!(alias.contains("memory_write"));
        assert!(alias.contains("launch_readiness_notes"));
        assert!(alias.contains("note"));
    }

    #[test]
    fn memory_alias_truncation_keeps_fixed_record_type_when_space_is_long() {
        let memory = memory_space(
            "@zack/m16a-memory-package-with-long-alias-truncation-name",
            "conversation_state_notes_with_intentionally_long_alias_tail",
            ["note"],
        );
        let phase = phase_with(
            vec![descriptor(
                "memory_write",
                "@zack/m16a-memory-package-with-long-alias-truncation-name/conversation_state_notes_with_intentionally_long_alias_tail",
            )],
            vec![memory],
        );

        let alias = &provider_action_aliases(&phase)[0].alias;

        assert!(is_provider_safe_action_alias(alias));
        assert!(alias.len() <= 64);
        assert!(alias.starts_with("memory_write_conversation_state_notes_with_note_"));
    }

    #[test]
    fn alias_truncation_prefers_component_boundaries_for_all_action_kinds() {
        let phase = phase_with(
            vec![descriptor(
                "external_mcp_tool",
                "incident-data/very-long-server-name-with-very-long-tool-name-that-should-drop-tail",
            )],
            vec![],
        );

        let alias = &provider_action_aliases(&phase)[0].alias;

        assert!(is_provider_safe_action_alias(alias));
        assert_eq!(alias.len(), 64);
        assert!(alias.starts_with("mcp_tool_very_long_server_name_with_very_long_tool_name_"));
        assert!(!alias.contains("nam_"));
    }

    #[test]
    fn alias_truncation_shortens_low_priority_parts_before_required_parts() {
        let memory = memory_space(
            "@very-long-scope/package-signal-that-should-shorten-first",
            "conversation_state",
            ["note"],
        );
        let phase = phase_with(
            vec![descriptor(
                "memory_write",
                "@very-long-scope/package-signal-that-should-shorten-first/conversation_state",
            )],
            vec![memory],
        );

        let alias = &provider_action_aliases(&phase)[0].alias;

        assert!(is_provider_safe_action_alias(alias));
        assert!(alias.len() <= 64);
        assert!(alias.starts_with("memory_write_conversation_state_note_package_signal_"));
        assert!(!alias.contains("package_signal_th"));
    }

    #[test]
    fn ordered_turns_preserve_every_transcript_entry_kind_and_call_correlation() {
        let transcript = vec![
            TranscriptEntry {
                kind: TranscriptEntryKind::UserInput,
                content: json!("write then read"),
                action_succeeded: None,
            },
            TranscriptEntry {
                kind: TranscriptEntryKind::Assistant,
                content: json!("I will write a note."),
                action_succeeded: None,
            },
            TranscriptEntry {
                kind: TranscriptEntryKind::ActionResult,
                content: json!({
                    "action_kind": "memory_write",
                    "identity": "@zack/memory/notes",
                    "provider_call_id": "call_1",
                    "provider_alias": "memory_write_notes_note_abcd1234",
                    "provider_arguments": {
                        "operation": "create",
                        "record_type": "note",
                        "content": { "body": "launch" }
                    },
                    "result": { "ok": true, "record_id": "mem-1" }
                }),
                action_succeeded: Some(true),
            },
            TranscriptEntry {
                kind: TranscriptEntryKind::RepairFeedback,
                content: json!("retry with query"),
                action_succeeded: None,
            },
        ];

        let turns = model_request_turns(&transcript);

        assert!(matches!(
            &turns[0],
            ModelRequestTurn::UserInput { content } if content == "write then read"
        ));
        assert!(matches!(
            &turns[1],
            ModelRequestTurn::AssistantContent { content } if content == "I will write a note."
        ));
        assert!(matches!(
            &turns[2],
            ModelRequestTurn::SemanticActionCall {
                provider_call_id: Some(provider_call_id),
                provider_alias: Some(provider_alias),
                action_kind,
                identity,
                ..
            } if provider_call_id == "call_1"
                && provider_alias == "memory_write_notes_note_abcd1234"
                && action_kind == "memory_write"
                && identity == "@zack/memory/notes"
        ));
        assert!(matches!(
            &turns[3],
            ModelRequestTurn::SemanticActionResult {
                provider_call_id: Some(provider_call_id),
                action_kind,
                identity,
                action_succeeded: Some(true),
                ..
            } if provider_call_id == "call_1"
                && action_kind == "memory_write"
                && identity == "@zack/memory/notes"
        ));
        assert!(matches!(
            &turns[4],
            ModelRequestTurn::RepairFeedback { content } if content == "retry with query"
        ));
    }

    #[test]
    fn ordered_turns_do_not_emit_orphaned_native_result_without_provider_alias() {
        let transcript = vec![TranscriptEntry {
            kind: TranscriptEntryKind::ActionResult,
            content: json!({
                "action_kind": "agentpm_tool",
                "identity": "@zack/search",
                "provider_call_id": "call_orphan",
                "result": { "ok": true }
            }),
            action_succeeded: Some(true),
        }];

        let turns = model_request_turns(&transcript);

        assert_eq!(turns.len(), 1);
        assert!(matches!(
            &turns[0],
            ModelRequestTurn::SemanticActionResult {
                provider_call_id: None,
                action_kind,
                identity,
                action_succeeded: Some(true),
                ..
            } if action_kind == "agentpm_tool" && identity == "@zack/search"
        ));
    }

    #[test]
    fn provider_native_prompt_omits_turn_backed_history_but_diagnostic_render_keeps_it() {
        let phase = phase_with(
            vec![descriptor("phase_completion", "remember/completion")],
            vec![],
        );
        let transcript = vec![
            TranscriptEntry {
                kind: TranscriptEntryKind::UserInput,
                content: json!("multi-line\nrun input"),
                action_succeeded: None,
            },
            TranscriptEntry {
                kind: TranscriptEntryKind::RepairFeedback,
                content: json!("repair once"),
                action_succeeded: None,
            },
        ];
        let prompt = assemble_logical_prompt(PromptAssemblyInput {
            purpose: PromptAssemblyPurpose::Phase,
            phase_id: "remember",
            phase_objective: "Remember useful details.",
            explicit_outcomes: &["done".into()],
            run_input: "multi-line\nrun input",
            consumer_context: Some(&ConsumerContextSnapshot {
                state: "NotConfigured".into(),
                file: None,
                path: None,
                content: None,
                byte_size: None,
                approximate_tokens: None,
                sha256: None,
            }),
            prior_phase_results: &[],
            effective_phase: &phase,
            transcript: &transcript,
            repair_feedback: Some("repair once"),
        });

        let diagnostic = prompt.render_text();
        assert!(diagnostic.contains(CURRENT_PHASE_LOCAL_TRANSCRIPT_SECTION_TITLE));
        assert!(diagnostic.contains("Run input:\nmulti-line\nrun input"));
        assert!(diagnostic.contains("Repair feedback from previous turn: repair once"));

        let provider = prompt.render_provider_text_with_native_turns(false);
        assert!(!provider.contains(CURRENT_PHASE_LOCAL_TRANSCRIPT_SECTION_TITLE));
        assert!(!provider.contains("Run input:\nmulti-line\nrun input"));
        assert!(!provider.contains("Repair feedback from previous turn: repair once"));
        assert!(provider.contains("Consumer Context snapshot:"));
        assert!(!provider.contains(EFFECTIVE_CAPABILITY_CATALOG_SECTION_TITLE));
    }

    #[test]
    fn persistence_review_control_guidance_names_exact_memory_target_selection() {
        let phase = phase_with(
            vec![
                descriptor("memory_write", "@zack/memory/notes"),
                descriptor("memory_write", "@zack/memory/current_note"),
                descriptor("persistence_review_complete", "harness/persistence_review"),
            ],
            vec![
                memory_space("@zack/memory", "notes", ["note"]),
                memory_space("@zack/memory", "current_note", ["note"]),
            ],
        );
        let aliases = provider_action_aliases(&phase);
        assert!(aliases.iter().any(|alias| {
            alias.action_kind == "memory_write"
                && alias.identity == "@zack/memory/notes"
                && alias.alias.starts_with("memory_write_notes_note_")
        }));
        assert!(aliases.iter().any(|alias| {
            alias.action_kind == "memory_write"
                && alias.identity == "@zack/memory/current_note"
                && alias.alias.starts_with("memory_write_current_note_note_")
        }));
        assert!(
            aliases
                .iter()
                .any(|alias| alias.alias == "persistence_review_complete")
        );
        let prompt = assemble_logical_prompt(PromptAssemblyInput {
            purpose: PromptAssemblyPurpose::MemoryWriteReview {
                point: "phase_end",
                pending_outcome: "done",
            },
            phase_id: "remember",
            phase_objective: "Remember useful details.",
            explicit_outcomes: &["done".into()],
            run_input: "input",
            consumer_context: None,
            prior_phase_results: &[],
            effective_phase: &phase,
            transcript: &[],
            repair_feedback: None,
        });

        let text = prompt.render_text();
        assert!(text.contains(PERSISTENCE_REVIEW_TARGET_SELECTION_CONTROL));
        assert!(text.contains("persistence_review_complete"));
    }
}
