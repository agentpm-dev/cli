#![allow(dead_code)]

use super::action::SemanticActionProposal;
use crate::harness_engine::{EffectivePhase, PhaseResult};
use crate::harness_observability::RunUsage;
use crate::manifest::{
    MemoryRetrievalMode, MemorySpaceModel, ProfileConstraintStrength, ProfileMetadata,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;

pub const CONSUMER_RUN_CONTEXT_SECTION_TITLE: &str = "CONSUMER / RUN CONTEXT";
pub const EFFECTIVE_CAPABILITY_CATALOG_SECTION_TITLE: &str = "EFFECTIVE CAPABILITY CATALOG";
pub(crate) const SUCCESSFUL_ACTION_RESULT_CONTROL: &str = "If the phase-local transcript already contains successful ActionResults for all requested executable actions, do not propose any of those actions again; propose phase_completion next. For repeated actions, compare action kind, identity, and arguments.";

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
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub append_only: bool,
    pub record_types: Vec<MemoryRecordTypeRuntimeSnapshot>,
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
            if !rendered.is_empty() {
                rendered.push_str("\n\n");
            }
            rendered.push_str(&format!("{}. {}\n", section.number, section.title));
            rendered.push_str(&section.content);
        }
        rendered
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LogicalPromptRenderOptions {
    include_capability_catalog: bool,
}

impl Default for LogicalPromptRenderOptions {
    fn default() -> Self {
        Self {
            include_capability_catalog: true,
        }
    }
}

pub struct PromptAssemblyInput<'a> {
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

pub fn assemble_logical_prompt(input: PromptAssemblyInput<'_>) -> LogicalPrompt {
    let implicit_complete = input.explicit_outcomes.is_empty();
    let completion = CompletionContract {
        phase_id: input.phase_id.to_string(),
        explicit_outcomes: input.explicit_outcomes.to_vec(),
        implicit_complete,
    };
    let mut diagnostics = Vec::new();
    let action_aliases = input
        .effective_phase
        .capability_catalog
        .iter()
        .enumerate()
        .map(|(index, descriptor)| ActionAlias {
            alias: format!("action_{}", index + 1),
            action_kind: descriptor.action_kind.clone(),
            identity: descriptor.identity.clone(),
        })
        .collect::<Vec<_>>();

    let outcome_contract = if implicit_complete {
        "This phase has implicit outcome `complete`; final assistant text with no action may complete the phase.".to_string()
    } else {
        format!(
            "This phase must complete with exactly one authored outcome: {}.",
            input.explicit_outcomes.join(", ")
        )
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
        control.push_str(SUCCESSFUL_ACTION_RESULT_CONTROL);
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
                title: "CURRENT PHASE-LOCAL TRANSCRIPT".into(),
                content: transcript,
            },
        ],
        action_aliases,
        completion,
        diagnostics,
    }
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
