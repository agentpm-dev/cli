use crate::semver::types::Lock;
use anyhow::{Context, Result, anyhow, bail};
use jsonschema::{Draft, JSONSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::{
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};

const DEFAULT_MANIFEST_SCHEMA_URL: &str = "https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json";
const EMBEDDED_MANIFEST_SCHEMA_JSON: &str =
    include_str!("../../../schemas/agentpm.manifest.schema.json");

#[derive(Serialize, Debug, Clone)]
pub struct LintIssue {
    pub file: String,
    pub level: &'static str, // "error" | "warning"
    pub message: String,
    pub instance_path: String,
    pub schema_path: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct LintFileReport {
    pub file: String,
    pub ok: bool,
    pub issues: Vec<LintIssue>,
}

fn default_cwd() -> String {
    ".".into()
}
fn default_timeout_ms() -> u64 {
    60_000
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Entrypoint {
    pub command: String,   // required
    pub args: Vec<String>, // default: []
    #[serde(default = "default_cwd")]
    pub cwd: String, // default: "."
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64, // default: 60000
    pub env: HashMap<String, String>, // default: {}
}

/// Minimal shape we need from agent.json for publish.
/// Keep it liberal (Value) for forward-compat fields.
#[derive(Debug, Deserialize)]
pub struct ToolManifest {
    pub kind: String,
    pub name: String,
    pub version: String,
    #[allow(dead_code)]
    pub description: Option<String>,
    pub entrypoint: Entrypoint,
    #[serde(default)]
    pub files: Vec<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub runtime: Value,
    #[allow(dead_code)]
    #[serde(default)]
    pub inputs: Value,
    #[allow(dead_code)]
    #[serde(default)]
    pub outputs: Value,
    // allow unknowns to pass through
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct AgentManifest {
    pub kind: String,
    pub name: String,
    pub version: String,
    #[allow(dead_code)]
    pub description: Option<String>,
    #[serde(default)]
    pub tools: Vec<PackageReference>,
    #[serde(default)]
    pub skills: Vec<PackageReference>,
    #[serde(default)]
    pub knowledge: Vec<PackageReference>,
    #[serde(default)]
    pub memory: Vec<PackageReference>,
    #[serde(default)]
    pub profiles: Vec<PackageReference>,
    #[serde(rename = "loop")]
    pub loop_ref: Option<PackageReference>,
    pub bindings: Option<AgentBindings>,
}

#[derive(Debug, Deserialize)]
pub struct TemplateMetadata {
    #[allow(dead_code)]
    pub display_name: Option<String>,
    #[allow(dead_code)]
    pub use_case: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub execution_surfaces: Vec<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub stack: Vec<String>,
    pub files_root: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub variables: Vec<TemplateVariable>,
    #[allow(dead_code)]
    #[serde(default)]
    pub dependencies: TemplateDependencies,
    #[allow(dead_code)]
    #[serde(default)]
    pub entrypoints: Vec<TemplateEntrypoint>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TemplateVariable {
    pub name: String,
    #[allow(dead_code)]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum PackageReference {
    String(String),
    Object {
        name: String,
        #[serde(default)]
        version: Option<String>,
    },
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct TemplateDependencies {
    pub tools: Vec<PackageReference>,
    pub agents: Vec<PackageReference>,
    pub skills: Vec<PackageReference>,
    pub knowledge: Vec<PackageReference>,
    pub memory: Vec<PackageReference>,
    pub profiles: Vec<PackageReference>,
    pub r#loop: Option<PackageReference>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TemplateEntrypoint {
    pub label: String,
    pub command: String,
}

#[derive(Debug, Deserialize)]
pub struct TemplateManifest {
    pub kind: String,
    pub name: String,
    pub version: String,
    #[allow(dead_code)]
    pub description: Option<String>,
    pub template: TemplateMetadata,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
#[allow(dead_code)]
pub struct SkillCompatibility {
    pub model_families: Vec<String>,
    pub runtimes: Vec<String>,
    pub environments: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct SkillMetadata {
    pub entrypoint: String,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub scripts: Vec<String>,
    #[serde(default)]
    pub compatibility: SkillCompatibility,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SkillManifest {
    pub kind: String,
    pub name: String,
    pub version: String,
    #[allow(dead_code)]
    pub description: Option<String>,
    #[serde(default)]
    pub tools: Vec<PackageReference>,
    pub skill: SkillMetadata,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
#[allow(dead_code)]
pub struct KnowledgeDocument {
    pub path: String,
    pub content_type: Option<String>,
    pub role: Option<String>,
    pub description: Option<String>,
    pub bytes: Option<u64>,
    pub sha256: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
#[allow(dead_code)]
pub struct KnowledgeContext {
    pub document_count: Option<u64>,
    pub total_bytes: Option<u64>,
    pub content_hash: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
#[allow(dead_code)]
pub struct KnowledgeCorpus {
    pub chunks_path: Option<String>,
    pub sources_path: Option<String>,
    pub chunk_count: Option<u64>,
    pub source_count: Option<u64>,
    pub content_hash: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
#[allow(dead_code)]
pub struct KnowledgeChunking {
    pub strategy: Option<String>,
    pub chunk_size: Option<u64>,
    pub overlap: Option<u64>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
#[allow(dead_code)]
pub struct KnowledgeEmbedding {
    pub id: String,
    pub provider: String,
    pub model: String,
    pub dimensions: u64,
    pub metric: String,
    pub normalized: bool,
    pub vectors_path: String,
    pub vector_count: Option<u64>,
    pub vectors_hash: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
#[allow(dead_code)]
pub struct KnowledgeIndex {
    pub id: String,
    pub r#type: String,
    pub path: String,
    pub embedding_id: String,
    pub generated_by: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
#[allow(dead_code)]
pub struct KnowledgeRetrieval {
    pub strategy: Option<String>,
    pub default_top_k: Option<u64>,
    pub default_score_threshold: Option<f64>,
    pub return_citations: Option<bool>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
#[allow(dead_code)]
pub struct KnowledgeBuilder {
    pub name: String,
    pub version: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
#[allow(dead_code)]
pub struct KnowledgeProvenance {
    pub sources_manifest_path: Option<String>,
    pub generated_at: Option<String>,
    pub builder: Option<KnowledgeBuilder>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
#[allow(dead_code)]
pub struct KnowledgeMetadata {
    pub mode: String,
    pub content_type: Option<String>,
    pub language: Option<String>,
    pub documents: Vec<KnowledgeDocument>,
    pub context: Option<KnowledgeContext>,
    pub corpus: Option<KnowledgeCorpus>,
    pub chunking: Option<KnowledgeChunking>,
    pub embedding: Option<KnowledgeEmbedding>,
    pub indexes: Vec<KnowledgeIndex>,
    pub retrieval: Option<KnowledgeRetrieval>,
    pub provenance: Option<KnowledgeProvenance>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct KnowledgeManifest {
    pub kind: String,
    pub name: String,
    pub version: String,
    #[allow(dead_code)]
    pub description: Option<String>,
    pub knowledge: KnowledgeMetadata,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct MemoryScope {
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct MemoryRecordType {
    pub version: String,
    pub description: String,
    pub schema: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemorySpaceModel {
    Document,
    Collection,
    Sequence,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum MemoryRetrievalMode {
    Key,
    Filter,
    Chronological,
    FullText,
    Semantic,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct MemoryRetrieval {
    pub modes: Vec<MemoryRetrievalMode>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct MemoryCapacity {
    pub max_records: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryRetentionAction {
    Delete,
    Archive,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct MemoryRetention {
    pub ttl: String,
    pub on_expire: MemoryRetentionAction,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
#[allow(dead_code)]
pub struct MemoryConstraints {
    pub append_only: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct MemorySpace {
    pub description: String,
    pub model: MemorySpaceModel,
    pub record_types: Vec<String>,
    pub scope: Vec<String>,
    pub retrieval: MemoryRetrieval,
    pub capacity: Option<MemoryCapacity>,
    pub retention: Option<MemoryRetention>,
    pub constraints: Option<MemoryConstraints>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct MemoryOperationRef {
    pub space: String,
    pub record_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct MemoryOperationTarget {
    pub space: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
pub enum MemoryTrigger {
    External,
    RecordCount { space: String, threshold: u64 },
    Capacity { space: String },
    Interval { every: String },
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemorySourceHandling {
    Retain,
    RetainUntilExpiration,
    DeleteAfterSuccess,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
pub enum MemoryOperation {
    Consolidate {
        description: String,
        trigger: MemoryTrigger,
        inputs: Vec<MemoryOperationRef>,
        output: MemoryOperationRef,
        source_handling: MemorySourceHandling,
        preserve_provenance: bool,
    },
    Transform {
        description: String,
        trigger: MemoryTrigger,
        // Schema validation enforces exactly one input for transform operations.
        // Keep semantic validation explicit in later milestones rather than
        // assuming deserialization alone preserves that invariant.
        inputs: Vec<MemoryOperationRef>,
        output: MemoryOperationRef,
        source_handling: MemorySourceHandling,
        preserve_provenance: bool,
    },
    Delete {
        description: String,
        trigger: MemoryTrigger,
        targets: Vec<MemoryOperationTarget>,
        cascade_derived_records: bool,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct MemoryMetadata {
    pub scopes: HashMap<String, MemoryScope>,
    pub record_types: HashMap<String, MemoryRecordType>,
    pub spaces: HashMap<String, MemorySpace>,
    #[serde(default)]
    pub operations: HashMap<String, MemoryOperation>,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct MemoryManifest {
    pub kind: String,
    pub name: String,
    pub version: String,
    #[allow(dead_code)]
    pub description: Option<String>,
    pub memory: MemoryMetadata,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct ProfileIdentity {
    pub role: String,
    pub description: Option<String>,
    #[serde(default)]
    pub expertise: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct ProfileAudience {
    pub description: Option<String>,
    pub assumed_knowledge: Option<String>,
    #[serde(default)]
    pub adaptation: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct ProfileVocabulary {
    #[serde(default)]
    pub prefer: Vec<String>,
    #[serde(default)]
    pub avoid: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileVerbosity {
    Concise,
    Balanced,
    Detailed,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct ProfileCommunication {
    pub tone: Vec<String>,
    pub verbosity: ProfileVerbosity,
    #[serde(default)]
    pub guidelines: Vec<String>,
    #[serde(default)]
    pub formatting: Vec<String>,
    pub vocabulary: Option<ProfileVocabulary>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileConstraintStrength {
    Required,
    Preferred,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct ProfileConstraint {
    pub id: String,
    pub strength: ProfileConstraintStrength,
    pub instruction: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
#[allow(dead_code)]
pub struct ProfileCapabilityHints {
    pub tool_use: Option<bool>,
    pub structured_output: Option<bool>,
    pub multimodal_input: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
#[allow(dead_code)]
pub struct ProfileCompatibility {
    pub minimum_context_tokens: Option<u64>,
    pub requires: Option<ProfileCapabilityHints>,
    pub recommends: Option<ProfileCapabilityHints>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct ProfileMetadata {
    pub identity: ProfileIdentity,
    pub objectives: Vec<String>,
    #[serde(default)]
    pub principles: Vec<String>,
    pub audience: Option<ProfileAudience>,
    pub communication: ProfileCommunication,
    #[serde(default)]
    pub boundaries: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<ProfileConstraint>,
    pub compatibility: Option<ProfileCompatibility>,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ProfileManifest {
    pub kind: String,
    pub name: String,
    pub version: String,
    #[allow(dead_code)]
    pub description: Option<String>,
    pub readme: Option<String>,
    #[serde(default)]
    pub license: Option<Value>,
    pub profile: ProfileMetadata,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct LoopAccessMemory {
    pub read: Option<bool>,
    pub write: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct LoopPhaseAccess {
    pub tools: Option<bool>,
    pub knowledge: Option<bool>,
    pub memory: Option<LoopAccessMemory>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct LoopOutcome {
    pub id: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct LoopPhase {
    pub id: String,
    pub objective: String,
    pub access: Option<LoopPhaseAccess>,
    #[serde(default)]
    pub outcomes: Vec<LoopOutcome>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct LoopTransition {
    pub from: String,
    pub on: String,
    pub to: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
#[allow(dead_code)]
pub struct LoopLimits {
    pub max_steps: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct LoopCheckpoint {
    pub id: String,
    pub r#type: String,
    pub before_phase: String,
    pub on_reject: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum LoopToolFailureAction {
    Retry,
    FailPhase,
    Abort,
    Handoff,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum LoopToolFailureExhaustedAction {
    FailPhase,
    Abort,
    Handoff,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct LoopToolFailurePolicy {
    pub action: LoopToolFailureAction,
    pub max_retries: Option<u64>,
    pub on_exhausted: Option<LoopToolFailureExhaustedAction>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum LoopPhaseFailureAction {
    Abort,
    Handoff,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct LoopPhaseFailurePolicy {
    pub action: LoopPhaseFailureAction,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
#[allow(dead_code)]
pub struct LoopErrorPolicy {
    pub tool_failure: Option<LoopToolFailurePolicy>,
    pub phase_failure: Option<LoopPhaseFailurePolicy>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct LoopMetadata {
    pub archetype: Option<String>,
    pub entry_phase: String,
    pub limits: Option<LoopLimits>,
    pub phases: Vec<LoopPhase>,
    pub transitions: Vec<LoopTransition>,
    #[serde(default)]
    pub checkpoints: Vec<LoopCheckpoint>,
    pub error_policy: Option<LoopErrorPolicy>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct LoopManifest {
    pub kind: String,
    pub name: String,
    pub version: String,
    #[allow(dead_code)]
    pub description: Option<String>,
    pub readme: Option<String>,
    #[serde(default)]
    pub license: Option<Value>,
    pub r#loop: LoopMetadata,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct AgentMemoryBinding {
    pub package: String,
    #[serde(default)]
    pub spaces: Vec<String>,
    #[serde(default)]
    pub operations: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
#[allow(dead_code)]
pub struct AgentBindingScope {
    pub tools: Vec<String>,
    pub skills: Vec<String>,
    pub knowledge: Vec<String>,
    pub memory: Vec<AgentMemoryBinding>,
    pub profiles: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct AgentMcpBinding {
    pub id: String,
    pub tools: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct AgentConsumerContext {
    pub file: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
#[allow(dead_code)]
pub struct AgentBindings {
    pub global: Option<AgentBindingScope>,
    pub phases: HashMap<String, AgentBindingScope>,
    pub mcp: Vec<AgentMcpBinding>,
    pub consumer_context: Option<AgentConsumerContext>,
}

#[derive(Debug)]
pub enum PublishManifest {
    Tool(Box<ToolManifest>),
    Agent(Box<AgentManifest>),
    Template(Box<TemplateManifest>),
    Skill(Box<SkillManifest>),
    Knowledge(Box<KnowledgeManifest>),
    Memory(Box<MemoryManifest>),
    Profile(Box<ProfileManifest>),
}

/// Resolve the schema source (local file if present; else hosted URL)
pub fn resolve_schema_source(override_opt: Option<String>) -> String {
    if let Some(x) = override_opt {
        return x;
    }
    let local_path = PathBuf::from("schemas/agentpm.manifest.schema.json");
    if local_path.exists() {
        local_path.to_string_lossy().into_owned()
    } else {
        DEFAULT_MANIFEST_SCHEMA_URL.to_string()
    }
}

/// Load a JSON schema from a file or URL.
pub fn load_schema_value(source: &str) -> Result<Value> {
    if source == DEFAULT_MANIFEST_SCHEMA_URL {
        return Ok(serde_json::from_str(EMBEDDED_MANIFEST_SCHEMA_JSON)?);
    }
    if source.starts_with("http://") || source.starts_with("https://") {
        let resp = reqwest::blocking::get(source)
            .with_context(|| format!("fetching schema from {source}"))?;
        let text = resp.text()?;
        Ok(serde_json::from_str(&text)?)
    } else {
        let text = fs::read_to_string(source)
            .with_context(|| format!("reading schema from {}", source))?;
        Ok(serde_json::from_str(&text)?)
    }
}

/// Read and parse a manifest file.
pub fn load_manifest_value(path: &Path) -> Result<(Value, String)> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let value: Value = serde_json::from_str(&text)
        .with_context(|| format!("parsing JSON from {}", path.display()))?;
    Ok((value, text))
}

/// Validate a manifest `Value` against the schema and our semantic warnings.
/// If `fix == true`, this may mutate the `value` (e.g., auto-insert $schema).
/// Returns (ok, issues).
pub fn validate_manifest_value(
    schema_source: &str,
    file_label: &str,
    value: &mut Value,
    fix: bool,
) -> Result<(bool, Vec<LintIssue>)> {
    // Compile schema (keep simple for now; we can cache later if needed)
    let schema_value = load_schema_value(schema_source)?;
    let schema_static: &'static serde_json::Value = Box::leak(Box::new(schema_value));
    let compiled = JSONSchema::options()
        .with_draft(Draft::Draft202012)
        .compile(schema_static)?;

    let mut issues: Vec<LintIssue> = Vec::new();

    // Schema errors
    if let Err(errors) = compiled.validate(value) {
        for e in errors {
            issues.push(LintIssue {
                file: file_label.to_string(),
                level: "error",
                message: e.to_string(),
                instance_path: e.instance_path.to_string(),
                schema_path: e.schema_path.to_string(),
            });
        }
    }

    // Semantic warnings (mirror what `lint` had)
    if value.get("$schema").is_none() {
        issues.push(LintIssue {
            file: file_label.to_string(),
            level: "warning",
            message: "Missing $schema; editors may lack IntelliSense.".into(),
            instance_path: "".into(),
            schema_path: "".into(),
        });
        if fix && let Some(obj) = value.as_object_mut() {
            obj.insert("$schema".into(), Value::String(schema_source.to_string()));
        }
    }

    if let Some(Value::String(desc)) = value.get("description")
        && desc.trim().is_empty()
    {
        issues.push(LintIssue {
            file: file_label.to_string(),
            level: "warning",
            message: "`description` should not be empty".into(),
            instance_path: "/description".into(),
            schema_path: "".into(),
        });
    }

    if value.get("kind").and_then(Value::as_str) == Some("template")
        && let Some(Value::Object(template)) = value.get("template")
        && let Some(Value::Array(variables)) = template.get("variables")
    {
        let mut seen = HashSet::new();
        for (idx, variable) in variables.iter().enumerate() {
            if let Some(name) = variable.get("name").and_then(Value::as_str)
                && !seen.insert(name.to_string())
            {
                issues.push(LintIssue {
                    file: file_label.to_string(),
                    level: "error",
                    message: format!("duplicate template variable name `{name}`"),
                    instance_path: format!("/template/variables/{idx}/name"),
                    schema_path: "".into(),
                });
            }
        }
    }

    if let Some(Value::Object(runtime)) = value.get("runtime")
        && let Some(Value::String(runtime_type)) = runtime.get("type")
        && let Some(Value::Object(entrypoint)) = value.get("entrypoint")
    {
        let command = entrypoint
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let canon = canonical_interpreter(command);
        let runtime_interpreter = canonical_interpreter(runtime_type);

        if canon != runtime_interpreter && !is_interpreter_match(&runtime_interpreter, &canon) {
            issues.push(LintIssue {
                file: file_label.to_string(),
                level: "error",
                message: format!(
                    "`runtime.type` should match `entrypoint.command` ({} vs {})",
                    canon, runtime_interpreter
                ),
                instance_path: "/runtime/type".into(),
                schema_path: "".into(),
            });
        }
    }

    if value.get("kind").and_then(Value::as_str) == Some("memory")
        && let Ok(manifest) = parse_memory_manifest(value)
    {
        let manifest_path = resolve_existing_manifest_path(file_label);
        issues.extend(validate_memory_manifest_semantics(
            file_label,
            value,
            &manifest,
            manifest_path.as_deref(),
        ));
    }

    if value.get("kind").and_then(Value::as_str) == Some("profile")
        && let Ok(manifest) = parse_profile_manifest(value)
    {
        issues.extend(validate_profile_manifest_semantics(file_label, &manifest));
    }

    if value.get("kind").and_then(Value::as_str) == Some("loop")
        && let Ok(manifest) = parse_loop_manifest(value)
    {
        issues.extend(validate_loop_manifest_semantics(file_label, &manifest));
    }

    if value.get("kind").and_then(Value::as_str) == Some("agent")
        && let Ok(manifest) = serde_json::from_value::<AgentManifest>(value.clone())
    {
        issues.extend(validate_agent_bindings_semantics(file_label, &manifest));
    }

    let has_error = issues.iter().any(|i| i.level == "error");
    Ok((!has_error, issues))
}

fn resolve_existing_manifest_path(file_label: &str) -> Option<PathBuf> {
    let path = PathBuf::from(file_label);
    if !path.exists() {
        return None;
    }

    path.canonicalize().ok().or(Some(path))
}

fn validate_memory_manifest_semantics(
    file_label: &str,
    value: &Value,
    manifest: &MemoryManifest,
    manifest_path: Option<&Path>,
) -> Vec<LintIssue> {
    let mut issues = Vec::new();

    validate_memory_keys(
        file_label,
        "scopes",
        manifest.memory.scopes.keys(),
        &mut issues,
    );
    validate_memory_keys(
        file_label,
        "record_types",
        manifest.memory.record_types.keys(),
        &mut issues,
    );
    validate_memory_keys(
        file_label,
        "spaces",
        manifest.memory.spaces.keys(),
        &mut issues,
    );
    validate_memory_keys(
        file_label,
        "operations",
        manifest.memory.operations.keys(),
        &mut issues,
    );

    let mut pairings = HashSet::new();
    for (space_key, space) in &manifest.memory.spaces {
        validate_unique_strings(
            file_label,
            &format!("/memory/spaces/{space_key}/scope"),
            "scope",
            &space.scope,
            &mut issues,
        );
        validate_unique_strings(
            file_label,
            &format!("/memory/spaces/{space_key}/record_types"),
            "record type",
            &space.record_types,
            &mut issues,
        );
        validate_unique_retrieval_modes(file_label, space_key, &space.retrieval.modes, &mut issues);

        for (idx, scope_key) in space.scope.iter().enumerate() {
            if !manifest.memory.scopes.contains_key(scope_key) {
                push_manifest_error(
                    file_label,
                    &format!("/memory/spaces/{space_key}/scope/{idx}"),
                    format!("unknown scope `{scope_key}` referenced by space `{space_key}`"),
                    &mut issues,
                );
            }
        }

        for (idx, record_type_key) in space.record_types.iter().enumerate() {
            if !manifest.memory.record_types.contains_key(record_type_key) {
                push_manifest_error(
                    file_label,
                    &format!("/memory/spaces/{space_key}/record_types/{idx}"),
                    format!(
                        "unknown record type `{record_type_key}` referenced by space `{space_key}`"
                    ),
                    &mut issues,
                );
                continue;
            }
            if !pairings.insert((space_key.clone(), record_type_key.clone())) {
                push_manifest_error(
                    file_label,
                    &format!("/memory/spaces/{space_key}/record_types/{idx}"),
                    format!(
                        "duplicate space-and-record-type pairing `{space_key}` + `{record_type_key}`"
                    ),
                    &mut issues,
                );
            }
        }

        match space.model {
            MemorySpaceModel::Document => {
                if !space.retrieval.modes.contains(&MemoryRetrievalMode::Key) {
                    push_manifest_error(
                        file_label,
                        &format!("/memory/spaces/{space_key}/retrieval/modes"),
                        format!("document space `{space_key}` must include retrieval mode `key`"),
                        &mut issues,
                    );
                }
                if matches!(
                    space
                        .constraints
                        .as_ref()
                        .and_then(|constraints| constraints.append_only),
                    Some(true)
                ) {
                    push_manifest_error(
                        file_label,
                        &format!("/memory/spaces/{space_key}/constraints/append_only"),
                        format!("document space `{space_key}` cannot be append-only"),
                        &mut issues,
                    );
                }
            }
            MemorySpaceModel::Sequence => {
                if !space
                    .retrieval
                    .modes
                    .contains(&MemoryRetrievalMode::Chronological)
                {
                    push_manifest_error(
                        file_label,
                        &format!("/memory/spaces/{space_key}/retrieval/modes"),
                        format!(
                            "sequence space `{space_key}` must include retrieval mode `chronological`"
                        ),
                        &mut issues,
                    );
                }
            }
            MemorySpaceModel::Collection => {}
        }

        if let Some(capacity) = &space.capacity
            && capacity.max_records == 0
        {
            push_manifest_error(
                file_label,
                &format!("/memory/spaces/{space_key}/capacity/max_records"),
                format!("space `{space_key}` capacity.max_records must be greater than zero"),
                &mut issues,
            );
        }

        if let Some(retention) = &space.retention
            && !is_supported_positive_iso8601_duration(&retention.ttl)
        {
            push_manifest_error(
                file_label,
                &format!("/memory/spaces/{space_key}/retention/ttl"),
                format!(
                    "space `{space_key}` retention.ttl must use the supported positive ISO 8601 duration subset"
                ),
                &mut issues,
            );
        }
    }

    for (operation_key, operation) in &manifest.memory.operations {
        validate_memory_operation_raw(file_label, value, operation_key, &mut issues);

        match operation {
            MemoryOperation::Consolidate {
                inputs,
                output,
                source_handling: _,
                preserve_provenance: _,
                trigger,
                ..
            } => {
                validate_unique_operation_refs(
                    file_label,
                    operation_key,
                    inputs,
                    &format!("/memory/operations/{operation_key}/inputs"),
                    &mut issues,
                );
                validate_operation_refs(
                    file_label,
                    operation_key,
                    inputs,
                    &manifest.memory.spaces,
                    &format!("/memory/operations/{operation_key}/inputs"),
                    &mut issues,
                );
                validate_operation_ref(
                    file_label,
                    operation_key,
                    output,
                    &manifest.memory.spaces,
                    &format!("/memory/operations/{operation_key}/output"),
                    &mut issues,
                );
                validate_memory_trigger(
                    file_label,
                    operation_key,
                    trigger,
                    &manifest.memory.spaces,
                    &mut issues,
                );
            }
            MemoryOperation::Transform {
                inputs,
                output,
                source_handling: _,
                preserve_provenance: _,
                trigger,
                ..
            } => {
                if inputs.len() != 1 {
                    push_manifest_error(
                        file_label,
                        &format!("/memory/operations/{operation_key}/inputs"),
                        format!(
                            "transform operation `{operation_key}` must declare exactly one input pairing"
                        ),
                        &mut issues,
                    );
                }
                validate_unique_operation_refs(
                    file_label,
                    operation_key,
                    inputs,
                    &format!("/memory/operations/{operation_key}/inputs"),
                    &mut issues,
                );
                validate_operation_refs(
                    file_label,
                    operation_key,
                    inputs,
                    &manifest.memory.spaces,
                    &format!("/memory/operations/{operation_key}/inputs"),
                    &mut issues,
                );
                validate_operation_ref(
                    file_label,
                    operation_key,
                    output,
                    &manifest.memory.spaces,
                    &format!("/memory/operations/{operation_key}/output"),
                    &mut issues,
                );
                validate_memory_trigger(
                    file_label,
                    operation_key,
                    trigger,
                    &manifest.memory.spaces,
                    &mut issues,
                );
            }
            MemoryOperation::Delete {
                targets,
                trigger,
                cascade_derived_records: _,
                ..
            } => {
                validate_unique_operation_targets(file_label, operation_key, targets, &mut issues);
                for (idx, target) in targets.iter().enumerate() {
                    if !manifest.memory.spaces.contains_key(&target.space) {
                        push_manifest_error(
                            file_label,
                            &format!("/memory/operations/{operation_key}/targets/{idx}/space"),
                            format!(
                                "delete operation `{operation_key}` references unknown target space `{}`",
                                target.space
                            ),
                            &mut issues,
                        );
                    }
                }
                validate_memory_trigger(
                    file_label,
                    operation_key,
                    trigger,
                    &manifest.memory.spaces,
                    &mut issues,
                );
            }
        }
    }

    if let Some(manifest_path) = manifest_path
        && let Ok(package_root) = manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .canonicalize()
    {
        for (record_type_key, record_type) in &manifest.memory.record_types {
            match resolve_existing_relative_file(&package_root, &record_type.schema) {
                Ok(schema_path) => validate_source_schema_file(
                    file_label,
                    record_type_key,
                    &schema_path,
                    &mut issues,
                ),
                Err(err) => push_manifest_error(
                    file_label,
                    &format!("/memory/record_types/{record_type_key}/schema"),
                    format!(
                        "record type `{record_type_key}` schema `{}` is invalid: {err}",
                        record_type.schema
                    ),
                    &mut issues,
                ),
            }
        }
    }

    issues
}

fn validate_profile_manifest_semantics(
    file_label: &str,
    manifest: &ProfileManifest,
) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    let profile = &manifest.profile;

    validate_non_empty_trimmed_string(
        file_label,
        "/profile/identity/role",
        "identity.role",
        &profile.identity.role,
        &mut issues,
    );
    validate_optional_trimmed_string(
        file_label,
        "/profile/identity/description",
        "identity.description",
        profile.identity.description.as_deref(),
        &mut issues,
    );
    validate_trimmed_string_list(
        file_label,
        "/profile/identity/expertise",
        "identity.expertise",
        &profile.identity.expertise,
        &mut issues,
    );
    validate_trimmed_string_list(
        file_label,
        "/profile/objectives",
        "objectives",
        &profile.objectives,
        &mut issues,
    );
    validate_trimmed_string_list(
        file_label,
        "/profile/principles",
        "principles",
        &profile.principles,
        &mut issues,
    );

    if let Some(audience) = &profile.audience {
        validate_optional_trimmed_string(
            file_label,
            "/profile/audience/description",
            "audience.description",
            audience.description.as_deref(),
            &mut issues,
        );
        validate_optional_trimmed_string(
            file_label,
            "/profile/audience/assumed_knowledge",
            "audience.assumed_knowledge",
            audience.assumed_knowledge.as_deref(),
            &mut issues,
        );
        validate_trimmed_string_list(
            file_label,
            "/profile/audience/adaptation",
            "audience.adaptation",
            &audience.adaptation,
            &mut issues,
        );
    }

    validate_trimmed_string_list(
        file_label,
        "/profile/communication/tone",
        "communication.tone",
        &profile.communication.tone,
        &mut issues,
    );
    validate_trimmed_string_list(
        file_label,
        "/profile/communication/guidelines",
        "communication.guidelines",
        &profile.communication.guidelines,
        &mut issues,
    );
    validate_trimmed_string_list(
        file_label,
        "/profile/communication/formatting",
        "communication.formatting",
        &profile.communication.formatting,
        &mut issues,
    );

    if let Some(vocabulary) = &profile.communication.vocabulary {
        validate_profile_vocabulary(file_label, vocabulary, &mut issues);
    }

    validate_trimmed_string_list(
        file_label,
        "/profile/boundaries",
        "boundaries",
        &profile.boundaries,
        &mut issues,
    );
    validate_profile_constraints(file_label, &profile.constraints, &mut issues);

    issues
}

fn validate_loop_manifest_semantics(file_label: &str, manifest: &LoopManifest) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    let loop_metadata = &manifest.r#loop;

    validate_optional_trimmed_string(
        file_label,
        "/loop/archetype",
        "loop.archetype",
        loop_metadata.archetype.as_deref(),
        &mut issues,
    );

    let mut phase_index_by_id: HashMap<String, usize> = HashMap::new();
    let mut phase_outcomes: HashMap<String, HashSet<String>> = HashMap::new();

    for (phase_idx, phase) in loop_metadata.phases.iter().enumerate() {
        validate_non_empty_trimmed_string(
            file_label,
            &format!("/loop/phases/{phase_idx}/objective"),
            "loop.phases[].objective",
            &phase.objective,
            &mut issues,
        );

        if let Some(previous_idx) = phase_index_by_id.insert(phase.id.clone(), phase_idx) {
            push_manifest_error(
                file_label,
                &format!("/loop/phases/{phase_idx}/id"),
                format!(
                    "duplicate loop phase id `{}` is not allowed (already declared at index {previous_idx})",
                    phase.id
                ),
                &mut issues,
            );
        }

        let mut outcomes = HashSet::new();
        if phase.outcomes.is_empty() {
            outcomes.insert("complete".to_string());
        } else {
            for (outcome_idx, outcome) in phase.outcomes.iter().enumerate() {
                validate_non_empty_trimmed_string(
                    file_label,
                    &format!("/loop/phases/{phase_idx}/outcomes/{outcome_idx}/description"),
                    "loop.phases[].outcomes[].description",
                    &outcome.description,
                    &mut issues,
                );
                if !outcomes.insert(outcome.id.clone()) {
                    push_manifest_error(
                        file_label,
                        &format!("/loop/phases/{phase_idx}/outcomes/{outcome_idx}/id"),
                        format!(
                            "duplicate loop outcome id `{}` is not allowed within phase `{}`",
                            outcome.id, phase.id
                        ),
                        &mut issues,
                    );
                }
            }
        }
        phase_outcomes.insert(phase.id.clone(), outcomes);
    }

    if !phase_index_by_id.contains_key(&loop_metadata.entry_phase) {
        push_manifest_error(
            file_label,
            "/loop/entry_phase",
            format!(
                "loop entry_phase `{}` must match a declared phase id",
                loop_metadata.entry_phase
            ),
            &mut issues,
        );
    }

    let mut transition_counts: HashMap<(String, String), usize> = HashMap::new();
    let mut phase_edges: HashMap<String, Vec<String>> = HashMap::new();
    let mut terminal_targets = HashSet::new();

    for (transition_idx, transition) in loop_metadata.transitions.iter().enumerate() {
        let pointer = format!("/loop/transitions/{transition_idx}");
        if !phase_index_by_id.contains_key(&transition.from) {
            push_manifest_error(
                file_label,
                &format!("{pointer}/from"),
                format!(
                    "transition source phase `{}` is not declared",
                    transition.from
                ),
                &mut issues,
            );
            continue;
        }

        if !phase_outcomes
            .get(&transition.from)
            .is_some_and(|outcomes| outcomes.contains(&transition.on))
        {
            push_manifest_error(
                file_label,
                &format!("{pointer}/on"),
                format!(
                    "transition outcome `{}` is not valid for phase `{}`",
                    transition.on, transition.from
                ),
                &mut issues,
            );
        }

        if phase_index_by_id.contains_key(&transition.to) {
            phase_edges
                .entry(transition.from.clone())
                .or_default()
                .push(transition.to.clone());
        } else if is_loop_terminal(&transition.to) {
            terminal_targets.insert(transition.to.clone());
        } else {
            push_manifest_error(
                file_label,
                &format!("{pointer}/to"),
                format!(
                    "transition destination `{}` must be a declared phase or supported terminal",
                    transition.to
                ),
                &mut issues,
            );
        }

        *transition_counts
            .entry((transition.from.clone(), transition.on.clone()))
            .or_insert(0) += 1;
    }

    for phase in &loop_metadata.phases {
        if let Some(outcomes) = phase_outcomes.get(&phase.id) {
            for outcome in outcomes {
                match transition_counts
                    .get(&(phase.id.clone(), outcome.clone()))
                    .copied()
                {
                    Some(1) => {}
                    Some(count) => push_manifest_error(
                        file_label,
                        "/loop/transitions",
                        format!(
                            "phase `{}` outcome `{}` must have exactly one transition (found {count})",
                            phase.id, outcome
                        ),
                        &mut issues,
                    ),
                    None => push_manifest_error(
                        file_label,
                        "/loop/transitions",
                        format!(
                            "phase `{}` outcome `{}` must declare exactly one transition",
                            phase.id, outcome
                        ),
                        &mut issues,
                    ),
                }
            }
        }
    }

    let mut reachable_terminals = HashSet::new();
    if phase_index_by_id.contains_key(&loop_metadata.entry_phase) {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::from([loop_metadata.entry_phase.clone()]);

        while let Some(current) = queue.pop_front() {
            if !visited.insert(current.clone()) {
                continue;
            }
            for transition in loop_metadata
                .transitions
                .iter()
                .filter(|transition| transition.from == current)
            {
                if phase_index_by_id.contains_key(&transition.to) {
                    queue.push_back(transition.to.clone());
                } else if is_loop_terminal(&transition.to) {
                    reachable_terminals.insert(transition.to.clone());
                }
            }
        }

        for (phase_id, phase_idx) in &phase_index_by_id {
            if !visited.contains(phase_id) {
                push_manifest_error(
                    file_label,
                    &format!("/loop/phases/{phase_idx}/id"),
                    format!("phase `{phase_id}` is unreachable from entry_phase"),
                    &mut issues,
                );
            }
        }
    }

    if terminal_targets.is_empty() || reachable_terminals.is_empty() {
        push_manifest_error(
            file_label,
            "/loop/transitions",
            "loop must be able to reach at least one terminal outcome".to_string(),
            &mut issues,
        );
    }

    let mut checkpoint_ids = HashSet::new();
    let mut approval_targets = HashSet::new();
    for (checkpoint_idx, checkpoint) in loop_metadata.checkpoints.iter().enumerate() {
        if !checkpoint_ids.insert(checkpoint.id.clone()) {
            push_manifest_error(
                file_label,
                &format!("/loop/checkpoints/{checkpoint_idx}/id"),
                format!("duplicate checkpoint id `{}` is not allowed", checkpoint.id),
                &mut issues,
            );
        }

        if !phase_index_by_id.contains_key(&checkpoint.before_phase) {
            push_manifest_error(
                file_label,
                &format!("/loop/checkpoints/{checkpoint_idx}/before_phase"),
                format!(
                    "checkpoint target phase `{}` must match a declared phase id",
                    checkpoint.before_phase
                ),
                &mut issues,
            );
        }

        if !(phase_index_by_id.contains_key(&checkpoint.on_reject)
            || is_loop_terminal(&checkpoint.on_reject))
        {
            push_manifest_error(
                file_label,
                &format!("/loop/checkpoints/{checkpoint_idx}/on_reject"),
                format!(
                    "checkpoint on_reject target `{}` must be a declared phase or supported terminal",
                    checkpoint.on_reject
                ),
                &mut issues,
            );
        }

        if checkpoint.r#type == "approval"
            && !approval_targets.insert(checkpoint.before_phase.clone())
        {
            push_manifest_error(
                file_label,
                &format!("/loop/checkpoints/{checkpoint_idx}/before_phase"),
                format!(
                    "multiple approval checkpoints cannot target phase `{}`",
                    checkpoint.before_phase
                ),
                &mut issues,
            );
        }
    }

    if let Some(error_policy) = &loop_metadata.error_policy
        && let Some(tool_failure) = &error_policy.tool_failure
    {
        match tool_failure.action {
            LoopToolFailureAction::Retry => {
                if tool_failure.max_retries.is_none() {
                    push_manifest_error(
                        file_label,
                        "/loop/error_policy/tool_failure/max_retries",
                        "tool_failure.max_retries is required when action is `retry`".to_string(),
                        &mut issues,
                    );
                }
                if tool_failure.on_exhausted.is_none() {
                    push_manifest_error(
                        file_label,
                        "/loop/error_policy/tool_failure/on_exhausted",
                        "tool_failure.on_exhausted is required when action is `retry`".to_string(),
                        &mut issues,
                    );
                }
            }
            _ => {
                if tool_failure.max_retries.is_some() {
                    push_manifest_error(
                        file_label,
                        "/loop/error_policy/tool_failure/max_retries",
                        "tool_failure.max_retries is allowed only when action is `retry`"
                            .to_string(),
                        &mut issues,
                    );
                }
                if tool_failure.on_exhausted.is_some() {
                    push_manifest_error(
                        file_label,
                        "/loop/error_policy/tool_failure/on_exhausted",
                        "tool_failure.on_exhausted is allowed only when action is `retry`"
                            .to_string(),
                        &mut issues,
                    );
                }
            }
        }

        let can_fail_phase = matches!(tool_failure.action, LoopToolFailureAction::FailPhase)
            || matches!(
                tool_failure.on_exhausted,
                Some(LoopToolFailureExhaustedAction::FailPhase)
            );
        if can_fail_phase && error_policy.phase_failure.is_none() {
            push_manifest_error(
                file_label,
                "/loop/error_policy/phase_failure",
                "phase_failure is required when tool_failure can fail the current phase"
                    .to_string(),
                &mut issues,
            );
        }
    }

    issues
}

fn validate_agent_bindings_semantics(file_label: &str, manifest: &AgentManifest) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    let Some(bindings) = &manifest.bindings else {
        return issues;
    };

    if manifest.loop_ref.is_none() && !bindings.phases.is_empty() {
        push_manifest_error(
            file_label,
            "/bindings/phases",
            "bindings.phases requires the agent to declare a top-level `loop` dependency"
                .to_string(),
            &mut issues,
        );
    }

    let allowed = AgentBindingAllowedSets {
        tools: canonical_package_reference_set(&manifest.tools),
        skills: canonical_package_reference_set(&manifest.skills),
        knowledge: canonical_package_reference_set(&manifest.knowledge),
        memory: canonical_package_reference_set(&manifest.memory),
        profiles: canonical_package_reference_set(&manifest.profiles),
    };

    if let Some(global) = &bindings.global {
        validate_agent_binding_scope(
            file_label,
            "/bindings/global",
            global,
            &allowed,
            &mut issues,
        );
    }

    for (phase_name, scope) in &bindings.phases {
        validate_agent_binding_scope(
            file_label,
            &format!("/bindings/phases/{phase_name}"),
            scope,
            &allowed,
            &mut issues,
        );
    }

    let mut seen_mcp_ids = HashSet::new();
    for (binding_idx, binding) in bindings.mcp.iter().enumerate() {
        if !seen_mcp_ids.insert(binding.id.clone()) {
            push_manifest_error(
                file_label,
                &format!("/bindings/mcp/{binding_idx}/id"),
                format!("duplicate MCP binding id `{}` is not allowed", binding.id),
                &mut issues,
            );
        }
        for (tool_idx, tool) in binding.tools.iter().enumerate() {
            if !allowed.tools.contains(tool) {
                push_manifest_error(
                    file_label,
                    &format!("/bindings/mcp/{binding_idx}/tools/{tool_idx}"),
                    format!(
                        "tool binding `{tool}` must match a top-level agent tool dependency by package identity"
                    ),
                    &mut issues,
                );
            }
        }
    }

    issues
}

fn validate_non_empty_trimmed_string(
    file_label: &str,
    pointer: &str,
    label: &str,
    value: &str,
    issues: &mut Vec<LintIssue>,
) {
    if value.trim().is_empty() {
        push_manifest_error(
            file_label,
            pointer,
            format!("`{label}` must not be empty after trimming"),
            issues,
        );
    }
}

fn validate_optional_trimmed_string(
    file_label: &str,
    pointer: &str,
    label: &str,
    value: Option<&str>,
    issues: &mut Vec<LintIssue>,
) {
    if let Some(value) = value {
        validate_non_empty_trimmed_string(file_label, pointer, label, value, issues);
    }
}

fn validate_trimmed_string_list(
    file_label: &str,
    pointer: &str,
    label: &str,
    values: &[String],
    issues: &mut Vec<LintIssue>,
) {
    for (idx, value) in values.iter().enumerate() {
        validate_non_empty_trimmed_string(
            file_label,
            &json_pointer_child(pointer, &idx.to_string()),
            label,
            value,
            issues,
        );
    }
}

fn normalize_profile_term(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn validate_profile_vocabulary(
    file_label: &str,
    vocabulary: &ProfileVocabulary,
    issues: &mut Vec<LintIssue>,
) {
    validate_trimmed_string_list(
        file_label,
        "/profile/communication/vocabulary/prefer",
        "communication.vocabulary.prefer",
        &vocabulary.prefer,
        issues,
    );
    validate_trimmed_string_list(
        file_label,
        "/profile/communication/vocabulary/avoid",
        "communication.vocabulary.avoid",
        &vocabulary.avoid,
        issues,
    );

    let mut prefer_seen: HashMap<String, usize> = HashMap::new();
    for (idx, value) in vocabulary.prefer.iter().enumerate() {
        let normalized = normalize_profile_term(value);
        if normalized.is_empty() {
            continue;
        }
        match prefer_seen.entry(normalized) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(idx);
            }
            std::collections::hash_map::Entry::Occupied(_) => {
                push_manifest_error(
                    file_label,
                    &format!("/profile/communication/vocabulary/prefer/{idx}"),
                    "normalized duplicate term in `communication.vocabulary.prefer` is not allowed"
                        .into(),
                    issues,
                );
            }
        }
    }

    let mut avoid_seen: HashMap<String, usize> = HashMap::new();
    for (idx, value) in vocabulary.avoid.iter().enumerate() {
        let normalized = normalize_profile_term(value);
        if normalized.is_empty() {
            continue;
        }
        if prefer_seen.contains_key(&normalized) {
            push_manifest_error(
                file_label,
                &format!("/profile/communication/vocabulary/avoid/{idx}"),
                "term must not appear in both `communication.vocabulary.prefer` and `communication.vocabulary.avoid` after trimming and case-folding".into(),
                issues,
            );
        }
        match avoid_seen.entry(normalized) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(idx);
            }
            std::collections::hash_map::Entry::Occupied(_) => {
                push_manifest_error(
                    file_label,
                    &format!("/profile/communication/vocabulary/avoid/{idx}"),
                    "normalized duplicate term in `communication.vocabulary.avoid` is not allowed"
                        .into(),
                    issues,
                );
            }
        }
    }
}

fn validate_profile_constraints(
    file_label: &str,
    constraints: &[ProfileConstraint],
    issues: &mut Vec<LintIssue>,
) {
    let mut seen = HashSet::new();
    for (idx, constraint) in constraints.iter().enumerate() {
        validate_non_empty_trimmed_string(
            file_label,
            &format!("/profile/constraints/{idx}/instruction"),
            "constraint instruction",
            &constraint.instruction,
            issues,
        );
        if !seen.insert(constraint.id.clone()) {
            push_manifest_error(
                file_label,
                &format!("/profile/constraints/{idx}/id"),
                format!("duplicate constraint id `{}` is not allowed", constraint.id),
                issues,
            );
        }
    }
}

struct AgentBindingAllowedSets {
    tools: HashSet<String>,
    skills: HashSet<String>,
    knowledge: HashSet<String>,
    memory: HashSet<String>,
    profiles: HashSet<String>,
}

fn validate_agent_binding_scope(
    file_label: &str,
    pointer: &str,
    scope: &AgentBindingScope,
    allowed: &AgentBindingAllowedSets,
    issues: &mut Vec<LintIssue>,
) {
    validate_bound_package_strings(
        file_label,
        &json_pointer_child(pointer, "tools"),
        "tool",
        &scope.tools,
        &allowed.tools,
        issues,
    );
    validate_bound_package_strings(
        file_label,
        &json_pointer_child(pointer, "skills"),
        "skill",
        &scope.skills,
        &allowed.skills,
        issues,
    );
    validate_bound_package_strings(
        file_label,
        &json_pointer_child(pointer, "knowledge"),
        "knowledge package",
        &scope.knowledge,
        &allowed.knowledge,
        issues,
    );
    validate_bound_package_strings(
        file_label,
        &json_pointer_child(pointer, "profiles"),
        "profile package",
        &scope.profiles,
        &allowed.profiles,
        issues,
    );

    let mut seen_memory_packages = HashSet::new();
    for (memory_idx, binding) in scope.memory.iter().enumerate() {
        if !allowed.memory.contains(&binding.package) {
            push_manifest_error(
                file_label,
                &format!("{pointer}/memory/{memory_idx}/package"),
                format!(
                    "memory binding `{}` must match a top-level agent memory dependency by package identity",
                    binding.package
                ),
                issues,
            );
        }
        if !seen_memory_packages.insert(binding.package.clone()) {
            push_manifest_error(
                file_label,
                &format!("{pointer}/memory/{memory_idx}/package"),
                format!(
                    "duplicate memory binding package `{}` is not allowed within one binding scope",
                    binding.package
                ),
                issues,
            );
        }
    }
}

fn validate_bound_package_strings(
    file_label: &str,
    pointer: &str,
    label: &str,
    values: &[String],
    allowed: &HashSet<String>,
    issues: &mut Vec<LintIssue>,
) {
    for (idx, value) in values.iter().enumerate() {
        if !allowed.contains(value) {
            push_manifest_error(
                file_label,
                &json_pointer_child(pointer, &idx.to_string()),
                format!(
                    "{label} binding `{value}` must match a top-level dependency by package identity"
                ),
                issues,
            );
        }
    }
}

fn canonical_package_reference_set(values: &[PackageReference]) -> HashSet<String> {
    values
        .iter()
        .map(package_reference_identity)
        .collect::<HashSet<_>>()
}

fn package_reference_identity(reference: &PackageReference) -> String {
    match reference {
        PackageReference::String(value) => package_identity(value),
        PackageReference::Object { name, .. } => package_identity(name),
    }
}

fn package_identity(value: &str) -> String {
    let slash_idx = value.rfind('/').unwrap_or(0);
    match value[slash_idx..].rfind('@') {
        Some(relative_idx) if slash_idx + relative_idx > 0 => {
            value[..slash_idx + relative_idx].to_string()
        }
        _ => value.to_string(),
    }
}

fn is_loop_terminal(value: &str) -> bool {
    matches!(value, "$end" | "$abort" | "$handoff")
}

fn validate_memory_keys<'a>(
    file_label: &str,
    section: &str,
    keys: impl Iterator<Item = &'a String>,
    issues: &mut Vec<LintIssue>,
) {
    for key in keys {
        if !is_valid_memory_key(key) {
            push_manifest_error(
                file_label,
                &format!("/memory/{section}/{key}"),
                format!("`{key}` is not a valid Memory Blueprint identifier"),
                issues,
            );
        }
    }
}

fn validate_unique_strings(
    file_label: &str,
    pointer: &str,
    label: &str,
    values: &[String],
    issues: &mut Vec<LintIssue>,
) {
    let mut seen = HashSet::new();
    for value in values {
        if !seen.insert(value.clone()) {
            push_manifest_error(
                file_label,
                pointer,
                format!("duplicate {label} `{value}` is not allowed"),
                issues,
            );
        }
    }
}

fn validate_unique_retrieval_modes(
    file_label: &str,
    space_key: &str,
    values: &[MemoryRetrievalMode],
    issues: &mut Vec<LintIssue>,
) {
    let mut seen = HashSet::new();
    for value in values {
        if !seen.insert(value.clone()) {
            push_manifest_error(
                file_label,
                &format!("/memory/spaces/{space_key}/retrieval/modes"),
                format!(
                    "duplicate retrieval mode `{}` is not allowed",
                    memory_retrieval_mode_name(value)
                ),
                issues,
            );
        }
    }
}

fn memory_retrieval_mode_name(value: &MemoryRetrievalMode) -> &'static str {
    match value {
        MemoryRetrievalMode::Key => "key",
        MemoryRetrievalMode::Filter => "filter",
        MemoryRetrievalMode::Chronological => "chronological",
        MemoryRetrievalMode::FullText => "full_text",
        MemoryRetrievalMode::Semantic => "semantic",
    }
}

fn validate_unique_operation_refs(
    file_label: &str,
    operation_key: &str,
    refs: &[MemoryOperationRef],
    pointer: &str,
    issues: &mut Vec<LintIssue>,
) {
    let mut seen = HashSet::new();
    for reference in refs {
        let identity = (reference.space.clone(), reference.record_type.clone());
        if !seen.insert(identity.clone()) {
            push_manifest_error(
                file_label,
                pointer,
                format!(
                    "operation `{operation_key}` repeats input pair `{}` + `{}`",
                    identity.0, identity.1
                ),
                issues,
            );
        }
    }
}

fn validate_unique_operation_targets(
    file_label: &str,
    operation_key: &str,
    targets: &[MemoryOperationTarget],
    issues: &mut Vec<LintIssue>,
) {
    let mut seen = HashSet::new();
    for target in targets {
        if !seen.insert(target.space.clone()) {
            push_manifest_error(
                file_label,
                &format!("/memory/operations/{operation_key}/targets"),
                format!(
                    "delete operation `{operation_key}` repeats target space `{}`",
                    target.space
                ),
                issues,
            );
        }
    }
}

fn validate_operation_refs(
    file_label: &str,
    operation_key: &str,
    refs: &[MemoryOperationRef],
    spaces: &HashMap<String, MemorySpace>,
    pointer: &str,
    issues: &mut Vec<LintIssue>,
) {
    for reference in refs {
        validate_operation_ref(
            file_label,
            operation_key,
            reference,
            spaces,
            pointer,
            issues,
        );
    }
}

fn validate_operation_ref(
    file_label: &str,
    operation_key: &str,
    reference: &MemoryOperationRef,
    spaces: &HashMap<String, MemorySpace>,
    pointer: &str,
    issues: &mut Vec<LintIssue>,
) {
    let Some(space) = spaces.get(&reference.space) else {
        push_manifest_error(
            file_label,
            pointer,
            format!(
                "operation `{operation_key}` references unknown space `{}`",
                reference.space
            ),
            issues,
        );
        return;
    };

    if !space.record_types.contains(&reference.record_type) {
        push_manifest_error(
            file_label,
            pointer,
            format!(
                "operation `{operation_key}` references record type `{}` that is not permitted by space `{}`",
                reference.record_type, reference.space
            ),
            issues,
        );
    }
}

fn validate_memory_trigger(
    file_label: &str,
    operation_key: &str,
    trigger: &MemoryTrigger,
    spaces: &HashMap<String, MemorySpace>,
    issues: &mut Vec<LintIssue>,
) {
    match trigger {
        MemoryTrigger::External => {}
        MemoryTrigger::RecordCount { space, threshold } => {
            if *threshold == 0 {
                push_manifest_error(
                    file_label,
                    &format!("/memory/operations/{operation_key}/trigger/threshold"),
                    format!(
                        "record_count trigger for operation `{operation_key}` must use a positive threshold"
                    ),
                    issues,
                );
            }
            if !spaces.contains_key(space) {
                push_manifest_error(
                    file_label,
                    &format!("/memory/operations/{operation_key}/trigger/space"),
                    format!(
                        "record_count trigger for operation `{operation_key}` references unknown space `{space}`"
                    ),
                    issues,
                );
            }
        }
        MemoryTrigger::Capacity { space } => match spaces.get(space) {
            Some(target_space)
                if target_space
                    .capacity
                    .as_ref()
                    .map(|capacity| capacity.max_records > 0)
                    .unwrap_or(false) => {}
            Some(_) => push_manifest_error(
                file_label,
                &format!("/memory/operations/{operation_key}/trigger/space"),
                format!(
                    "capacity trigger for operation `{operation_key}` requires space `{space}` to declare capacity.max_records"
                ),
                issues,
            ),
            None => push_manifest_error(
                file_label,
                &format!("/memory/operations/{operation_key}/trigger/space"),
                format!(
                    "capacity trigger for operation `{operation_key}` references unknown space `{space}`"
                ),
                issues,
            ),
        },
        MemoryTrigger::Interval { every } => {
            if !is_supported_positive_iso8601_duration(every) {
                push_manifest_error(
                    file_label,
                    &format!("/memory/operations/{operation_key}/trigger/every"),
                    format!(
                        "interval trigger for operation `{operation_key}` must use the supported positive ISO 8601 duration subset"
                    ),
                    issues,
                );
            }
        }
    }
}

fn validate_memory_operation_raw(
    file_label: &str,
    value: &Value,
    operation_key: &str,
    issues: &mut Vec<LintIssue>,
) {
    let Some(operation) = value
        .get("memory")
        .and_then(Value::as_object)
        .and_then(|memory| memory.get("operations"))
        .and_then(Value::as_object)
        .and_then(|operations| operations.get(operation_key))
        .and_then(Value::as_object)
    else {
        return;
    };

    let Some(operation_type) = operation.get("type").and_then(Value::as_str) else {
        return;
    };

    let required: &[&str];
    let forbidden: &[&str];
    match operation_type {
        "consolidate" => {
            required = &[
                "type",
                "description",
                "trigger",
                "inputs",
                "output",
                "source_handling",
                "preserve_provenance",
            ];
            forbidden = &["targets", "cascade_derived_records"];
        }
        "transform" => {
            required = &[
                "type",
                "description",
                "trigger",
                "inputs",
                "output",
                "source_handling",
                "preserve_provenance",
            ];
            forbidden = &["targets", "cascade_derived_records"];
        }
        "delete" => {
            required = &[
                "type",
                "description",
                "trigger",
                "targets",
                "cascade_derived_records",
            ];
            forbidden = &["inputs", "output", "source_handling", "preserve_provenance"];
        }
        _ => return,
    }

    for field in required {
        if !operation.contains_key(*field) {
            push_manifest_error(
                file_label,
                &format!("/memory/operations/{operation_key}"),
                format!("operation `{operation_key}` is missing required field `{field}`"),
                issues,
            );
        }
    }

    for field in forbidden {
        if operation.contains_key(*field) {
            push_manifest_error(
                file_label,
                &format!("/memory/operations/{operation_key}/{field}"),
                format!(
                    "operation `{operation_key}` of type `{operation_type}` must not declare `{field}`"
                ),
                issues,
            );
        }
    }

    if let Some(trigger) = operation.get("trigger").and_then(Value::as_object)
        && let Some(trigger_type) = trigger.get("type").and_then(Value::as_str)
    {
        let allowed_fields: &[&str] = match trigger_type {
            "external" => &["type"],
            "record_count" => &["type", "space", "threshold"],
            "capacity" => &["type", "space"],
            "interval" => &["type", "every"],
            _ => return,
        };

        for key in trigger.keys() {
            if !allowed_fields.contains(&key.as_str()) {
                push_manifest_error(
                    file_label,
                    &format!("/memory/operations/{operation_key}/trigger/{key}"),
                    format!(
                        "trigger type `{trigger_type}` for operation `{operation_key}` must not declare `{key}`"
                    ),
                    issues,
                );
            }
        }
    }
}

fn validate_source_schema_file(
    _file_label: &str,
    _record_type_key: &str,
    schema_path: &Path,
    issues: &mut Vec<LintIssue>,
) {
    let schema_file = schema_path.to_string_lossy().to_string();
    let text = match fs::read_to_string(schema_path) {
        Ok(text) => text,
        Err(err) => {
            issues.push(LintIssue {
                file: schema_file,
                level: "error",
                message: format!("failed to read source schema: {err}"),
                instance_path: "".into(),
                schema_path: "".into(),
            });
            return;
        }
    };

    let schema_value: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(err) => {
            issues.push(LintIssue {
                file: schema_file,
                level: "error",
                message: format!("source schema is not valid JSON: {err}"),
                instance_path: "".into(),
                schema_path: "".into(),
            });
            return;
        }
    };

    let schema_static: &'static Value = Box::leak(Box::new(schema_value.clone()));
    if let Err(err) = JSONSchema::options()
        .with_draft(Draft::Draft202012)
        .compile(schema_static)
    {
        issues.push(LintIssue {
            file: schema_file.clone(),
            level: "error",
            message: format!("source schema is not valid JSON Schema Draft 2020-12: {err}"),
            instance_path: "".into(),
            schema_path: "".into(),
        });
    }

    validate_source_schema_tree(&schema_file, "", &schema_value, issues);
}

fn validate_source_schema_tree(
    schema_file: &str,
    pointer: &str,
    value: &Value,
    issues: &mut Vec<LintIssue>,
) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_pointer = json_pointer_child(pointer, key);
                if key.starts_with("x-agentpm-") {
                    match key.as_str() {
                        "x-agentpm-data-class" => {
                            if !matches!(
                                child.as_str(),
                                Some(
                                    "public"
                                        | "internal"
                                        | "personal"
                                        | "authentication"
                                        | "financial"
                                        | "health"
                                        | "legal"
                                        | "operational"
                                        | "other"
                                )
                            ) {
                                issues.push(LintIssue {
                                    file: schema_file.to_string(),
                                    level: "error",
                                    message: format!(
                                        "`{key}` must be one of the supported AgentPM data-class values"
                                    ),
                                    instance_path: child_pointer.clone(),
                                    schema_path: "".into(),
                                });
                            }
                        }
                        "x-agentpm-sensitivity" => {
                            if !matches!(
                                child.as_str(),
                                Some("low" | "moderate" | "high" | "critical")
                            ) {
                                issues.push(LintIssue {
                                    file: schema_file.to_string(),
                                    level: "error",
                                    message: format!(
                                        "`{key}` must be one of the supported AgentPM sensitivity values"
                                    ),
                                    instance_path: child_pointer.clone(),
                                    schema_path: "".into(),
                                });
                            }
                        }
                        "x-agentpm-persist" | "x-agentpm-shareable" => {
                            if !child.is_boolean() {
                                issues.push(LintIssue {
                                    file: schema_file.to_string(),
                                    level: "error",
                                    message: format!("`{key}` must be a boolean"),
                                    instance_path: child_pointer.clone(),
                                    schema_path: "".into(),
                                });
                            }
                        }
                        _ => issues.push(LintIssue {
                            file: schema_file.to_string(),
                            level: "error",
                            message: format!("unsupported AgentPM governance keyword `{key}`"),
                            instance_path: child_pointer.clone(),
                            schema_path: "".into(),
                        }),
                    }
                }

                if key == "$ref"
                    && let Some(reference) = child.as_str()
                    && !reference.starts_with('#')
                {
                    issues.push(LintIssue {
                        file: schema_file.to_string(),
                        level: "error",
                        message: "only in-document `#...` JSON Schema references are supported in Memory Blueprint source schemas".into(),
                        instance_path: child_pointer.clone(),
                        schema_path: "".into(),
                    });
                }

                validate_source_schema_tree(schema_file, &child_pointer, child, issues);
            }
        }
        Value::Array(items) => {
            for (idx, child) in items.iter().enumerate() {
                validate_source_schema_tree(
                    schema_file,
                    &json_pointer_child(pointer, &idx.to_string()),
                    child,
                    issues,
                );
            }
        }
        _ => {}
    }
}

pub(crate) fn resolve_existing_relative_file(root: &Path, relative: &str) -> Result<PathBuf> {
    let safe_rel = parse_safe_relative_path(relative)?;
    let canonical_root = root
        .canonicalize()
        .with_context(|| format!("reading package root {}", root.display()))?;
    let candidate = canonical_root.join(&safe_rel);
    let resolved = candidate
        .canonicalize()
        .with_context(|| format!("reading {}", candidate.display()))?;
    if !resolved.starts_with(&canonical_root) {
        return Err(anyhow!(
            "resolved path escapes the package root: {}",
            candidate.display()
        ));
    }
    if !resolved.is_file() {
        return Err(anyhow!("not a file: {}", candidate.display()));
    }
    Ok(resolved)
}

pub(crate) fn parse_safe_relative_path(path: &str) -> Result<PathBuf> {
    if path.trim().is_empty() {
        bail!("path must not be empty");
    }

    let parsed = PathBuf::from(path);
    if parsed.is_absolute() {
        bail!("path must be package-relative");
    }

    for component in parsed.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir => {}
            Component::ParentDir => bail!("path must not contain `..`"),
            Component::RootDir | Component::Prefix(_) => bail!("path must be package-relative"),
        }
    }

    Ok(parsed)
}

fn push_manifest_error(
    file_label: &str,
    instance_path: &str,
    message: String,
    issues: &mut Vec<LintIssue>,
) {
    issues.push(LintIssue {
        file: file_label.to_string(),
        level: "error",
        message,
        instance_path: instance_path.to_string(),
        schema_path: "".into(),
    });
}

fn json_pointer_child(base: &str, key: &str) -> String {
    let escaped = key.replace('~', "~0").replace('/', "~1");
    if base.is_empty() {
        format!("/{escaped}")
    } else {
        format!("{base}/{escaped}")
    }
}

fn is_valid_memory_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

fn is_supported_positive_iso8601_duration(value: &str) -> bool {
    if !value.starts_with('P') {
        return false;
    }
    let body = &value[1..];
    if body.is_empty() {
        return false;
    }
    if let Some(weeks) = body.strip_suffix('W') {
        return is_positive_integer(weeks);
    }

    let (date_part, time_part) = match body.split_once('T') {
        Some((date, time)) => (date, Some(time)),
        None => (body, None),
    };

    let mut seen_any = false;
    let mut seen_positive = false;

    if !date_part.is_empty()
        && !consume_duration_section(date_part, &['D'], &mut seen_any, &mut seen_positive)
    {
        return false;
    }

    if let Some(time_part) = time_part {
        if time_part.is_empty() {
            return false;
        }
        if !consume_duration_section(
            time_part,
            &['H', 'M', 'S'],
            &mut seen_any,
            &mut seen_positive,
        ) {
            return false;
        }
    }

    seen_any && seen_positive
}

fn consume_duration_section(
    section: &str,
    allowed_units: &[char],
    seen_any: &mut bool,
    seen_positive: &mut bool,
) -> bool {
    let mut idx = 0usize;
    let bytes = section.as_bytes();
    let mut used_units = HashSet::new();

    while idx < bytes.len() {
        let start = idx;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        if start == idx || idx >= bytes.len() {
            return false;
        }

        let value = &section[start..idx];
        let unit = section.as_bytes()[idx] as char;
        idx += 1;

        if !allowed_units.contains(&unit) || !used_units.insert(unit) {
            return false;
        }
        *seen_any = true;
        if !is_positive_integer(value) {
            return false;
        }
        if value != "0" {
            *seen_positive = true;
        }
    }

    true
}

fn is_positive_integer(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|ch| ch.is_ascii_digit())
        && value.parse::<u64>().map(|num| num > 0).unwrap_or(false)
}

fn canonical_interpreter(cmd: &str) -> String {
    let base = cmd.to_lowercase();

    base.strip_suffix(".exe")
        .or_else(|| base.strip_suffix(".cmd"))
        .or_else(|| base.strip_suffix(".bat"))
        .unwrap_or(&base)
        .to_string()
}

fn is_interpreter_match(runtime: &str, command: &str) -> bool {
    if runtime == command {
        return true;
    }

    // map runtime -> acceptable command names
    let mut aliases: HashMap<&str, &[&str]> = HashMap::new();
    aliases.insert("python", &["python3"]);
    aliases.insert("node", &["nodejs"]);

    if let Some(cmds) = aliases.get(runtime) {
        cmds.contains(&command)
    } else {
        false
    }
}

/// Discover manifest files from CLI args (dirs/globs/files).
pub fn discover_manifest_files(paths: &[String]) -> Result<Vec<PathBuf>> {
    // Default: ./agent.json
    if paths.is_empty() {
        let p = PathBuf::from("agent.json");
        return Ok(if p.exists() { vec![p] } else { vec![] });
    }

    let mut out = Vec::new();
    for raw in paths {
        let p = PathBuf::from(raw);
        if p.is_dir() {
            let candidate = p.join("agent.json");
            if candidate.exists() {
                out.push(candidate);
            }
        } else if p.file_name().map(|f| f == "agent.json").unwrap_or(false) && p.exists() {
            out.push(p);
        } else {
            // Glob-ish: allow users to pass things like "**/agent.json"
            if let Ok(paths) = glob::glob(raw) {
                for entry in paths.flatten() {
                    if entry
                        .file_name()
                        .map(|f| f == "agent.json")
                        .unwrap_or(false)
                        && entry.exists()
                    {
                        out.push(entry);
                    }
                }
            }
        }
    }
    Ok(out)
}

/// Write back a (possibly fixed) manifest in pretty JSON.
pub fn write_manifest_pretty(path: &Path, value: &Value) -> Result<()> {
    let pretty = serde_json::to_string_pretty(value)?;
    fs::write(path, pretty + "\n")
        .with_context(|| format!("failed to write fixed file {}", path.display()))?;
    Ok(())
}

pub fn write_manifest_pretty_atomic(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    // Serialize with trailing newline
    let mut data = serde_json::to_vec_pretty(value)?;
    if !data.ends_with(b"\n") {
        data.push(b'\n');
    }

    // Write to .tmp then rename
    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp)
            .with_context(|| format!("opening temp file {}", tmp.display()))?;
        f.write_all(&data)
            .with_context(|| format!("writing {}", tmp.display()))?;
        let _ = f.sync_all(); // best-effort
    }

    // On Windows, replace existing file safely
    if path.exists() {
        let _ = fs::remove_file(path);
    }
    fs::rename(&tmp, path)
        .with_context(|| format!("renaming {} -> {}", tmp.display(), path.display()))?;

    // Best-effort fsync of the directory
    if let Some(parent) = path.parent()
        && let Ok(dirf) = fs::File::open(parent)
    {
        let _ = dirf.sync_all();
    }

    Ok(())
}

/// Parse the strongly typed Tool manifest; enforce kind = "tool" here if desired.
pub fn parse_tool_manifest(value: &Value) -> Result<ToolManifest> {
    let mf: ToolManifest =
        serde_json::from_value(value.clone()).context("parsing manifest into ToolManifest")?;
    if mf.kind != "tool" {
        return Err(anyhow!(format!(
            "parse_tool_manifest requires kind=\"tool\" (got kind=\"{}\"); `agentpm publish` supports kind=\"tool\", \"skill\", \"knowledge\", \"memory\", \"agent\", and \"template\"",
            mf.kind
        )));
    }
    Ok(mf)
}

pub fn parse_publish_manifest(value: &Value) -> Result<PublishManifest> {
    let kind = value
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("manifest must include kind"))?;

    match kind {
        "tool" => Ok(PublishManifest::Tool(Box::new(parse_tool_manifest(value)?))),
        "agent" => {
            let mf: AgentManifest = serde_json::from_value(value.clone())
                .context("parsing manifest into AgentManifest")?;
            Ok(PublishManifest::Agent(Box::new(mf)))
        }
        "template" => Ok(PublishManifest::Template(Box::new(
            parse_template_manifest(value)?,
        ))),
        "skill" => Ok(PublishManifest::Skill(Box::new(parse_skill_manifest(
            value,
        )?))),
        "knowledge" => Ok(PublishManifest::Knowledge(Box::new(
            parse_knowledge_manifest(value)?,
        ))),
        "memory" => Ok(PublishManifest::Memory(Box::new(parse_memory_manifest(
            value,
        )?))),
        "profile" => Ok(PublishManifest::Profile(Box::new(parse_profile_manifest(
            value,
        )?))),
        other => Err(anyhow!(format!(
            "`agentpm publish` supports kind=\"tool\", kind=\"agent\", kind=\"template\", kind=\"skill\", kind=\"knowledge\", kind=\"memory\", and kind=\"profile\" manifests (got kind=\"{}\")",
            other
        ))),
    }
}

#[allow(dead_code)]
pub fn parse_template_manifest(value: &Value) -> Result<TemplateManifest> {
    let mf: TemplateManifest =
        serde_json::from_value(value.clone()).context("parsing manifest into TemplateManifest")?;
    if mf.kind != "template" {
        return Err(anyhow!(format!(
            "expected kind=\"template\" manifest (got kind=\"{}\")",
            mf.kind
        )));
    }
    Ok(mf)
}

pub fn parse_skill_manifest(value: &Value) -> Result<SkillManifest> {
    let mf: SkillManifest =
        serde_json::from_value(value.clone()).context("parsing manifest into SkillManifest")?;
    if mf.kind != "skill" {
        return Err(anyhow!(format!(
            "expected kind=\"skill\" manifest (got kind=\"{}\")",
            mf.kind
        )));
    }
    Ok(mf)
}

pub fn parse_knowledge_manifest(value: &Value) -> Result<KnowledgeManifest> {
    let mf: KnowledgeManifest =
        serde_json::from_value(value.clone()).context("parsing manifest into KnowledgeManifest")?;
    if mf.kind != "knowledge" {
        return Err(anyhow!(format!(
            "expected kind=\"knowledge\" manifest (got kind=\"{}\")",
            mf.kind
        )));
    }
    Ok(mf)
}

pub fn parse_memory_manifest(value: &Value) -> Result<MemoryManifest> {
    let mf: MemoryManifest =
        serde_json::from_value(value.clone()).context("parsing manifest into MemoryManifest")?;
    if mf.kind != "memory" {
        return Err(anyhow!(format!(
            "expected kind=\"memory\" manifest (got kind=\"{}\")",
            mf.kind
        )));
    }
    Ok(mf)
}

#[allow(dead_code)]
pub fn parse_loop_manifest(value: &Value) -> Result<LoopManifest> {
    let mf: LoopManifest =
        serde_json::from_value(value.clone()).context("parsing manifest into LoopManifest")?;
    if mf.kind != "loop" {
        return Err(anyhow!(format!(
            "expected kind=\"loop\" manifest (got kind=\"{}\")",
            mf.kind
        )));
    }
    Ok(mf)
}

#[allow(dead_code)]
pub fn parse_profile_manifest(value: &Value) -> Result<ProfileManifest> {
    let mf: ProfileManifest =
        serde_json::from_value(value.clone()).context("parsing manifest into ProfileManifest")?;
    if mf.kind != "profile" {
        return Err(anyhow!(format!(
            "expected kind=\"profile\" manifest (got kind=\"{}\")",
            mf.kind
        )));
    }
    Ok(mf)
}

/// Lock files
pub fn write_lock<P: AsRef<Path>>(dir: P, lock: &Lock) -> Result<()> {
    let path = dir.as_ref().join("agent.lock");
    let v: Value = serde_json::to_value(lock)?; // convert to Value
    write_manifest_pretty_atomic(&path, &v) // reuse atomic writer
}

pub fn read_lock_or_default<P: AsRef<Path>>(dir: P) -> Result<Lock> {
    let path = dir.as_ref().join("agent.lock");
    if path.exists() {
        let data = fs::read(path)?;
        let lock: Lock = serde_json::from_slice(&data)?;
        Ok(lock)
    } else {
        Ok(Lock::empty_v2())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn schema_path() -> String {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../schemas/agentpm.manifest.schema.json")
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn load_schema_value_uses_embedded_default_schema_for_hosted_url() {
        let schema = load_schema_value(DEFAULT_MANIFEST_SCHEMA_URL).unwrap();
        assert_eq!(
            schema.get("$schema").and_then(Value::as_str),
            Some("https://json-schema.org/draft/2020-12/schema")
        );
        assert_eq!(
            schema
                .get("properties")
                .and_then(|props| props.get("kind"))
                .and_then(|kind| kind.get("enum"))
                .and_then(Value::as_array)
                .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>())
                .unwrap(),
            vec![
                "agent",
                "tool",
                "template",
                "skill",
                "knowledge",
                "memory",
                "profile",
                "loop",
            ]
        );
    }

    fn assert_manifest_ok(mut manifest: Value) {
        let (ok, issues) =
            validate_manifest_value(&schema_path(), "agent.json", &mut manifest, false).unwrap();
        assert!(ok, "expected manifest to validate, got issues: {issues:#?}");
    }

    fn assert_manifest_invalid(mut manifest: Value) -> Vec<LintIssue> {
        let (ok, issues) =
            validate_manifest_value(&schema_path(), "agent.json", &mut manifest, false).unwrap();
        assert!(!ok, "expected manifest to fail validation");
        issues
    }

    fn temp_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("agentpm-{label}-{nanos}.json"))
    }

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("agentpm-{label}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_fixture_file(dir: &Path, relative: &str, contents: &str) {
        let path = dir.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }

    fn assert_manifest_file_ok(dir: &Path, manifest: Value, extra_files: &[(&str, &str)]) {
        for (relative, contents) in extra_files {
            write_fixture_file(dir, relative, contents);
        }
        let manifest_path = dir.join("agent.json");
        write_manifest_pretty(&manifest_path, &manifest).unwrap();
        let (mut loaded, _) = load_manifest_value(&manifest_path).unwrap();
        let (ok, issues) = validate_manifest_value(
            &schema_path(),
            &manifest_path.to_string_lossy(),
            &mut loaded,
            false,
        )
        .unwrap();
        assert!(ok, "expected manifest to validate, got issues: {issues:#?}");
    }

    fn assert_manifest_file_invalid(
        dir: &Path,
        manifest: Value,
        extra_files: &[(&str, &str)],
    ) -> Vec<LintIssue> {
        for (relative, contents) in extra_files {
            write_fixture_file(dir, relative, contents);
        }
        let manifest_path = dir.join("agent.json");
        write_manifest_pretty(&manifest_path, &manifest).unwrap();
        let (mut loaded, _) = load_manifest_value(&manifest_path).unwrap();
        let (ok, issues) = validate_manifest_value(
            &schema_path(),
            &manifest_path.to_string_lossy(),
            &mut loaded,
            false,
        )
        .unwrap();
        assert!(!ok, "expected manifest to fail validation");
        issues
    }

    fn base_memory_manifest() -> Value {
        json!({
            "kind": "memory",
            "name": "conversation-continuity",
            "version": "0.1.0",
            "description": "Portable structure for conversational continuity memory.",
            "memory": {
                "scopes": {
                    "user": {
                        "description": "The user whose memory is being retained."
                    }
                },
                "record_types": {
                    "user_preference": {
                        "version": "1.0.0",
                        "description": "Durable structured preferences for one user.",
                        "schema": "schemas/user-preference.schema.json"
                    }
                },
                "spaces": {
                    "profile": {
                        "description": "The current durable profile for one user.",
                        "model": "document",
                        "record_types": ["user_preference"],
                        "scope": ["user"],
                        "retrieval": {
                            "modes": ["key"]
                        }
                    }
                }
            }
        })
    }

    fn base_loop_manifest() -> Value {
        json!({
            "kind": "loop",
            "name": "incident-response-loop",
            "version": "1.0.0",
            "description": "A bounded triage, investigation, review, and response loop.",
            "loop": {
                "entry_phase": "triage",
                "phases": [
                    {
                        "id": "triage",
                        "objective": "Assess the incident."
                    }
                ],
                "transitions": [
                    {
                        "from": "triage",
                        "on": "complete",
                        "to": "$end"
                    }
                ]
            }
        })
    }

    fn base_profile_manifest() -> Value {
        json!({
            "kind": "profile",
            "name": "customer-success-advocate",
            "version": "1.0.0",
            "description": "A warm, professional behavior profile for customer-facing support agents.",
            "profile": {
                "identity": {
                    "role": "Senior Customer Success Advocate"
                },
                "objectives": [
                    "Help customers reach a clear resolution or next step."
                ],
                "communication": {
                    "tone": ["warm"],
                    "verbosity": "concise"
                }
            }
        })
    }

    fn valid_memory_schema() -> &'static str {
        r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "favorite_color": {
      "type": "string",
      "x-agentpm-data-class": "personal",
      "x-agentpm-sensitivity": "moderate",
      "x-agentpm-persist": true,
      "x-agentpm-shareable": false
    }
  },
  "required": ["favorite_color"],
  "additionalProperties": false
}
"#
    }

    #[test]
    fn valid_tool_manifest_validates() {
        assert_manifest_ok(json!({
            "kind": "tool",
            "name": "capitalize",
            "version": "0.1.0",
            "description": "Uppercase text.",
            "entrypoint": {
                "command": "python",
                "args": ["capitalize.py"]
            },
            "inputs": {
                "type": "object",
                "properties": {
                    "text": { "type": "string" }
                },
                "required": ["text"]
            },
            "outputs": {
                "type": "object",
                "properties": {
                    "upper": { "type": "string" }
                },
                "required": ["upper"]
            },
            "files": ["capitalize.py"],
            "runtime": {
                "type": "python",
                "version": "3.12"
            }
        }));
    }

    #[test]
    fn valid_agent_manifest_validates_with_examples_and_reserved_fields() {
        assert_manifest_ok(json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "description": "Triage support requests using installed tools.",
            "tools": [
                "@zack/slack-post-message@0.1.0",
                {
                    "name": "@zack/github-issues",
                    "version": "0.2.3"
                }
            ],
            "skills": ["@zack/support-triage-skill@0.1.0"],
            "knowledge": [
                {
                    "name": "@zack/internal-playbooks",
                    "version": "0.1.0"
                }
            ],
            "memory": [],
            "profiles": [],
            "examples": [
                {
                    "title": "Triage an incident",
                    "prompt": "Summarize this incident and draft a follow-up issue."
                }
            ]
        }));
    }

    #[test]
    fn valid_procedural_skill_manifest_validates_without_tools() {
        assert_manifest_ok(json!({
            "kind": "skill",
            "name": "incident-commander",
            "version": "0.1.0",
            "description": "Incident response coordination playbook.",
            "skill": {
                "entrypoint": "SKILL.md"
            }
        }));
    }

    #[test]
    fn valid_tool_backed_skill_manifest_validates() {
        assert_manifest_ok(json!({
            "kind": "skill",
            "name": "slack-incident-update",
            "version": "0.1.0",
            "description": "A playbook for posting structured incident updates to Slack.",
            "tools": [
                {
                    "name": "@zack/slack-post-message",
                    "version": "0.1.1"
                }
            ],
            "skill": {
                "entrypoint": "SKILL.md",
                "references": [
                    "references/tool-contract.md",
                    "references/examples.md"
                ],
                "scripts": ["scripts/run.sh"],
                "compatibility": {
                    "runtimes": ["agentpm-run", "shell"]
                }
            }
        }));
    }

    #[test]
    fn skill_manifest_rejects_unknown_compatibility_runtime() {
        let issues = assert_manifest_invalid(json!({
            "kind": "skill",
            "name": "invalid-compatibility-runtime",
            "version": "0.1.0",
            "description": "Unknown compatibility runtime should fail schema validation.",
            "skill": {
                "entrypoint": "SKILL.md",
                "compatibility": {
                    "runtimes": ["whatever"]
                }
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/skill/compatibility/runtimes/0"),
            "expected invalid compatibility runtime failure, got: {issues:#?}"
        );
    }

    #[test]
    fn valid_template_manifest_validates() {
        assert_manifest_ok(json!({
            "kind": "template",
            "name": "research-assistant-python",
            "version": "0.1.0",
            "description": "Python SDK starter for a local research assistant.",
            "template": {
                "display_name": "Python Research Assistant",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "stack": ["python"],
                "files_root": "template",
                "variables": [
                    {
                        "name": "project_name",
                        "description": "Generated project name. Do not use for secrets.",
                        "required": true,
                        "default": "research-assistant"
                    }
                ],
                "dependencies": {
                    "tools": [
                        {
                            "name": "@zack/web-page-extract",
                            "version": "0.1.2"
                        }
                    ],
                    "agents": []
                },
                "entrypoints": [
                    {
                        "label": "Run locally",
                        "command": "python main.py \"AgentPM\""
                    }
                ]
            }
        }));
    }

    #[test]
    fn valid_template_manifest_with_skill_dependencies_validates() {
        assert_manifest_ok(json!({
            "kind": "template",
            "name": "incident-response-workspace",
            "version": "0.1.0",
            "description": "Workspace starter with a first-class skill dependency.",
            "template": {
                "display_name": "Incident Response Workspace",
                "use_case": "incident-response",
                "execution_surfaces": ["multi-agent-workspace"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": [],
                    "skills": [
                        {
                            "name": "@zack/incident-commander",
                            "version": "0.1.0"
                        }
                    ]
                },
                "entrypoints": [
                    {
                        "label": "Open workspace",
                        "command": "agentpm run"
                    }
                ]
            }
        }));
    }

    #[test]
    fn valid_agent_manifest_with_knowledge_dependencies_validates() {
        assert_manifest_ok(json!({
            "kind": "agent",
            "name": "research-agent",
            "version": "0.1.0",
            "description": "Agent with a knowledge dependency.",
            "tools": [],
            "knowledge": [
                { "name": "@zack/python-docs", "version": "0.1.0" }
            ]
        }));
    }

    #[test]
    fn valid_agent_manifest_with_profile_dependencies_validates() {
        assert_manifest_ok(json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "description": "Agent with Instruction Profile dependencies.",
            "tools": [],
            "profiles": [
                "@zack/customer-success-advocate@1.0.0",
                {
                    "name": "@zack/escalation-manager",
                    "version": "1.0.0"
                }
            ]
        }));
    }

    #[test]
    fn valid_template_manifest_with_knowledge_dependencies_validates() {
        assert_manifest_ok(json!({
            "kind": "template",
            "name": "knowledge-workspace",
            "version": "0.1.0",
            "description": "Workspace starter with a first-class knowledge dependency.",
            "template": {
                "display_name": "Knowledge Workspace",
                "use_case": "research",
                "execution_surfaces": ["multi-agent-workspace"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": [],
                    "knowledge": [
                        {
                            "name": "@zack/python-docs",
                            "version": "0.1.0"
                        }
                    ]
                },
                "entrypoints": [
                    {
                        "label": "Open workspace",
                        "command": "agentpm run"
                    }
                ]
            }
        }));
    }

    #[test]
    fn valid_template_manifest_with_profile_dependencies_validates() {
        assert_manifest_ok(json!({
            "kind": "template",
            "name": "support-workspace",
            "version": "0.1.0",
            "description": "Workspace starter with Instruction Profile dependencies.",
            "template": {
                "display_name": "Support Workspace",
                "use_case": "support",
                "execution_surfaces": ["multi-agent-workspace"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": [],
                    "profiles": [
                        "@zack/customer-success-advocate@1.0.0"
                    ]
                },
                "entrypoints": [
                    {
                        "label": "Open workspace",
                        "command": "agentpm run"
                    }
                ]
            }
        }));
    }

    #[test]
    fn valid_minimal_profile_manifest_validates() {
        assert_manifest_ok(base_profile_manifest());
    }

    #[test]
    fn valid_full_profile_manifest_validates_and_parses() {
        let manifest = json!({
            "kind": "profile",
            "name": "customer-success-advocate",
            "version": "1.0.0",
            "description": "A warm, professional behavior profile for customer-facing SaaS support agents.",
            "readme": "README.md",
            "license": {
                "spdx": "MIT",
                "file": "LICENSE"
            },
            "profile": {
                "identity": {
                    "role": "Senior Customer Success Advocate",
                    "description": "Represents the company while helping customers resolve product and account issues.",
                    "expertise": [
                        "Customer communication",
                        "Subscription billing"
                    ]
                },
                "objectives": [
                    "Help customers reach a clear resolution or next step.",
                    "Protect customer trust and sensitive information."
                ],
                "principles": [
                    "Acknowledge uncertainty instead of presenting guesses as facts."
                ],
                "audience": {
                    "description": "Customers ranging from non-technical administrators to experienced developers.",
                    "assumed_knowledge": "Basic familiarity with common software interfaces.",
                    "adaptation": [
                        "Match the technical depth demonstrated by the customer."
                    ]
                },
                "communication": {
                    "tone": ["warm", "professional", "solution-oriented"],
                    "verbosity": "balanced",
                    "guidelines": [
                        "State the most useful next action early."
                    ],
                    "formatting": [
                        "Use numbered lists when presenting multiple actions."
                    ],
                    "vocabulary": {
                        "prefer": ["resolve", "next step"],
                        "avoid": ["obviously", "as an AI"]
                    }
                },
                "boundaries": [
                    "Do not claim access to systems that are not actually available."
                ],
                "constraints": [
                    {
                        "id": "protect-authentication-data",
                        "strength": "required",
                        "instruction": "Never request a raw password."
                    },
                    {
                        "id": "avoid-generic-ai-disclaimers",
                        "strength": "preferred",
                        "instruction": "Do not introduce responses with generic AI disclaimers."
                    }
                ],
                "compatibility": {
                    "minimum_context_tokens": 8000,
                    "requires": {
                        "tool_use": false,
                        "structured_output": false,
                        "multimodal_input": false
                    },
                    "recommends": {
                        "tool_use": true,
                        "structured_output": true,
                        "multimodal_input": false
                    }
                }
            }
        });

        assert_manifest_ok(manifest.clone());
        let parsed = parse_profile_manifest(&manifest).unwrap();
        assert_eq!(parsed.kind, "profile");
        assert_eq!(
            parsed.profile.identity.role,
            "Senior Customer Success Advocate"
        );
        assert_eq!(
            parsed.profile.communication.verbosity,
            ProfileVerbosity::Balanced
        );
        assert_eq!(parsed.profile.constraints.len(), 2);
    }

    #[test]
    fn profile_manifest_rejects_missing_required_core_fields() {
        let cases = vec![
            (
                json!({
                    "kind": "profile",
                    "name": "missing-profile",
                    "version": "1.0.0",
                    "description": "Missing profile object."
                }),
                "/oneOf",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "missing-identity",
                    "version": "1.0.0",
                    "description": "Missing identity.",
                    "profile": {
                        "objectives": ["Help."],
                        "communication": {
                            "tone": ["warm"],
                            "verbosity": "concise"
                        }
                    }
                }),
                "/properties/profile",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "missing-role",
                    "version": "1.0.0",
                    "description": "Missing role.",
                    "profile": {
                        "identity": {},
                        "objectives": ["Help."],
                        "communication": {
                            "tone": ["warm"],
                            "verbosity": "concise"
                        }
                    }
                }),
                "/profile/identity",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "missing-objectives",
                    "version": "1.0.0",
                    "description": "Missing objectives.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "communication": {
                            "tone": ["warm"],
                            "verbosity": "concise"
                        }
                    }
                }),
                "/profile",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "missing-communication",
                    "version": "1.0.0",
                    "description": "Missing communication.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."]
                    }
                }),
                "/profile",
            ),
        ];

        for (manifest, expected_path) in cases {
            let issues = assert_manifest_invalid(manifest);
            assert!(
                issues.iter().any(|issue| {
                    issue.instance_path == expected_path
                        || issue.schema_path.contains(expected_path)
                }),
                "expected missing core field failure at {expected_path}, got: {issues:#?}"
            );
        }
    }

    #[test]
    fn profile_manifest_rejects_empty_required_and_optional_fields() {
        let cases = vec![
            (
                json!({
                    "kind": "profile",
                    "name": "empty-objectives",
                    "version": "1.0.0",
                    "description": "Objectives must not be empty.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": [],
                        "communication": { "tone": ["warm"], "verbosity": "concise" }
                    }
                }),
                "/profile/objectives",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "empty-tone",
                    "version": "1.0.0",
                    "description": "Tone must not be empty.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."],
                        "communication": { "tone": [], "verbosity": "concise" }
                    }
                }),
                "/profile/communication/tone",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "empty-audience",
                    "version": "1.0.0",
                    "description": "Audience must not be empty when present.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."],
                        "audience": {},
                        "communication": { "tone": ["warm"], "verbosity": "concise" }
                    }
                }),
                "/profile/audience",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "empty-vocabulary",
                    "version": "1.0.0",
                    "description": "Vocabulary must not be empty when present.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."],
                        "communication": {
                            "tone": ["warm"],
                            "verbosity": "concise",
                            "vocabulary": {}
                        }
                    }
                }),
                "/profile/communication/vocabulary",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "empty-compatibility",
                    "version": "1.0.0",
                    "description": "Compatibility must not be empty when present.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."],
                        "communication": { "tone": ["warm"], "verbosity": "concise" },
                        "compatibility": {}
                    }
                }),
                "/profile/compatibility",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "empty-principles",
                    "version": "1.0.0",
                    "description": "Principles must not be empty when present.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."],
                        "principles": [],
                        "communication": { "tone": ["warm"], "verbosity": "concise" }
                    }
                }),
                "/profile/principles",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "empty-boundaries",
                    "version": "1.0.0",
                    "description": "Boundaries must not be empty when present.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."],
                        "communication": { "tone": ["warm"], "verbosity": "concise" },
                        "boundaries": []
                    }
                }),
                "/profile/boundaries",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "empty-constraints",
                    "version": "1.0.0",
                    "description": "Constraints must not be empty when present.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."],
                        "communication": { "tone": ["warm"], "verbosity": "concise" },
                        "constraints": []
                    }
                }),
                "/profile/constraints",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "empty-expertise",
                    "version": "1.0.0",
                    "description": "Identity expertise must not be empty when present.",
                    "profile": {
                        "identity": {
                            "role": "Support agent",
                            "expertise": []
                        },
                        "objectives": ["Help."],
                        "communication": { "tone": ["warm"], "verbosity": "concise" }
                    }
                }),
                "/profile/identity/expertise",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "empty-guidelines",
                    "version": "1.0.0",
                    "description": "Communication guidelines must not be empty when present.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."],
                        "communication": {
                            "tone": ["warm"],
                            "verbosity": "concise",
                            "guidelines": []
                        }
                    }
                }),
                "/profile/communication/guidelines",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "empty-formatting",
                    "version": "1.0.0",
                    "description": "Communication formatting must not be empty when present.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."],
                        "communication": {
                            "tone": ["warm"],
                            "verbosity": "concise",
                            "formatting": []
                        }
                    }
                }),
                "/profile/communication/formatting",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "empty-adaptation",
                    "version": "1.0.0",
                    "description": "Audience adaptation must not be empty when present.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."],
                        "audience": {
                            "adaptation": []
                        },
                        "communication": { "tone": ["warm"], "verbosity": "concise" }
                    }
                }),
                "/profile/audience/adaptation",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "empty-prefer",
                    "version": "1.0.0",
                    "description": "Vocabulary prefer terms must not be empty when present.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."],
                        "communication": {
                            "tone": ["warm"],
                            "verbosity": "concise",
                            "vocabulary": {
                                "prefer": []
                            }
                        }
                    }
                }),
                "/profile/communication/vocabulary/prefer",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "empty-avoid",
                    "version": "1.0.0",
                    "description": "Vocabulary avoid terms must not be empty when present.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."],
                        "communication": {
                            "tone": ["warm"],
                            "verbosity": "concise",
                            "vocabulary": {
                                "avoid": []
                            }
                        }
                    }
                }),
                "/profile/communication/vocabulary/avoid",
            ),
        ];

        for (manifest, expected_path) in cases {
            let issues = assert_manifest_invalid(manifest);
            assert!(
                issues
                    .iter()
                    .any(|issue| issue.instance_path == expected_path),
                "expected empty-field failure at {expected_path}, got: {issues:#?}"
            );
        }
    }

    #[test]
    fn profile_manifest_rejects_invalid_enums_constraint_ids_and_duplicates() {
        let constraint_id_cases = vec![
            "Bad-Constraint",
            "bad_constraint",
            "-bad-constraint",
            "bad-constraint-",
            "bad--constraint",
            "a2345678901234567890123456789012345678901234567890123456789012345",
        ];
        for invalid_id in constraint_id_cases {
            let mut manifest = base_profile_manifest();
            manifest["profile"]["constraints"] = json!([
                {
                    "id": invalid_id,
                    "strength": "required",
                    "instruction": "Never expose secrets."
                }
            ]);
            let issues = assert_manifest_invalid(manifest);
            assert!(
                issues
                    .iter()
                    .any(|issue| issue.instance_path == "/profile/constraints/0/id"),
                "expected invalid constraint id failure for {invalid_id}, got: {issues:#?}"
            );
        }

        let cases = vec![
            (
                json!({
                    "kind": "profile",
                    "name": "bad-verbosity",
                    "version": "1.0.0",
                    "description": "Invalid verbosity should fail.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."],
                        "communication": { "tone": ["warm"], "verbosity": "brief" }
                    }
                }),
                "/profile/communication/verbosity",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "bad-strength",
                    "version": "1.0.0",
                    "description": "Invalid constraint strength should fail.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."],
                        "communication": { "tone": ["warm"], "verbosity": "concise" },
                        "constraints": [
                            {
                                "id": "protect-secrets",
                                "strength": "advisory",
                                "instruction": "Never expose secrets."
                            }
                        ]
                    }
                }),
                "/profile/constraints/0/strength",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "empty-instruction",
                    "version": "1.0.0",
                    "description": "Empty constraint instruction should fail.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."],
                        "communication": { "tone": ["warm"], "verbosity": "concise" },
                        "constraints": [
                            {
                                "id": "protect-secrets",
                                "strength": "required",
                                "instruction": ""
                            }
                        ]
                    }
                }),
                "/profile/constraints/0/instruction",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "duplicate-objectives",
                    "version": "1.0.0",
                    "description": "Duplicate objectives should fail.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help.", "Help."],
                        "communication": { "tone": ["warm"], "verbosity": "concise" }
                    }
                }),
                "/profile/objectives",
            ),
        ];

        for (manifest, expected_path) in cases {
            let issues = assert_manifest_invalid(manifest);
            assert!(
                issues
                    .iter()
                    .any(|issue| issue.instance_path == expected_path),
                "expected profile validation failure at {expected_path}, got: {issues:#?}"
            );
        }
    }

    #[test]
    fn profile_manifest_rejects_invalid_compatibility_metadata() {
        let cases = vec![
            (
                json!({
                    "kind": "profile",
                    "name": "zero-min-context",
                    "version": "1.0.0",
                    "description": "Zero minimum context should fail.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."],
                        "communication": { "tone": ["warm"], "verbosity": "concise" },
                        "compatibility": {
                            "minimum_context_tokens": 0
                        }
                    }
                }),
                "/profile/compatibility/minimum_context_tokens",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "negative-min-context",
                    "version": "1.0.0",
                    "description": "Negative minimum context should fail.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."],
                        "communication": { "tone": ["warm"], "verbosity": "concise" },
                        "compatibility": {
                            "minimum_context_tokens": -1
                        }
                    }
                }),
                "/profile/compatibility/minimum_context_tokens",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "unknown-capability",
                    "version": "1.0.0",
                    "description": "Unknown capability names should fail.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."],
                        "communication": { "tone": ["warm"], "verbosity": "concise" },
                        "compatibility": {
                            "requires": {
                                "telepathy": true
                            }
                        }
                    }
                }),
                "/profile/compatibility/requires",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "non-boolean-capability",
                    "version": "1.0.0",
                    "description": "Capability values must be booleans.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."],
                        "communication": { "tone": ["warm"], "verbosity": "concise" },
                        "compatibility": {
                            "recommends": {
                                "tool_use": "yes"
                            }
                        }
                    }
                }),
                "/profile/compatibility/recommends/tool_use",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "empty-capability-group",
                    "version": "1.0.0",
                    "description": "Capability groups must not be empty.",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."],
                        "communication": { "tone": ["warm"], "verbosity": "concise" },
                        "compatibility": {
                            "requires": {}
                        }
                    }
                }),
                "/profile/compatibility/requires",
            ),
        ];

        for (manifest, expected_path) in cases {
            let issues = assert_manifest_invalid(manifest);
            assert!(
                issues
                    .iter()
                    .any(|issue| issue.instance_path == expected_path),
                "expected compatibility failure at {expected_path}, got: {issues:#?}"
            );
        }
    }

    #[test]
    fn profile_manifest_rejects_additional_properties_and_display_name() {
        let cases = vec![
            (
                json!({
                    "kind": "profile",
                    "name": "bad-display-name",
                    "version": "1.0.0",
                    "description": "Profiles must not declare display_name.",
                    "display_name": "Support Persona",
                    "profile": {
                        "identity": { "role": "Support agent" },
                        "objectives": ["Help."],
                        "communication": { "tone": ["warm"], "verbosity": "concise" }
                    }
                }),
                "/display_name",
            ),
            (
                json!({
                    "kind": "profile",
                    "name": "bad-extra-property",
                    "version": "1.0.0",
                    "description": "Unsupported nested profile properties should fail.",
                    "profile": {
                        "identity": {
                            "role": "Support agent",
                            "alias": "helper"
                        },
                        "objectives": ["Help."],
                        "communication": { "tone": ["warm"], "verbosity": "concise" }
                    }
                }),
                "/profile/identity",
            ),
        ];

        for (manifest, expected_path) in cases {
            let issues = assert_manifest_invalid(manifest);
            assert!(
                issues
                    .iter()
                    .any(|issue| issue.instance_path == expected_path
                        || issue.instance_path.is_empty()),
                "expected additional-property rejection at {expected_path}, got: {issues:#?}"
            );
        }
    }

    #[test]
    fn valid_agent_manifest_with_memory_dependencies_validates() {
        assert_manifest_ok(json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "description": "Agent with a memory blueprint dependency.",
            "tools": [],
            "memory": [
                { "name": "@zack/conversation-continuity", "version": "0.1.0" }
            ]
        }));
    }

    #[test]
    fn valid_template_manifest_with_memory_dependencies_validates() {
        assert_manifest_ok(json!({
            "kind": "template",
            "name": "memory-workspace",
            "version": "0.1.0",
            "description": "Workspace starter with a Memory Blueprint dependency.",
            "template": {
                "display_name": "Memory Workspace",
                "use_case": "assistant",
                "execution_surfaces": ["multi-agent-workspace"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": [],
                    "memory": [
                        {
                            "name": "@zack/conversation-continuity",
                            "version": "0.1.0"
                        }
                    ]
                },
                "entrypoints": [
                    {
                        "label": "Open workspace",
                        "command": "agentpm run"
                    }
                ]
            }
        }));
    }

    #[test]
    fn valid_minimal_loop_manifest_validates() {
        assert_manifest_ok(base_loop_manifest());
    }

    #[test]
    fn valid_full_loop_manifest_validates() {
        assert_manifest_ok(json!({
            "kind": "loop",
            "name": "incident-response-loop",
            "version": "1.0.0",
            "description": "A bounded triage, investigation, review, and response loop with approval and escalation paths.",
            "loop": {
                "archetype": "investigate_review_respond",
                "entry_phase": "triage",
                "limits": {
                    "max_steps": 16
                },
                "phases": [
                    {
                        "id": "triage",
                        "objective": "Assess the incident and determine whether investigation should proceed.",
                        "access": {
                            "tools": false,
                            "knowledge": true,
                            "memory": {
                                "read": true,
                                "write": false
                            }
                        },
                        "outcomes": [
                            {
                                "id": "proceed",
                                "description": "The incident has enough information to begin investigation."
                            },
                            {
                                "id": "cannot-proceed",
                                "description": "The incident cannot be investigated safely or meaningfully."
                            }
                        ]
                    },
                    {
                        "id": "investigate",
                        "objective": "Gather evidence, test hypotheses, and update the working understanding of the incident.",
                        "access": {
                            "tools": true,
                            "knowledge": true,
                            "memory": {
                                "read": true,
                                "write": true
                            }
                        }
                    },
                    {
                        "id": "review",
                        "objective": "Evaluate the evidence and decide whether more investigation, escalation, or response is appropriate.",
                        "access": {
                            "tools": false,
                            "knowledge": true,
                            "memory": {
                                "read": true,
                                "write": false
                            }
                        },
                        "outcomes": [
                            {
                                "id": "needs-more-evidence",
                                "description": "Important questions remain and another investigation cycle is required."
                            },
                            {
                                "id": "ready",
                                "description": "The evidence is sufficient to prepare the incident response."
                            },
                            {
                                "id": "escalate",
                                "description": "The incident requires an external actor or system to take over."
                            }
                        ]
                    },
                    {
                        "id": "respond",
                        "objective": "Produce and deliver the final incident response using the reviewed evidence.",
                        "access": {
                            "tools": true,
                            "knowledge": false,
                            "memory": {
                                "read": true,
                                "write": true
                            }
                        }
                    }
                ],
                "transitions": [
                    { "from": "triage", "on": "proceed", "to": "investigate" },
                    { "from": "triage", "on": "cannot-proceed", "to": "$abort" },
                    { "from": "investigate", "on": "complete", "to": "review" },
                    { "from": "review", "on": "needs-more-evidence", "to": "investigate" },
                    { "from": "review", "on": "ready", "to": "respond" },
                    { "from": "review", "on": "escalate", "to": "$handoff" },
                    { "from": "respond", "on": "complete", "to": "$end" }
                ],
                "checkpoints": [
                    {
                        "id": "approve-response",
                        "type": "approval",
                        "before_phase": "respond",
                        "on_reject": "review"
                    }
                ],
                "error_policy": {
                    "tool_failure": {
                        "action": "retry",
                        "max_retries": 2,
                        "on_exhausted": "fail_phase"
                    },
                    "phase_failure": {
                        "action": "abort"
                    }
                }
            },
            "readme": "README.md",
            "license": {
                "spdx": "Apache-2.0"
            }
        }));
    }

    #[test]
    fn agent_manifest_with_full_bindings_validates() {
        assert_manifest_ok(json!({
            "kind": "agent",
            "name": "incident-response-agent",
            "version": "1.0.0",
            "description": "Incident response agent.",
            "tools": ["@acme/get-incident-context@1.0.0", "@acme/search-logs@1.0.0"],
            "skills": ["@acme/incident-investigation@1.0.0"],
            "knowledge": ["@acme/incident-runbooks@1.0.0"],
            "memory": ["@acme/incident-memory@1.0.0"],
            "profiles": ["@acme/incident-responder@1.0.0"],
            "loop": "@acme/incident-response-loop@1.0.0",
            "bindings": {
                "global": {
                    "tools": ["@acme/get-incident-context"],
                    "skills": ["@acme/incident-investigation"],
                    "knowledge": ["@acme/incident-runbooks"],
                    "memory": [
                        {
                            "package": "@acme/incident-memory",
                            "spaces": ["incident_state"]
                        }
                    ],
                    "profiles": ["@acme/incident-responder"]
                },
                "phases": {
                    "review": {
                        "tools": ["@acme/search-logs"],
                        "memory": [
                            {
                                "package": "@acme/incident-memory",
                                "spaces": ["incident_state", "evidence"],
                                "operations": ["compact_evidence"]
                            }
                        ]
                    }
                },
                "mcp": [
                    {
                        "id": "investigation-tools",
                        "tools": ["@acme/get-incident-context", "@acme/search-logs"]
                    }
                ],
                "consumer_context": {
                    "file": "INCIDENT_AGENT.md"
                }
            },
            "examples": [
                {
                    "title": "Example prompt",
                    "prompt": "Investigate the incident."
                }
            ]
        }));
    }

    #[test]
    fn agent_bindings_reject_invalid_binding_shapes() {
        let cases = vec![
            (
                "versioned-tool-binding",
                json!({
                    "global": { "tools": ["@acme/get-incident-context@1.0.0"] }
                }),
                "/bindings/global/tools/0",
            ),
            (
                "object-skill-binding",
                json!({
                    "global": { "skills": [{ "name": "@acme/incident-investigation" }] }
                }),
                "/bindings/global/skills/0",
            ),
            (
                "unsafe-consumer-context",
                json!({
                    "consumer_context": { "file": "../INCIDENT_AGENT.md" }
                }),
                "/bindings/consumer_context/file",
            ),
            (
                "empty-memory-selectors",
                json!({
                    "global": {
                        "memory": [{ "package": "@acme/incident-memory" }]
                    }
                }),
                "/bindings/global/memory/0",
            ),
            (
                "invalid-memory-key",
                json!({
                    "global": {
                        "memory": [{
                            "package": "@acme/incident-memory",
                            "spaces": ["incident-state"]
                        }]
                    }
                }),
                "/bindings/global/memory/0/spaces/0",
            ),
            (
                "empty-mcp-tools",
                json!({
                    "mcp": [{ "id": "investigation-tools", "tools": [] }]
                }),
                "/bindings/mcp/0/tools",
            ),
            (
                "invalid-mcp-id",
                json!({
                    "mcp": [{ "id": "investigation_tools", "tools": ["@acme/get-incident-context"] }]
                }),
                "/bindings/mcp/0/id",
            ),
        ];

        for (label, bindings, expected_path) in cases {
            let manifest = json!({
                "kind": "agent",
                "name": format!("invalid-{label}"),
                "version": "1.0.0",
                "description": "Invalid agent bindings should fail.",
                "tools": ["@acme/get-incident-context@1.0.0"],
                "skills": ["@acme/incident-investigation@1.0.0"],
                "knowledge": ["@acme/incident-runbooks@1.0.0"],
                "memory": ["@acme/incident-memory@1.0.0"],
                "profiles": ["@acme/incident-responder@1.0.0"],
                "loop": "@acme/incident-response-loop@1.0.0",
                "bindings": bindings
            });

            let issues = assert_manifest_invalid(manifest);
            assert!(
                issues
                    .iter()
                    .any(|issue| issue.instance_path == expected_path),
                "expected invalid binding path {expected_path} for {label}, got: {issues:#?}"
            );
        }
    }

    #[test]
    fn loop_manifest_rejects_invalid_loop_shapes() {
        let mut invalid_id = base_loop_manifest();
        invalid_id["loop"]["phases"][0]["id"] = json!("triage_phase");
        let issues = assert_manifest_invalid(invalid_id);
        assert!(
            issues.iter().any(|issue| issue.instance_path == "/loop"),
            "expected invalid phase id failure, got: {issues:#?}"
        );

        let mut empty_outcomes = base_loop_manifest();
        empty_outcomes["loop"]["phases"][0]["outcomes"] = json!([]);
        let issues = assert_manifest_invalid(empty_outcomes);
        assert!(
            issues.iter().any(|issue| issue.instance_path == "/loop"),
            "expected empty outcomes failure, got: {issues:#?}"
        );

        let mut bad_terminal = base_loop_manifest();
        bad_terminal["loop"]["transitions"][0]["to"] = json!("$pause");
        let issues = assert_manifest_invalid(bad_terminal);
        assert!(
            issues.iter().any(|issue| issue.instance_path == "/loop"),
            "expected invalid terminal target failure, got: {issues:#?}"
        );

        let mut bad_access = base_loop_manifest();
        bad_access["loop"]["phases"][0]["access"] = json!({ "skills": true });
        let issues = assert_manifest_invalid(bad_access);
        assert!(
            issues.iter().any(|issue| issue.instance_path == "/loop"),
            "expected unsupported access field failure, got: {issues:#?}"
        );
    }

    #[test]
    fn bindings_are_rejected_on_non_agent_manifests() {
        let issues = assert_manifest_invalid(json!({
            "kind": "loop",
            "name": "bad-loop",
            "version": "1.0.0",
            "description": "Loop packages must not declare Agent bindings.",
            "loop": {
                "entry_phase": "triage",
                "phases": [{ "id": "triage", "objective": "Assess." }],
                "transitions": [{ "from": "triage", "on": "complete", "to": "$end" }]
            },
            "bindings": {
                "global": {
                    "tools": ["@acme/get-incident-context"]
                }
            }
        }));

        assert!(
            issues.iter().any(|issue| issue.instance_path == "/kind"),
            "expected bindings rejection on non-agent manifest, got: {issues:#?}"
        );
    }

    #[test]
    fn loop_manifest_rejects_package_dependencies() {
        let mut manifest = base_loop_manifest();
        manifest["tools"] = json!(["@acme/get-incident-context@1.0.0"]);

        let issues = assert_manifest_invalid(manifest);
        assert!(
            issues.iter().any(|issue| issue.instance_path == "/kind"),
            "expected dependency rejection for kind=loop, got: {issues:#?}"
        );
    }

    #[test]
    fn loop_semantics_reject_whitespace_only_text_fields() {
        let mut manifest = base_loop_manifest();
        manifest["loop"]["archetype"] = json!("   ");
        manifest["loop"]["phases"][0]["objective"] = json!("   ");
        manifest["loop"]["phases"][0]["outcomes"] = json!([
            {
                "id": "complete-review",
                "description": "   "
            }
        ]);
        manifest["loop"]["transitions"][0]["on"] = json!("complete-review");

        let issues = assert_manifest_invalid(manifest);
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/loop/archetype")
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/loop/phases/0/objective")
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/loop/phases/0/outcomes/0/description")
        );
    }

    #[test]
    fn loop_semantics_reject_invalid_entry_phase_and_transitions() {
        let mut manifest = base_loop_manifest();
        manifest["loop"]["entry_phase"] = json!("review");
        manifest["loop"]["transitions"] = json!([
            { "from": "triage", "on": "wrong", "to": "$end" },
            { "from": "missing", "on": "complete", "to": "$end" },
            { "from": "triage", "on": "complete", "to": "$pause" }
        ]);

        let issues = assert_manifest_invalid(manifest);
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/loop/entry_phase")
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/loop/transitions/0/on")
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/loop/transitions/1/from")
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/loop/transitions/2/to")
        );
    }

    #[test]
    fn loop_semantics_require_exactly_one_transition_and_reachable_terminal() {
        let manifest = json!({
            "kind": "loop",
            "name": "bad-loop",
            "version": "1.0.0",
            "description": "Loop with missing and duplicate transitions.",
            "loop": {
                "entry_phase": "triage",
                "phases": [
                    {
                        "id": "triage",
                        "objective": "Assess work.",
                        "outcomes": [
                            { "id": "proceed", "description": "Proceed." },
                            { "id": "stop", "description": "Stop." }
                        ]
                    },
                    {
                        "id": "review",
                        "objective": "Review work."
                    },
                    {
                        "id": "orphan",
                        "objective": "Never reached."
                    }
                ],
                "transitions": [
                    { "from": "triage", "on": "proceed", "to": "review" },
                    { "from": "triage", "on": "proceed", "to": "review" },
                    { "from": "review", "on": "complete", "to": "triage" }
                ]
            }
        });

        let issues = assert_manifest_invalid(manifest);
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/loop/transitions"
                    && issue.message.contains("triage")
                    && issue.message.contains("proceed"))
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/loop/transitions"
                    && issue.message.contains("stop"))
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/loop/phases/2/id")
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/loop/transitions"
                    && issue.message.contains("terminal"))
        );
    }

    #[test]
    fn loop_semantics_reject_invalid_checkpoints() {
        let mut manifest = base_loop_manifest();
        manifest["loop"]["phases"] = json!([
            { "id": "triage", "objective": "Assess." },
            { "id": "respond", "objective": "Respond." }
        ]);
        manifest["loop"]["transitions"] = json!([
            { "from": "triage", "on": "complete", "to": "respond" },
            { "from": "respond", "on": "complete", "to": "$end" }
        ]);
        manifest["loop"]["checkpoints"] = json!([
            {
                "id": "approve-response",
                "type": "approval",
                "before_phase": "respond",
                "on_reject": "triage"
            },
            {
                "id": "approve-response",
                "type": "approval",
                "before_phase": "respond",
                "on_reject": "$resume"
            }
        ]);

        let issues = assert_manifest_invalid(manifest);
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/loop/checkpoints/1/id")
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/loop/checkpoints/1/before_phase")
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/loop/checkpoints/1/on_reject")
        );
    }

    #[test]
    fn loop_semantics_reject_invalid_tool_failure_policy_shapes() {
        let mut missing_retry_fields = base_loop_manifest();
        missing_retry_fields["loop"]["error_policy"] = json!({
            "tool_failure": {
                "action": "retry"
            }
        });
        let issues = assert_manifest_invalid(missing_retry_fields);
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/loop/error_policy/tool_failure/max_retries")
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/loop/error_policy/tool_failure/on_exhausted")
        );

        let mut invalid_non_retry = base_loop_manifest();
        invalid_non_retry["loop"]["error_policy"] = json!({
            "tool_failure": {
                "action": "abort",
                "max_retries": 2,
                "on_exhausted": "handoff"
            }
        });
        let issues = assert_manifest_invalid(invalid_non_retry);
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/loop/error_policy/tool_failure/max_retries")
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/loop/error_policy/tool_failure/on_exhausted")
        );

        let mut missing_phase_failure = base_loop_manifest();
        missing_phase_failure["loop"]["error_policy"] = json!({
            "tool_failure": {
                "action": "retry",
                "max_retries": 1,
                "on_exhausted": "fail_phase"
            }
        });
        let issues = assert_manifest_invalid(missing_phase_failure);
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/loop/error_policy/phase_failure")
        );
    }

    #[test]
    fn agent_binding_semantics_reject_missing_top_level_bindings_and_duplicates() {
        let issues = assert_manifest_invalid(json!({
            "kind": "agent",
            "name": "bad-bindings-agent",
            "version": "1.0.0",
            "description": "Agent with bad bindings.",
            "tools": ["@acme/get-incident-context@1.0.0"],
            "skills": ["@acme/incident-investigation@1.0.0"],
            "knowledge": ["@acme/incident-runbooks@1.0.0"],
            "memory": ["@acme/incident-memory@1.0.0"],
            "profiles": ["@acme/incident-responder@1.0.0"],
            "bindings": {
                "global": {
                    "tools": ["@acme/missing-tool"],
                    "skills": ["@acme/missing-skill"],
                    "knowledge": ["@acme/missing-knowledge"],
                    "memory": [
                        {
                            "package": "@acme/missing-memory",
                            "spaces": ["incident_state"]
                        },
                        {
                            "package": "@acme/missing-memory",
                            "spaces": ["evidence"]
                        }
                    ],
                    "profiles": ["@acme/missing-profile"]
                },
                "phases": {
                    "review": {
                        "tools": ["@acme/get-incident-context"]
                    }
                },
                "mcp": [
                    {
                        "id": "ops-tools",
                        "tools": ["@acme/missing-tool"]
                    },
                    {
                        "id": "ops-tools",
                        "tools": ["@acme/get-incident-context"]
                    }
                ]
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/bindings/global/tools/0")
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/bindings/global/skills/0")
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/bindings/global/knowledge/0")
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/bindings/global/memory/0/package")
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/bindings/global/memory/1/package")
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/bindings/global/profiles/0")
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/bindings/phases")
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/bindings/mcp/0/tools/0")
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/bindings/mcp/1/id")
        );
    }

    #[test]
    fn template_manifest_accepts_zero_or_one_loop_and_rejects_plural_loop_shapes() {
        let no_loop = json!({
            "kind": "template",
            "name": "incident-template",
            "version": "0.1.0",
            "description": "Incident template.",
            "template": {
                "display_name": "Incident Template",
                "use_case": "incident-response",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": []
                },
                "entrypoints": [{ "label": "Run", "command": "python main.py" }]
            }
        });
        assert_manifest_ok(no_loop);

        let one_loop = json!({
            "kind": "template",
            "name": "incident-template",
            "version": "0.1.0",
            "description": "Incident template.",
            "template": {
                "display_name": "Incident Template",
                "use_case": "incident-response",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": [],
                    "loop": "@acme/incident-response-loop@1.0.0"
                },
                "entrypoints": [{ "label": "Run", "command": "python main.py" }]
            }
        });
        assert_manifest_ok(one_loop);

        let issues = assert_manifest_invalid(json!({
            "kind": "template",
            "name": "incident-template",
            "version": "0.1.0",
            "description": "Incident template.",
            "template": {
                "display_name": "Incident Template",
                "use_case": "incident-response",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": [],
                    "loop": [
                        "@acme/incident-response-loop@1.0.0",
                        "@acme/fallback-loop@1.0.0"
                    ]
                },
                "entrypoints": [{ "label": "Run", "command": "python main.py" }]
            }
        }));
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/template/dependencies/loop"),
            "expected singular template loop shape failure, got: {issues:#?}"
        );
    }

    #[test]
    fn valid_minimal_memory_manifest_validates() {
        assert_manifest_ok(json!({
            "kind": "memory",
            "name": "conversation-continuity",
            "version": "0.1.0",
            "description": "Portable structure for conversational continuity memory.",
            "memory": {
                "scopes": {
                    "user": {
                        "description": "The user whose memory is being retained."
                    }
                },
                "record_types": {
                    "user_preference": {
                        "version": "1.0.0",
                        "description": "Durable structured preferences for one user.",
                        "schema": "schemas/user-preference.schema.json"
                    }
                },
                "spaces": {
                    "profile": {
                        "description": "The current durable profile for one user.",
                        "model": "document",
                        "record_types": ["user_preference"],
                        "scope": ["user"],
                        "retrieval": {
                            "modes": ["key"]
                        }
                    }
                }
            }
        }));
    }

    #[test]
    fn valid_advanced_memory_manifest_validates() {
        assert_manifest_ok(json!({
            "kind": "memory",
            "name": "support-memory",
            "version": "0.1.0",
            "description": "Structured durable memory blueprint for support workflows.",
            "memory": {
                "scopes": {
                    "user": { "description": "The current user." },
                    "conversation": { "description": "The active conversation thread." }
                },
                "record_types": {
                    "interaction": {
                        "version": "1.0.0",
                        "description": "One interaction in a conversation.",
                        "schema": "schemas/user-preference.schema.json"
                    },
                    "conversation_summary": {
                        "version": "1.2.0",
                        "description": "Derived durable summary of a conversation.",
                        "schema": "schemas/conversation-summary.schema.json"
                    }
                },
                "spaces": {
                    "recent_interactions": {
                        "description": "Short-term ordered interaction history.",
                        "model": "sequence",
                        "record_types": ["interaction"],
                        "scope": ["user", "conversation"],
                        "retrieval": {
                            "modes": ["chronological", "semantic"]
                        },
                        "capacity": {
                            "max_records": 20
                        },
                        "retention": {
                            "ttl": "P7D",
                            "on_expire": "delete"
                        },
                        "constraints": {
                            "append_only": true
                        }
                    },
                    "conversation_history": {
                        "description": "Durable summary document for a conversation.",
                        "model": "document",
                        "record_types": ["conversation_summary"],
                        "scope": ["user", "conversation"],
                        "retrieval": {
                            "modes": ["key", "semantic"]
                        },
                        "retention": {
                            "ttl": "P30D",
                            "on_expire": "archive"
                        }
                    }
                },
                "operations": {
                    "consolidate_recent_interactions": {
                        "type": "consolidate",
                        "description": "Convert recent interactions into a durable summary.",
                        "trigger": {
                            "type": "record_count",
                            "space": "recent_interactions",
                            "threshold": 20
                        },
                        "inputs": [
                            {
                                "space": "recent_interactions",
                                "record_type": "interaction"
                            }
                        ],
                        "output": {
                            "space": "conversation_history",
                            "record_type": "conversation_summary"
                        },
                        "source_handling": "delete_after_success",
                        "preserve_provenance": true
                    },
                    "transform_interaction_summary": {
                        "type": "transform",
                        "description": "Convert one interaction record into a normalized summary record.",
                        "trigger": {
                            "type": "interval",
                            "every": "P1D"
                        },
                        "inputs": [
                            {
                                "space": "recent_interactions",
                                "record_type": "interaction"
                            }
                        ],
                        "output": {
                            "space": "conversation_history",
                            "record_type": "conversation_summary"
                        },
                        "source_handling": "retain",
                        "preserve_provenance": true
                    },
                    "delete_user_memory": {
                        "type": "delete",
                        "description": "Delete durable memory for a user conversation.",
                        "trigger": {
                            "type": "external"
                        },
                        "targets": [
                            { "space": "conversation_history" },
                            { "space": "recent_interactions" }
                        ],
                        "cascade_derived_records": true
                    }
                }
            }
        }));
    }

    #[test]
    fn valid_memory_manifest_with_source_schema_and_governance_annotations_validates() {
        let dir = temp_dir("memory-valid-schema");
        assert_manifest_file_ok(
            &dir,
            base_memory_manifest(),
            &[("schemas/user-preference.schema.json", valid_memory_schema())],
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_semantics_reject_unknown_scope_reference() {
        let dir = temp_dir("memory-unknown-scope");
        let mut manifest = base_memory_manifest();
        manifest["memory"]["spaces"]["profile"]["scope"] = json!(["account"]);

        let issues = assert_manifest_file_invalid(
            &dir,
            manifest,
            &[("schemas/user-preference.schema.json", valid_memory_schema())],
        );

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/memory/spaces/profile/scope/0"),
            "expected unknown scope failure, got: {issues:#?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_semantics_reject_unknown_record_type_reference() {
        let dir = temp_dir("memory-unknown-record-type");
        let mut manifest = base_memory_manifest();
        manifest["memory"]["spaces"]["profile"]["record_types"] = json!(["missing_type"]);

        let issues = assert_manifest_file_invalid(
            &dir,
            manifest,
            &[("schemas/user-preference.schema.json", valid_memory_schema())],
        );

        assert!(
            issues
                .iter()
                .any(|issue| { issue.instance_path == "/memory/spaces/profile/record_types/0" }),
            "expected unknown record type failure, got: {issues:#?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_semantics_reject_document_without_key_retrieval() {
        let dir = temp_dir("memory-document-without-key");
        let mut manifest = base_memory_manifest();
        manifest["memory"]["spaces"]["profile"]["retrieval"]["modes"] = json!(["filter"]);

        let issues = assert_manifest_file_invalid(
            &dir,
            manifest,
            &[("schemas/user-preference.schema.json", valid_memory_schema())],
        );

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/memory/spaces/profile/retrieval/modes"),
            "expected document retrieval semantic failure, got: {issues:#?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_semantics_reject_sequence_without_chronological_retrieval() {
        let dir = temp_dir("memory-sequence-without-chronological");
        let mut manifest = base_memory_manifest();
        manifest["memory"]["spaces"]["profile"]["model"] = json!("sequence");
        manifest["memory"]["spaces"]["profile"]["retrieval"]["modes"] = json!(["key"]);

        let issues = assert_manifest_file_invalid(
            &dir,
            manifest,
            &[("schemas/user-preference.schema.json", valid_memory_schema())],
        );

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/memory/spaces/profile/retrieval/modes"),
            "expected sequence retrieval semantic failure, got: {issues:#?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_semantics_reject_document_append_only() {
        let dir = temp_dir("memory-document-append-only");
        let mut manifest = base_memory_manifest();
        manifest["memory"]["spaces"]["profile"]["constraints"] = json!({ "append_only": true });

        let issues = assert_manifest_file_invalid(
            &dir,
            manifest,
            &[("schemas/user-preference.schema.json", valid_memory_schema())],
        );

        assert!(
            issues.iter().any(|issue| {
                issue.instance_path == "/memory/spaces/profile/constraints/append_only"
            }),
            "expected document append_only semantic failure, got: {issues:#?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_semantics_accept_collection_append_only() {
        let dir = temp_dir("memory-collection-append-only");
        let mut manifest = base_memory_manifest();
        manifest["memory"]["spaces"]["profile"]["model"] = json!("collection");
        manifest["memory"]["spaces"]["profile"]["constraints"] = json!({ "append_only": true });

        assert_manifest_file_ok(
            &dir,
            manifest,
            &[("schemas/user-preference.schema.json", valid_memory_schema())],
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_semantics_accept_sequence_append_only() {
        let dir = temp_dir("memory-sequence-append-only");
        let mut manifest = base_memory_manifest();
        manifest["memory"]["spaces"]["profile"]["model"] = json!("sequence");
        manifest["memory"]["spaces"]["profile"]["retrieval"]["modes"] = json!(["chronological"]);
        manifest["memory"]["spaces"]["profile"]["constraints"] = json!({ "append_only": true });

        assert_manifest_file_ok(
            &dir,
            manifest,
            &[("schemas/user-preference.schema.json", valid_memory_schema())],
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_semantics_reject_missing_source_schema_file() {
        let dir = temp_dir("memory-missing-schema-file");
        let issues = assert_manifest_file_invalid(&dir, base_memory_manifest(), &[]);

        assert!(
            issues.iter().any(|issue| {
                issue.instance_path == "/memory/record_types/user_preference/schema"
            }),
            "expected missing schema file failure, got: {issues:#?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_semantics_reject_invalid_source_schema_json() {
        let dir = temp_dir("memory-invalid-schema-json");
        let issues = assert_manifest_file_invalid(
            &dir,
            base_memory_manifest(),
            &[("schemas/user-preference.schema.json", "{ not-json }")],
        );

        assert!(
            issues
                .iter()
                .any(|issue| issue.message.contains("source schema is not valid JSON")),
            "expected invalid JSON schema failure, got: {issues:#?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_semantics_reject_invalid_source_schema_draft() {
        let dir = temp_dir("memory-invalid-schema-draft");
        let issues = assert_manifest_file_invalid(
            &dir,
            base_memory_manifest(),
            &[(
                "schemas/user-preference.schema.json",
                r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": 7
}
"#,
            )],
        );

        assert!(
            issues.iter().any(|issue| issue
                .message
                .contains("source schema is not valid JSON Schema Draft 2020-12")),
            "expected invalid Draft 2020-12 schema failure, got: {issues:#?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_semantics_reject_unknown_governance_keyword() {
        let dir = temp_dir("memory-unknown-governance-keyword");
        let issues = assert_manifest_file_invalid(
            &dir,
            base_memory_manifest(),
            &[(
                "schemas/user-preference.schema.json",
                r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "favorite_color": {
      "type": "string",
      "x-agentpm-unknown": "nope"
    }
  }
}
"#,
            )],
        );

        assert!(
            issues.iter().any(
                |issue| issue.file.ends_with("schemas/user-preference.schema.json")
                    && issue.instance_path == "/properties/favorite_color/x-agentpm-unknown"
            ),
            "expected unknown governance keyword failure, got: {issues:#?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_semantics_reject_unknown_governance_keyword_for_default_agent_json_path() {
        let dir = temp_dir("memory-unknown-governance-default-path");
        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "favorite_color": {
      "type": "string",
      "x-agentpm-unknown": "nope"
    }
  }
}
"#,
        );

        let manifest_path = dir.join("agent.json");
        write_manifest_pretty(&manifest_path, &base_memory_manifest()).unwrap();

        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let result = {
            let (mut loaded, _) = load_manifest_value(Path::new("agent.json")).unwrap();
            validate_manifest_value(&schema_path(), "agent.json", &mut loaded, false).unwrap()
        };

        std::env::set_current_dir(cwd).unwrap();

        let (ok, issues) = result;
        assert!(!ok, "expected manifest to fail validation");
        assert!(
            issues.iter().any(
                |issue| issue.file.ends_with("schemas/user-preference.schema.json")
                    && issue.instance_path == "/properties/favorite_color/x-agentpm-unknown"
            ),
            "expected unknown governance keyword failure, got: {issues:#?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_semantics_reject_invalid_governance_value_types() {
        let dir = temp_dir("memory-invalid-governance-values");
        let issues = assert_manifest_file_invalid(
            &dir,
            base_memory_manifest(),
            &[(
                "schemas/user-preference.schema.json",
                r##"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "$defs": {
    "detail": {
      "type": "object",
      "properties": {
        "note": {
          "type": "string",
          "x-agentpm-data-class": "secret",
          "x-agentpm-shareable": "sometimes"
        }
      }
    }
  },
  "properties": {
    "detail": {
      "allOf": [
        { "$ref": "#/$defs/detail" }
      ],
      "x-agentpm-persist": "yes"
    }
  }
}
"##,
            )],
        );

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path
                    == "/$defs/detail/properties/note/x-agentpm-data-class")
                && issues
                    .iter()
                    .any(|issue| issue.instance_path == "/properties/detail/x-agentpm-persist")
                && issues.iter().any(|issue| issue.instance_path
                    == "/$defs/detail/properties/note/x-agentpm-shareable"),
            "expected invalid governance values failure, got: {issues:#?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_semantics_reject_external_schema_refs() {
        let dir = temp_dir("memory-external-schema-ref");
        let issues = assert_manifest_file_invalid(
            &dir,
            base_memory_manifest(),
            &[(
                "schemas/user-preference.schema.json",
                r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$ref": "https://example.com/schema.json"
}
"#,
            )],
        );

        assert!(
            issues.iter().any(|issue| issue.instance_path == "/$ref"),
            "expected external $ref rejection, got: {issues:#?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_semantics_reject_ambiguous_retention_duration() {
        let dir = temp_dir("memory-ambiguous-retention-duration");
        let mut manifest = base_memory_manifest();
        manifest["memory"]["spaces"]["profile"]["retention"] = json!({
            "ttl": "P1M",
            "on_expire": "delete"
        });

        let issues = assert_manifest_file_invalid(
            &dir,
            manifest,
            &[("schemas/user-preference.schema.json", valid_memory_schema())],
        );

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/memory/spaces/profile/retention/ttl"),
            "expected retention duration failure, got: {issues:#?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_semantics_reject_interval_trigger_invalid_duration() {
        let dir = temp_dir("memory-invalid-interval-trigger");
        let mut manifest = base_memory_manifest();
        manifest["memory"]["operations"] = json!({
            "transform_profile": {
                "type": "transform",
                "description": "Transform profile memory.",
                "trigger": {
                    "type": "interval",
                    "every": "P1M"
                },
                "inputs": [
                    {
                        "space": "profile",
                        "record_type": "user_preference"
                    }
                ],
                "output": {
                    "space": "profile",
                    "record_type": "user_preference"
                },
                "source_handling": "retain",
                "preserve_provenance": true
            }
        });

        let issues = assert_manifest_file_invalid(
            &dir,
            manifest,
            &[("schemas/user-preference.schema.json", valid_memory_schema())],
        );

        assert!(
            issues.iter().any(|issue| {
                issue.instance_path == "/memory/operations/transform_profile/trigger/every"
            }),
            "expected interval duration failure, got: {issues:#?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_semantics_reject_record_count_trigger_zero_threshold() {
        let dir = temp_dir("memory-zero-record-count-threshold");
        let mut manifest = base_memory_manifest();
        manifest["memory"]["spaces"]["history"] = json!({
            "description": "History records for one user.",
            "model": "sequence",
            "record_types": ["user_preference"],
            "scope": ["user"],
            "retrieval": {
                "modes": ["chronological"]
            }
        });
        manifest["memory"]["operations"] = json!({
            "consolidate_history": {
                "type": "consolidate",
                "description": "Consolidate history records.",
                "trigger": {
                    "type": "record_count",
                    "space": "history",
                    "threshold": 0
                },
                "inputs": [
                    {
                        "space": "history",
                        "record_type": "user_preference"
                    }
                ],
                "output": {
                    "space": "profile",
                    "record_type": "user_preference"
                },
                "source_handling": "retain",
                "preserve_provenance": true
            }
        });

        let issues = assert_manifest_file_invalid(
            &dir,
            manifest,
            &[("schemas/user-preference.schema.json", valid_memory_schema())],
        );

        assert!(
            issues.iter().any(|issue| {
                issue.instance_path == "/memory/operations/consolidate_history/trigger/threshold"
            }),
            "expected record_count threshold failure, got: {issues:#?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_semantics_reject_duplicate_scope_in_space() {
        let dir = temp_dir("memory-duplicate-scope");
        let mut manifest = base_memory_manifest();
        manifest["memory"]["spaces"]["profile"]["scope"] = json!(["user", "user"]);

        let issues = assert_manifest_file_invalid(
            &dir,
            manifest,
            &[("schemas/user-preference.schema.json", valid_memory_schema())],
        );

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/memory/spaces/profile/scope"),
            "expected duplicate scope failure, got: {issues:#?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_semantics_reject_duplicate_record_type_in_space() {
        let dir = temp_dir("memory-duplicate-record-type");
        let mut manifest = base_memory_manifest();
        manifest["memory"]["spaces"]["profile"]["record_types"] =
            json!(["user_preference", "user_preference"]);

        let issues = assert_manifest_file_invalid(
            &dir,
            manifest,
            &[("schemas/user-preference.schema.json", valid_memory_schema())],
        );

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/memory/spaces/profile/record_types"),
            "expected duplicate record_type failure, got: {issues:#?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_semantics_reject_capacity_trigger_without_capacity() {
        let dir = temp_dir("memory-capacity-trigger-without-capacity");
        let mut manifest = base_memory_manifest();
        manifest["memory"]["operations"] = json!({
            "compact_profile": {
                "type": "delete",
                "description": "Delete profile records.",
                "trigger": {
                    "type": "capacity",
                    "space": "profile"
                },
                "targets": [
                    { "space": "profile" }
                ],
                "cascade_derived_records": false
            }
        });

        let issues = assert_manifest_file_invalid(
            &dir,
            manifest,
            &[("schemas/user-preference.schema.json", valid_memory_schema())],
        );

        assert!(
            issues.iter().any(|issue| {
                issue.instance_path == "/memory/operations/compact_profile/trigger/space"
            }),
            "expected capacity trigger semantic failure, got: {issues:#?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_semantics_reject_operation_record_type_not_permitted_by_space() {
        let dir = temp_dir("memory-operation-record-type-mismatch");
        let mut manifest = base_memory_manifest();
        manifest["memory"]["record_types"]["summary"] = json!({
            "version": "1.0.0",
            "description": "Summary record.",
            "schema": "schemas/summary.schema.json"
        });
        manifest["memory"]["operations"] = json!({
            "transform_profile": {
                "type": "transform",
                "description": "Transform profile memory.",
                "trigger": { "type": "external" },
                "inputs": [
                    {
                        "space": "profile",
                        "record_type": "summary"
                    }
                ],
                "output": {
                    "space": "profile",
                    "record_type": "user_preference"
                },
                "source_handling": "retain",
                "preserve_provenance": true
            }
        });

        let issues = assert_manifest_file_invalid(
            &dir,
            manifest,
            &[
                ("schemas/user-preference.schema.json", valid_memory_schema()),
                ("schemas/summary.schema.json", valid_memory_schema()),
            ],
        );

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/memory/operations/transform_profile/inputs"),
            "expected operation record-type compatibility failure, got: {issues:#?}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn valid_minimal_context_mode_knowledge_manifest_validates() {
        assert_manifest_ok(json!({
            "kind": "knowledge",
            "name": "engineering-playbook",
            "version": "0.1.0",
            "description": "Engineering playbook intended for direct context loading.",
            "knowledge": {
                "mode": "context"
            }
        }));
    }

    #[test]
    fn valid_built_context_mode_knowledge_manifest_validates() {
        assert_manifest_ok(json!({
            "kind": "knowledge",
            "name": "engineering-playbook",
            "version": "0.1.0",
            "description": "Engineering playbook intended for direct context loading.",
            "knowledge": {
                "mode": "context",
                "documents": [
                    {
                        "path": "knowledge/docs/playbook.md",
                        "content_type": "text/markdown",
                        "role": "context",
                        "bytes": 18432,
                        "sha256": "sha256:abc123"
                    }
                ],
                "context": {
                    "document_count": 1,
                    "total_bytes": 18432,
                    "content_hash": "sha256:def456"
                },
                "provenance": {
                    "generated_at": "2026-07-02T00:00:00Z",
                    "builder": {
                        "name": "custom",
                        "version": "unknown"
                    }
                }
            }
        }));
    }

    #[test]
    fn valid_minimal_vector_mode_knowledge_manifest_validates() {
        assert_manifest_ok(json!({
            "kind": "knowledge",
            "name": "python-docs",
            "version": "0.1.0",
            "description": "Prepared retrieval corpus for Python documentation.",
            "knowledge": {
                "mode": "vector",
                "corpus": {
                    "chunks_path": "knowledge/chunks.jsonl",
                    "sources_path": "knowledge/sources.jsonl"
                },
                "embedding": {
                    "id": "default",
                    "provider": "openai",
                    "model": "text-embedding-3-small",
                    "dimensions": 1536,
                    "metric": "cosine",
                    "normalized": true,
                    "vectors_path": "knowledge/embeddings/default.f32"
                }
            }
        }));
    }

    #[test]
    fn valid_built_vector_mode_knowledge_manifest_validates() {
        assert_manifest_ok(json!({
            "kind": "knowledge",
            "name": "python-docs",
            "version": "0.1.0",
            "description": "Prepared retrieval corpus for Python documentation.",
            "knowledge": {
                "mode": "vector",
                "corpus": {
                    "chunks_path": "knowledge/chunks.jsonl",
                    "sources_path": "knowledge/sources.jsonl",
                    "chunk_count": 12482,
                    "source_count": 327,
                    "content_hash": "sha256:content"
                },
                "chunking": {
                    "strategy": "recursive-text-splitter",
                    "chunk_size": 512,
                    "overlap": 64
                },
                "embedding": {
                    "id": "default",
                    "provider": "openai",
                    "model": "text-embedding-3-small",
                    "dimensions": 1536,
                    "metric": "cosine",
                    "normalized": true,
                    "vectors_path": "knowledge/embeddings/default.f32",
                    "vector_count": 12482,
                    "vectors_hash": "sha256:vectors"
                },
                "indexes": [
                    {
                        "id": "default",
                        "type": "agentpm-local",
                        "path": "knowledge/indexes/default",
                        "embedding_id": "default",
                        "generated_by": "agentpm knowledge build"
                    }
                ],
                "retrieval": {
                    "strategy": "vector",
                    "default_top_k": 8,
                    "return_citations": true
                },
                "provenance": {
                    "sources_manifest_path": "knowledge/provenance/sources.jsonl",
                    "generated_at": "2026-07-02T00:00:00Z",
                    "builder": {
                        "name": "custom",
                        "version": "unknown"
                    }
                }
            }
        }));
    }

    #[test]
    fn invalid_tool_manifest_missing_single_required_tool_field_fails() {
        let issues = assert_manifest_invalid(json!({
            "kind": "tool",
            "name": "broken-tool",
            "version": "0.1.0",
            "description": "Missing one required tool-only field.",
            "entrypoint": {
                "command": "python",
                "args": ["capitalize.py"]
            },
            "inputs": {
                "type": "object"
            },
            "outputs": {
                "type": "object"
            }
        }));

        assert!(
            issues.iter().any(|issue| issue.schema_path == "/oneOf"),
            "expected oneOf failure caused by the remaining missing tool field, got: {issues:#?}"
        );
    }

    #[test]
    fn invalid_agent_manifest_with_tool_only_fields_fails() {
        let issues = assert_manifest_invalid(json!({
            "kind": "agent",
            "name": "misclassified-agent",
            "version": "0.1.0",
            "description": "Should not carry tool runtime contract.",
            "tools": ["@zack/slack-post-message@0.1.0"],
            "entrypoint": {
                "command": "node",
                "args": ["dist/index.js"]
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/kind" || issue.instance_path.is_empty()),
            "expected kind/oneOf validation error, got: {issues:#?}"
        );
    }

    #[test]
    fn agent_manifest_with_recursive_agents_dependencies_fails() {
        let issues = assert_manifest_invalid(json!({
            "kind": "agent",
            "name": "recursive-agent",
            "version": "0.1.0",
            "description": "Should not accept recursive agent dependencies.",
            "tools": ["@zack/web-page-extract@0.1.0"],
            "agents": ["@zack/another-agent@0.1.0"]
        }));

        assert!(
            issues.iter().any(|issue| issue
                .message
                .contains("Additional properties are not allowed")
                || issue.instance_path.is_empty()),
            "expected recursive agents dependency failure, got: {issues:#?}"
        );
    }

    #[test]
    fn reserved_agent_fields_validate_and_are_preserved_without_warning_on_skills_or_knowledge() {
        let mut manifest = json!({
            "kind": "agent",
            "name": "preserved-agent",
            "version": "0.1.0",
            "description": "Reserved references should survive lint validation untouched.",
            "tools": ["@zack/slack-post-message@0.1.0"],
            "skills": ["@zack/support-triage-skill@0.1.0"],
            "knowledge": [{"name": "@zack/internal-playbooks", "version": "0.1.0"}],
            "memory": ["@zack/session-memory@0.1.0"],
            "profiles": ["@zack/escalation-profile@0.1.0"]
        });
        let original = manifest.clone();

        let (ok, issues) =
            validate_manifest_value(&schema_path(), "agent.json", &mut manifest, false).unwrap();

        assert!(ok, "expected manifest to validate, got issues: {issues:#?}");
        assert_eq!(
            manifest, original,
            "validation should preserve reserved fields"
        );
        assert!(
            issues.iter().all(|issue| issue.instance_path != "/skills"),
            "skills should not emit a reserved-field warning, got: {issues:#?}"
        );
        assert!(
            issues
                .iter()
                .all(|issue| issue.instance_path != "/knowledge"),
            "knowledge should not emit a reserved-field warning, got: {issues:#?}"
        );
        assert!(
            issues.iter().all(|issue| issue.instance_path != "/memory"),
            "memory should not emit a reserved-field warning, got: {issues:#?}"
        );
    }

    #[test]
    fn reserved_future_fields_do_not_replace_required_tools() {
        let issues = assert_manifest_invalid(json!({
            "kind": "agent",
            "name": "missing-tools-agent",
            "version": "0.1.0",
            "description": "Reserved refs alone must not imply installable tool dependencies.",
            "skills": ["@zack/support-triage-skill@0.1.0"],
            "knowledge": ["@zack/internal-playbooks@0.1.0"],
            "memory": [],
            "profiles": []
        }));

        assert!(
            issues.iter().any(|issue| issue.instance_path.is_empty()),
            "expected missing-tools validation error, got: {issues:#?}"
        );
    }

    #[test]
    fn template_manifest_rejects_invalid_variable_name() {
        let issues = assert_manifest_invalid(json!({
            "kind": "template",
            "name": "bad-variable-template",
            "version": "0.1.0",
            "description": "Invalid variable name should fail schema validation.",
            "template": {
                "display_name": "Bad Variable Template",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [
                    {
                        "name": "ProjectName",
                        "description": "Bad variable name.",
                        "required": true
                    }
                ],
                "dependencies": {
                    "tools": [],
                    "agents": []
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ]
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/template/variables/0/name"),
            "expected invalid template variable name error, got: {issues:#?}"
        );
    }

    #[test]
    fn template_manifest_rejects_invalid_stack_value() {
        let issues = assert_manifest_invalid(json!({
            "kind": "template",
            "name": "bad-stack-template",
            "version": "0.1.0",
            "description": "Invalid stack values should fail schema validation.",
            "template": {
                "display_name": "Bad Stack Template",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "stack": ["cobol"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": []
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ]
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/template/stack/0"),
            "expected invalid stack enum error, got: {issues:#?}"
        );
    }

    #[test]
    fn template_manifest_rejects_duplicate_variable_names() {
        let issues = assert_manifest_invalid(json!({
            "kind": "template",
            "name": "duplicate-variables-template",
            "version": "0.1.0",
            "description": "Duplicate variable names should fail validation.",
            "template": {
                "display_name": "Duplicate Variables Template",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [
                    {
                        "name": "project_name",
                        "description": "Project name.",
                        "required": true
                    },
                    {
                        "name": "project_name",
                        "description": "Duplicate project name.",
                        "required": false,
                        "default": "duplicate"
                    }
                ],
                "dependencies": {
                    "tools": [],
                    "agents": []
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ]
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.message.contains("duplicate template variable name")),
            "expected duplicate template variable name error, got: {issues:#?}"
        );
    }

    #[test]
    fn template_manifest_requires_template_object() {
        let issues = assert_manifest_invalid(json!({
            "kind": "template",
            "name": "missing-template-object",
            "version": "0.1.0",
            "description": "Missing template metadata should fail."
        }));

        assert!(
            issues.iter().any(|issue| issue.schema_path == "/oneOf"
                || issue.message.contains("required property")),
            "expected missing template object failure, got: {issues:#?}"
        );
    }

    #[test]
    fn template_manifest_rejects_missing_files_root() {
        let issues = assert_manifest_invalid(json!({
            "kind": "template",
            "name": "missing-files-root",
            "version": "0.1.0",
            "description": "Missing files_root should fail.",
            "template": {
                "display_name": "Missing Files Root",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": []
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ]
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.message.contains("files_root")),
            "expected missing files_root validation error, got: {issues:#?}"
        );
    }

    #[test]
    fn template_manifest_rejects_invalid_dependency_shape() {
        let issues = assert_manifest_invalid(json!({
            "kind": "template",
            "name": "invalid-dependency-shape",
            "version": "0.1.0",
            "description": "Template dependency refs must match packageRef shape.",
            "template": {
                "display_name": "Invalid Dependency Shape",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [
                        {
                            "version": "0.1.0"
                        }
                    ],
                    "agents": []
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ]
            }
        }));

        assert!(
            issues.iter().any(|issue| issue
                .instance_path
                .starts_with("/template/dependencies/tools/0")),
            "expected invalid dependency shape failure, got: {issues:#?}"
        );
    }

    #[test]
    fn skill_manifest_rejects_top_level_skill_dependencies() {
        let issues = assert_manifest_invalid(json!({
            "kind": "skill",
            "name": "recursive-skill",
            "version": "0.1.0",
            "description": "Should not accept skill-to-skill dependencies in Phase 6A.",
            "tools": [],
            "skills": ["@zack/other-skill@0.1.0"],
            "skill": {
                "entrypoint": "SKILL.md"
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/kind" || issue.instance_path.is_empty()),
            "expected top-level skills rejection for kind=skill, got: {issues:#?}"
        );
    }

    #[test]
    fn knowledge_manifest_rejects_dependency_arrays() {
        let issues = assert_manifest_invalid(json!({
            "kind": "knowledge",
            "name": "bad-knowledge",
            "version": "0.1.0",
            "description": "Knowledge packages must not declare dependencies.",
            "tools": ["@zack/slack-post-message@0.1.0"],
            "knowledge": {
                "mode": "context"
            }
        }));

        assert!(
            issues.iter().any(|issue| issue.instance_path == "/kind"),
            "expected dependency rejection for kind=knowledge, got: {issues:#?}"
        );
    }

    #[test]
    fn memory_manifest_rejects_dependency_arrays() {
        let issues = assert_manifest_invalid(json!({
            "kind": "memory",
            "name": "bad-memory",
            "version": "0.1.0",
            "description": "Memory packages must not declare dependencies.",
            "tools": ["@zack/slack-post-message@0.1.0"],
            "memory": {
                "scopes": {
                    "user": { "description": "User scope." }
                },
                "record_types": {
                    "preference": {
                        "version": "1.0.0",
                        "description": "Preference record.",
                        "schema": "schemas/preference.schema.json"
                    }
                },
                "spaces": {
                    "profile": {
                        "description": "Profile document.",
                        "model": "document",
                        "record_types": ["preference"],
                        "scope": ["user"],
                        "retrieval": { "modes": ["key"] }
                    }
                }
            }
        }));

        assert!(
            issues.iter().any(|issue| issue.instance_path == "/kind"),
            "expected dependency rejection for kind=memory, got: {issues:#?}"
        );
    }

    #[test]
    fn profile_manifest_rejects_dependency_arrays_and_foreign_package_fields() {
        let base = json!({
            "kind": "profile",
            "name": "bad-profile",
            "version": "1.0.0",
            "description": "Profiles must not declare package dependencies or foreign package metadata.",
            "profile": {
                "identity": { "role": "Support agent" },
                "objectives": ["Help."],
                "communication": { "tone": ["warm"], "verbosity": "concise" }
            }
        });
        let cases = vec![
            ("tools", json!(["@zack/slack-post-message@0.1.0"])),
            ("agents", json!(["@zack/ops-console@0.1.0"])),
            ("skills", json!(["@zack/triage-skill@0.1.0"])),
            ("knowledge", json!(["@zack/python-docs@0.1.0"])),
            ("memory", json!(["@zack/conversation-continuity@0.1.0"])),
            ("profiles", json!(["@zack/other-profile@1.0.0"])),
            ("skill", json!({ "entrypoint": "SKILL.md" })),
            (
                "template",
                json!({
                    "display_name": "Bad",
                    "use_case": "support",
                    "execution_surfaces": ["python-sdk"],
                    "files_root": "template",
                    "variables": [],
                    "dependencies": { "tools": [], "agents": [] },
                    "entrypoints": [{ "label": "Run", "command": "python main.py" }]
                }),
            ),
        ];

        for (field_name, field_value) in cases {
            let mut manifest = base.clone();
            manifest[field_name] = field_value;
            let issues = assert_manifest_invalid(manifest);
            assert!(
                !issues.is_empty(),
                "expected profile manifest with forbidden field `{field_name}` to fail"
            );
        }
    }

    #[test]
    fn profile_semantics_reject_whitespace_only_required_text() {
        let cases = vec![
            (
                {
                    let mut manifest = base_profile_manifest();
                    manifest["profile"]["identity"]["role"] = json!("   ");
                    manifest
                },
                "/profile/identity/role",
            ),
            (
                {
                    let mut manifest = base_profile_manifest();
                    manifest["profile"]["objectives"][0] = json!("  ");
                    manifest
                },
                "/profile/objectives/0",
            ),
            (
                {
                    let mut manifest = base_profile_manifest();
                    manifest["profile"]["communication"]["tone"][0] = json!("\n\t");
                    manifest
                },
                "/profile/communication/tone/0",
            ),
            (
                {
                    let mut manifest = base_profile_manifest();
                    manifest["profile"]["constraints"] = json!([
                        {
                            "id": "protect-authentication-data",
                            "strength": "required",
                            "instruction": "   "
                        }
                    ]);
                    manifest
                },
                "/profile/constraints/0/instruction",
            ),
        ];

        for (manifest, expected_path) in cases {
            let issues = assert_manifest_invalid(manifest);
            assert!(
                issues
                    .iter()
                    .any(|issue| issue.instance_path == expected_path),
                "expected whitespace-only failure at {expected_path}, got: {issues:#?}"
            );
        }
    }

    #[test]
    fn profile_semantics_reject_whitespace_only_optional_text() {
        let cases = vec![
            (
                {
                    let mut manifest = base_profile_manifest();
                    manifest["profile"]["identity"]["description"] = json!("  ");
                    manifest
                },
                "/profile/identity/description",
            ),
            (
                {
                    let mut manifest = base_profile_manifest();
                    manifest["profile"]["identity"]["expertise"] = json!(["\t"]);
                    manifest
                },
                "/profile/identity/expertise/0",
            ),
            (
                {
                    let mut manifest = base_profile_manifest();
                    manifest["profile"]["principles"] = json!(["  "]);
                    manifest
                },
                "/profile/principles/0",
            ),
            (
                {
                    let mut manifest = base_profile_manifest();
                    manifest["profile"]["audience"] = json!({
                        "description": "  ",
                        "adaptation": ["Match the customer."]
                    });
                    manifest
                },
                "/profile/audience/description",
            ),
            (
                {
                    let mut manifest = base_profile_manifest();
                    manifest["profile"]["audience"] = json!({
                        "assumed_knowledge": " ",
                        "adaptation": ["Match the customer."]
                    });
                    manifest
                },
                "/profile/audience/assumed_knowledge",
            ),
            (
                {
                    let mut manifest = base_profile_manifest();
                    manifest["profile"]["audience"] = json!({
                        "description": "Audience",
                        "adaptation": ["   "]
                    });
                    manifest
                },
                "/profile/audience/adaptation/0",
            ),
            (
                {
                    let mut manifest = base_profile_manifest();
                    manifest["profile"]["communication"]["guidelines"] = json!([" "]);
                    manifest
                },
                "/profile/communication/guidelines/0",
            ),
            (
                {
                    let mut manifest = base_profile_manifest();
                    manifest["profile"]["communication"]["formatting"] = json!(["\n"]);
                    manifest
                },
                "/profile/communication/formatting/0",
            ),
            (
                {
                    let mut manifest = base_profile_manifest();
                    manifest["profile"]["communication"]["vocabulary"] = json!({
                        "prefer": ["  "]
                    });
                    manifest
                },
                "/profile/communication/vocabulary/prefer/0",
            ),
            (
                {
                    let mut manifest = base_profile_manifest();
                    manifest["profile"]["communication"]["vocabulary"] = json!({
                        "avoid": ["\t"]
                    });
                    manifest
                },
                "/profile/communication/vocabulary/avoid/0",
            ),
            (
                {
                    let mut manifest = base_profile_manifest();
                    manifest["profile"]["boundaries"] = json!(["  "]);
                    manifest
                },
                "/profile/boundaries/0",
            ),
        ];

        for (manifest, expected_path) in cases {
            let issues = assert_manifest_invalid(manifest);
            assert!(
                issues
                    .iter()
                    .any(|issue| issue.instance_path == expected_path),
                "expected whitespace-only optional failure at {expected_path}, got: {issues:#?}"
            );
        }
    }

    #[test]
    fn profile_semantics_reject_duplicate_constraint_ids_with_precise_path() {
        let mut manifest = base_profile_manifest();
        manifest["profile"]["constraints"] = json!([
            {
                "id": "protect-authentication-data",
                "strength": "required",
                "instruction": "Never request a raw password."
            },
            {
                "id": "protect-authentication-data",
                "strength": "preferred",
                "instruction": "Do not ask for secrets in chat."
            }
        ]);

        let issues = assert_manifest_invalid(manifest);
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/profile/constraints/1/id"),
            "expected precise duplicate constraint id path, got: {issues:#?}"
        );
    }

    #[test]
    fn profile_semantics_reject_normalized_vocabulary_duplicates_and_overlap() {
        let duplicate_prefer = {
            let mut manifest = base_profile_manifest();
            manifest["profile"]["communication"]["vocabulary"] = json!({
                "prefer": ["Resolve", " resolve  "]
            });
            manifest
        };
        let issues = assert_manifest_invalid(duplicate_prefer);
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/profile/communication/vocabulary/prefer/1"),
            "expected normalized prefer duplicate path, got: {issues:#?}"
        );

        let duplicate_avoid = {
            let mut manifest = base_profile_manifest();
            manifest["profile"]["communication"]["vocabulary"] = json!({
                "avoid": ["obviously", " Obviously "]
            });
            manifest
        };
        let issues = assert_manifest_invalid(duplicate_avoid);
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/profile/communication/vocabulary/avoid/1"),
            "expected normalized avoid duplicate path, got: {issues:#?}"
        );

        let overlap = {
            let mut manifest = base_profile_manifest();
            manifest["profile"]["communication"]["vocabulary"] = json!({
                "prefer": ["Resolve"],
                "avoid": [" resolve "]
            });
            manifest
        };
        let issues = assert_manifest_invalid(overlap);
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/profile/communication/vocabulary/avoid/0"),
            "expected prefer/avoid overlap path, got: {issues:#?}"
        );
    }

    #[test]
    fn profile_semantics_profile_errors_fail_but_schema_warnings_remain_strict_only() {
        let path = temp_path("profile-strict");
        let raw = json!({
            "kind": "profile",
            "name": "customer-success-advocate",
            "version": "1.0.0",
            "description": "Support behavior profile.",
            "profile": {
                "identity": {
                    "role": "Support agent"
                },
                "objectives": ["Help."],
                "communication": {
                    "tone": ["warm"],
                    "verbosity": "concise"
                }
            }
        });

        write_manifest_pretty(&path, &raw).unwrap();
        let (mut manifest, _) = load_manifest_value(&path).unwrap();
        let (ok, issues) =
            validate_manifest_value(&schema_path(), "agent.json", &mut manifest, false).unwrap();
        assert!(
            ok,
            "expected non-strict semantic validation to pass with only warning"
        );
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].level, "warning");

        let invalid = {
            let mut invalid = raw.clone();
            invalid["profile"]["communication"]["tone"][0] = json!("   ");
            invalid
        };
        write_manifest_pretty(&path, &invalid).unwrap();
        let (mut manifest, _) = load_manifest_value(&path).unwrap();
        let (ok, issues) =
            validate_manifest_value(&schema_path(), "agent.json", &mut manifest, false).unwrap();
        assert!(!ok, "expected semantic profile error to fail validation");
        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/profile/communication/tone/0"),
            "expected precise semantic error path, got: {issues:#?}"
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn lint_fix_can_add_schema_to_profile_without_rewriting_authored_content() {
        let path = temp_path("profile-fix");
        let raw = json!({
            "kind": "profile",
            "name": "customer-success-advocate",
            "version": "1.0.0",
            "description": "Support behavior profile.",
            "profile": {
                "identity": {
                    "role": "Support agent",
                    "description": "Represents the team."
                },
                "objectives": ["Help."],
                "communication": {
                    "tone": ["warm", "professional"],
                    "verbosity": "concise",
                    "vocabulary": {
                        "prefer": ["Resolve", "Next step"]
                    }
                }
            }
        });

        write_manifest_pretty(&path, &raw).unwrap();
        let (mut manifest, _) = load_manifest_value(&path).unwrap();
        let (ok, issues) =
            validate_manifest_value(&schema_path(), "agent.json", &mut manifest, true).unwrap();

        assert!(
            ok,
            "expected lint fix validation to succeed, got: {issues:#?}"
        );
        assert_eq!(
            manifest.get("$schema").and_then(Value::as_str),
            Some(schema_path().as_str())
        );
        assert_eq!(manifest["profile"]["identity"]["role"], "Support agent");
        assert_eq!(
            manifest["profile"]["communication"]["vocabulary"]["prefer"][0],
            "Resolve"
        );
        assert_eq!(
            manifest["profile"]["communication"]["vocabulary"]["prefer"][1],
            "Next step"
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn top_level_profile_is_rejected_for_every_non_profile_kind() {
        let cases = vec![
            json!({
                "kind": "agent",
                "name": "bad-agent-profile",
                "version": "0.1.0",
                "description": "Agents must not use singular profile metadata.",
                "tools": [],
                "profile": {
                    "identity": { "role": "Support agent" },
                    "objectives": ["Help."],
                    "communication": { "tone": ["warm"], "verbosity": "concise" }
                }
            }),
            json!({
                "kind": "tool",
                "name": "bad-tool-profile",
                "version": "0.1.0",
                "description": "Tools must not use singular profile metadata.",
                "entrypoint": { "command": "python", "args": ["main.py"] },
                "inputs": { "type": "object" },
                "outputs": { "type": "object" },
                "files": ["main.py"],
                "profile": {
                    "identity": { "role": "Support agent" },
                    "objectives": ["Help."],
                    "communication": { "tone": ["warm"], "verbosity": "concise" }
                }
            }),
            json!({
                "kind": "template",
                "name": "bad-template-profile",
                "version": "0.1.0",
                "description": "Templates must not use singular profile metadata.",
                "template": {
                    "display_name": "Bad Template",
                    "use_case": "support",
                    "execution_surfaces": ["python-sdk"],
                    "files_root": "template",
                    "variables": [],
                    "dependencies": { "tools": [], "agents": [] },
                    "entrypoints": [{ "label": "Run", "command": "python main.py" }]
                },
                "profile": {
                    "identity": { "role": "Support agent" },
                    "objectives": ["Help."],
                    "communication": { "tone": ["warm"], "verbosity": "concise" }
                }
            }),
            json!({
                "kind": "skill",
                "name": "bad-skill-profile",
                "version": "0.1.0",
                "description": "Skills must not use singular profile metadata.",
                "skill": { "entrypoint": "SKILL.md" },
                "profile": {
                    "identity": { "role": "Support agent" },
                    "objectives": ["Help."],
                    "communication": { "tone": ["warm"], "verbosity": "concise" }
                }
            }),
            json!({
                "kind": "knowledge",
                "name": "bad-knowledge-profile",
                "version": "0.1.0",
                "description": "Knowledge packages must not use singular profile metadata.",
                "knowledge": { "mode": "context" },
                "profile": {
                    "identity": { "role": "Support agent" },
                    "objectives": ["Help."],
                    "communication": { "tone": ["warm"], "verbosity": "concise" }
                }
            }),
            json!({
                "kind": "memory",
                "name": "bad-memory-profile",
                "version": "0.1.0",
                "description": "Memory packages must not use singular profile metadata.",
                "memory": {
                    "scopes": { "user": { "description": "User scope." } },
                    "record_types": {
                        "preference": {
                            "version": "1.0.0",
                            "description": "Preference record.",
                            "schema": "schemas/preference.schema.json"
                        }
                    },
                    "spaces": {
                        "profile": {
                            "description": "Profile document.",
                            "model": "document",
                            "record_types": ["preference"],
                            "scope": ["user"],
                            "retrieval": { "modes": ["key"] }
                        }
                    }
                },
                "profile": {
                    "identity": { "role": "Support agent" },
                    "objectives": ["Help."],
                    "communication": { "tone": ["warm"], "verbosity": "concise" }
                }
            }),
        ];

        for manifest in cases {
            let issues = assert_manifest_invalid(manifest);
            assert!(
                issues
                    .iter()
                    .any(|issue| issue.schema_path
                        == "/dependentSchemas/profile/properties/kind/const"),
                "expected singular profile rejection outside kind=profile, got: {issues:#?}"
            );
        }
    }

    #[test]
    fn top_level_profiles_is_rejected_for_every_non_agent_kind() {
        let cases = vec![
            json!({
                "kind": "tool",
                "name": "bad-tool-profiles",
                "version": "0.1.0",
                "description": "Tools must not declare top-level profiles.",
                "entrypoint": { "command": "python", "args": ["main.py"] },
                "inputs": { "type": "object" },
                "outputs": { "type": "object" },
                "files": ["main.py"],
                "profiles": ["@zack/customer-success-advocate@1.0.0"]
            }),
            json!({
                "kind": "template",
                "name": "bad-template-profiles",
                "version": "0.1.0",
                "description": "Templates must not declare top-level profiles.",
                "template": {
                    "display_name": "Bad Template",
                    "use_case": "support",
                    "execution_surfaces": ["python-sdk"],
                    "files_root": "template",
                    "variables": [],
                    "dependencies": { "tools": [], "agents": [] },
                    "entrypoints": [{ "label": "Run", "command": "python main.py" }]
                },
                "profiles": ["@zack/customer-success-advocate@1.0.0"]
            }),
            json!({
                "kind": "skill",
                "name": "bad-skill-profiles",
                "version": "0.1.0",
                "description": "Skills must not declare top-level profiles.",
                "skill": { "entrypoint": "SKILL.md" },
                "profiles": ["@zack/customer-success-advocate@1.0.0"]
            }),
            json!({
                "kind": "knowledge",
                "name": "bad-knowledge-profiles",
                "version": "0.1.0",
                "description": "Knowledge must not declare top-level profiles.",
                "knowledge": { "mode": "context" },
                "profiles": ["@zack/customer-success-advocate@1.0.0"]
            }),
            json!({
                "kind": "memory",
                "name": "bad-memory-profiles",
                "version": "0.1.0",
                "description": "Memory must not declare top-level profiles.",
                "memory": {
                    "scopes": { "user": { "description": "User scope." } },
                    "record_types": {
                        "preference": {
                            "version": "1.0.0",
                            "description": "Preference record.",
                            "schema": "schemas/preference.schema.json"
                        }
                    },
                    "spaces": {
                        "profile": {
                            "description": "Profile document.",
                            "model": "document",
                            "record_types": ["preference"],
                            "scope": ["user"],
                            "retrieval": { "modes": ["key"] }
                        }
                    }
                },
                "profiles": ["@zack/customer-success-advocate@1.0.0"]
            }),
            json!({
                "kind": "profile",
                "name": "bad-profile-profiles",
                "version": "1.0.0",
                "description": "Profiles must not declare top-level profiles.",
                "profile": {
                    "identity": { "role": "Support agent" },
                    "objectives": ["Help."],
                    "communication": { "tone": ["warm"], "verbosity": "concise" }
                },
                "profiles": ["@zack/customer-success-advocate@1.0.0"]
            }),
        ];

        for manifest in cases {
            let issues = assert_manifest_invalid(manifest);
            assert!(
                issues
                    .iter()
                    .any(|issue| issue.schema_path
                        == "/dependentSchemas/profiles/properties/kind/const"),
                "expected plural profiles rejection outside kind=agent, got: {issues:#?}"
            );
        }
    }

    #[test]
    fn top_level_loop_is_rejected_for_every_non_agent_non_loop_kind() {
        let cases = vec![
            json!({
                "kind": "tool",
                "name": "bad-tool-loop",
                "version": "0.1.0",
                "description": "Tools must not declare top-level loop.",
                "entrypoint": { "command": "python", "args": ["main.py"] },
                "inputs": { "type": "object" },
                "outputs": { "type": "object" },
                "files": ["main.py"],
                "loop": {
                    "entry_phase": "triage",
                    "phases": [{ "id": "triage", "objective": "Assess." }],
                    "transitions": [{ "from": "triage", "on": "complete", "to": "$end" }]
                }
            }),
            json!({
                "kind": "template",
                "name": "bad-template-loop",
                "version": "0.1.0",
                "description": "Templates must not declare top-level loop.",
                "template": {
                    "display_name": "Bad Template",
                    "use_case": "support",
                    "execution_surfaces": ["python-sdk"],
                    "files_root": "template",
                    "variables": [],
                    "dependencies": { "tools": [], "agents": [] },
                    "entrypoints": [{ "label": "Run", "command": "python main.py" }]
                },
                "loop": {
                    "entry_phase": "triage",
                    "phases": [{ "id": "triage", "objective": "Assess." }],
                    "transitions": [{ "from": "triage", "on": "complete", "to": "$end" }]
                }
            }),
            json!({
                "kind": "skill",
                "name": "bad-skill-loop",
                "version": "0.1.0",
                "description": "Skills must not declare top-level loop.",
                "skill": { "entrypoint": "SKILL.md" },
                "loop": {
                    "entry_phase": "triage",
                    "phases": [{ "id": "triage", "objective": "Assess." }],
                    "transitions": [{ "from": "triage", "on": "complete", "to": "$end" }]
                }
            }),
            json!({
                "kind": "knowledge",
                "name": "bad-knowledge-loop",
                "version": "0.1.0",
                "description": "Knowledge must not declare top-level loop.",
                "knowledge": { "mode": "context" },
                "loop": {
                    "entry_phase": "triage",
                    "phases": [{ "id": "triage", "objective": "Assess." }],
                    "transitions": [{ "from": "triage", "on": "complete", "to": "$end" }]
                }
            }),
            json!({
                "kind": "memory",
                "name": "bad-memory-loop",
                "version": "0.1.0",
                "description": "Memory must not declare top-level loop.",
                "memory": {
                    "scopes": { "user": { "description": "User scope." } },
                    "record_types": {
                        "preference": {
                            "version": "1.0.0",
                            "description": "Preference record.",
                            "schema": "schemas/preference.schema.json"
                        }
                    },
                    "spaces": {
                        "profile": {
                            "description": "Profile document.",
                            "model": "document",
                            "record_types": ["preference"],
                            "scope": ["user"],
                            "retrieval": { "modes": ["key"] }
                        }
                    }
                },
                "loop": {
                    "entry_phase": "triage",
                    "phases": [{ "id": "triage", "objective": "Assess." }],
                    "transitions": [{ "from": "triage", "on": "complete", "to": "$end" }]
                }
            }),
            json!({
                "kind": "profile",
                "name": "bad-profile-loop",
                "version": "1.0.0",
                "description": "Profiles must not declare top-level loop.",
                "profile": {
                    "identity": { "role": "Support agent" },
                    "objectives": ["Help."],
                    "communication": { "tone": ["warm"], "verbosity": "concise" }
                },
                "loop": {
                    "entry_phase": "triage",
                    "phases": [{ "id": "triage", "objective": "Assess." }],
                    "transitions": [{ "from": "triage", "on": "complete", "to": "$end" }]
                }
            }),
        ];

        for manifest in cases {
            let issues = assert_manifest_invalid(manifest);
            assert!(
                issues
                    .iter()
                    .any(|issue| issue.schema_path == "/dependentSchemas/loop/oneOf"),
                "expected top-level loop rejection outside kind=agent/kind=loop, got: {issues:#?}"
            );
        }
    }

    #[test]
    fn non_agent_non_memory_top_level_memory_fails() {
        let issues = assert_manifest_invalid(json!({
            "kind": "skill",
            "name": "bad-skill-memory",
            "version": "0.1.0",
            "description": "Only agents and memory packages may use top-level memory.",
            "memory": [],
            "skill": {
                "entrypoint": "SKILL.md"
            }
        }));

        assert!(
            issues.iter().any(|issue| issue.instance_path.is_empty())
                || issues
                    .iter()
                    .any(|issue| issue.schema_path == "/dependentSchemas/memory/oneOf"),
            "expected top-level memory rejection outside agent/memory kinds, got: {issues:#?}"
        );
    }

    #[test]
    fn tool_manifest_with_top_level_memory_fails() {
        let issues = assert_manifest_invalid(json!({
            "kind": "tool",
            "name": "bad-tool-memory",
            "version": "0.1.0",
            "description": "Tools must not declare top-level memory.",
            "memory": [],
            "entrypoint": {
                "command": "python",
                "args": ["main.py"]
            },
            "inputs": { "type": "object" },
            "outputs": { "type": "object" },
            "files": ["main.py"]
        }));

        assert!(
            issues.iter().any(|issue| issue.instance_path.is_empty())
                || issues
                    .iter()
                    .any(|issue| issue.schema_path == "/dependentSchemas/memory/oneOf"),
            "expected top-level memory rejection for kind=tool, got: {issues:#?}"
        );
    }

    #[test]
    fn template_manifest_with_top_level_memory_fails() {
        let issues = assert_manifest_invalid(json!({
            "kind": "template",
            "name": "bad-template-memory",
            "version": "0.1.0",
            "description": "Templates must not declare top-level memory.",
            "memory": [],
            "template": {
                "display_name": "Bad Template Memory",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": []
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ]
            }
        }));

        assert!(
            issues.iter().any(|issue| issue.instance_path.is_empty())
                || issues
                    .iter()
                    .any(|issue| issue.schema_path == "/dependentSchemas/memory/oneOf"),
            "expected top-level memory rejection for kind=template, got: {issues:#?}"
        );
    }

    #[test]
    fn knowledge_manifest_with_top_level_memory_fails() {
        let issues = assert_manifest_invalid(json!({
            "kind": "knowledge",
            "name": "bad-knowledge-memory",
            "version": "0.1.0",
            "description": "Knowledge packages must not declare top-level memory.",
            "memory": [],
            "knowledge": {
                "mode": "context"
            }
        }));

        assert!(
            issues.iter().any(|issue| issue.instance_path.is_empty())
                || issues
                    .iter()
                    .any(|issue| issue.schema_path == "/dependentSchemas/memory/oneOf"),
            "expected top-level memory rejection for kind=knowledge, got: {issues:#?}"
        );
    }

    #[test]
    fn memory_kind_with_dependency_array_fails() {
        let issues = assert_manifest_invalid(json!({
            "kind": "memory",
            "name": "bad-memory-shape",
            "version": "0.1.0",
            "description": "Memory packages must use metadata objects, not dependency arrays.",
            "memory": [
                { "name": "@zack/other-memory", "version": "0.1.0" }
            ]
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.schema_path == "/dependentSchemas/memory/oneOf"),
            "expected overloaded memory property rejection for kind=memory, got: {issues:#?}"
        );
    }

    #[test]
    fn memory_manifest_requires_scopes_record_types_and_spaces() {
        let issues = assert_manifest_invalid(json!({
            "kind": "memory",
            "name": "missing-sections",
            "version": "0.1.0",
            "description": "Memory manifests must declare the required top-level sections.",
            "memory": {
                "record_types": {},
                "spaces": {}
            }
        }));

        assert!(
            issues.iter().any(|issue| {
                issue.instance_path == "/memory" && issue.schema_path == "/properties/memory/oneOf"
            }),
            "expected missing required memory sections failure, got: {issues:#?}"
        );
    }

    #[test]
    fn memory_manifest_rejects_invalid_space_model() {
        let issues = assert_manifest_invalid(json!({
            "kind": "memory",
            "name": "bad-model",
            "version": "0.1.0",
            "description": "Invalid model should fail.",
            "memory": {
                "scopes": {
                    "user": { "description": "User scope." }
                },
                "record_types": {
                    "preference": {
                        "version": "1.0.0",
                        "description": "Preference record.",
                        "schema": "schemas/preference.schema.json"
                    }
                },
                "spaces": {
                    "profile": {
                        "description": "Profile document.",
                        "model": "graph",
                        "record_types": ["preference"],
                        "scope": ["user"],
                        "retrieval": { "modes": ["key"] }
                    }
                }
            }
        }));

        assert!(
            issues.iter().any(|issue| {
                issue.instance_path == "/memory" && issue.schema_path == "/properties/memory/oneOf"
            }),
            "expected invalid model failure, got: {issues:#?}"
        );
    }

    #[test]
    fn memory_manifest_rejects_invalid_retrieval_mode() {
        let issues = assert_manifest_invalid(json!({
            "kind": "memory",
            "name": "bad-retrieval",
            "version": "0.1.0",
            "description": "Invalid retrieval mode should fail.",
            "memory": {
                "scopes": {
                    "user": { "description": "User scope." }
                },
                "record_types": {
                    "preference": {
                        "version": "1.0.0",
                        "description": "Preference record.",
                        "schema": "schemas/preference.schema.json"
                    }
                },
                "spaces": {
                    "profile": {
                        "description": "Profile document.",
                        "model": "document",
                        "record_types": ["preference"],
                        "scope": ["user"],
                        "retrieval": { "modes": ["ranking"] }
                    }
                }
            }
        }));

        assert!(
            issues.iter().any(|issue| {
                issue.instance_path == "/memory" && issue.schema_path == "/properties/memory/oneOf"
            }),
            "expected invalid retrieval mode failure, got: {issues:#?}"
        );
    }

    #[test]
    fn memory_manifest_rejects_invalid_retention_action() {
        let issues = assert_manifest_invalid(json!({
            "kind": "memory",
            "name": "bad-retention",
            "version": "0.1.0",
            "description": "Invalid retention action should fail.",
            "memory": {
                "scopes": {
                    "user": { "description": "User scope." }
                },
                "record_types": {
                    "preference": {
                        "version": "1.0.0",
                        "description": "Preference record.",
                        "schema": "schemas/preference.schema.json"
                    }
                },
                "spaces": {
                    "profile": {
                        "description": "Profile document.",
                        "model": "document",
                        "record_types": ["preference"],
                        "scope": ["user"],
                        "retrieval": { "modes": ["key"] },
                        "retention": {
                            "ttl": "P30D",
                            "on_expire": "compact"
                        }
                    }
                }
            }
        }));

        assert!(
            issues.iter().any(|issue| {
                issue.instance_path == "/memory" && issue.schema_path == "/properties/memory/oneOf"
            }),
            "expected invalid retention action failure, got: {issues:#?}"
        );
    }

    #[test]
    fn memory_manifest_rejects_invalid_operation_type() {
        let issues = assert_manifest_invalid(json!({
            "kind": "memory",
            "name": "bad-operation",
            "version": "0.1.0",
            "description": "Invalid operation type should fail.",
            "memory": {
                "scopes": {
                    "user": { "description": "User scope." }
                },
                "record_types": {
                    "preference": {
                        "version": "1.0.0",
                        "description": "Preference record.",
                        "schema": "schemas/preference.schema.json"
                    }
                },
                "spaces": {
                    "profile": {
                        "description": "Profile document.",
                        "model": "document",
                        "record_types": ["preference"],
                        "scope": ["user"],
                        "retrieval": { "modes": ["key"] }
                    }
                },
                "operations": {
                    "refresh_profile": {
                        "type": "upsert",
                        "description": "Unsupported operation type.",
                        "trigger": { "type": "external" },
                        "inputs": [
                            {
                                "space": "profile",
                                "record_type": "preference"
                            }
                        ],
                        "output": {
                            "space": "profile",
                            "record_type": "preference"
                        },
                        "source_handling": "retain",
                        "preserve_provenance": true
                    }
                }
            }
        }));

        assert!(
            issues.iter().any(|issue| {
                issue.instance_path == "/memory" && issue.schema_path == "/properties/memory/oneOf"
            }),
            "expected invalid operation type failure, got: {issues:#?}"
        );
    }

    #[test]
    fn memory_manifest_rejects_transform_with_multiple_inputs() {
        let issues = assert_manifest_invalid(json!({
            "kind": "memory",
            "name": "bad-transform",
            "version": "0.1.0",
            "description": "Transform operations must declare exactly one input pairing.",
            "memory": {
                "scopes": {
                    "user": { "description": "User scope." }
                },
                "record_types": {
                    "interaction": {
                        "version": "1.0.0",
                        "description": "Interaction record.",
                        "schema": "schemas/interaction.schema.json"
                    },
                    "summary": {
                        "version": "1.0.0",
                        "description": "Summary record.",
                        "schema": "schemas/summary.schema.json"
                    }
                },
                "spaces": {
                    "recent_interactions": {
                        "description": "Recent interactions.",
                        "model": "sequence",
                        "record_types": ["interaction"],
                        "scope": ["user"],
                        "retrieval": { "modes": ["chronological"] }
                    },
                    "conversation_history": {
                        "description": "Conversation summary document.",
                        "model": "document",
                        "record_types": ["summary"],
                        "scope": ["user"],
                        "retrieval": { "modes": ["key"] }
                    }
                },
                "operations": {
                    "transform_summary": {
                        "type": "transform",
                        "description": "Bad transform with too many inputs.",
                        "trigger": { "type": "external" },
                        "inputs": [
                            {
                                "space": "recent_interactions",
                                "record_type": "interaction"
                            },
                            {
                                "space": "recent_interactions",
                                "record_type": "interaction"
                            }
                        ],
                        "output": {
                            "space": "conversation_history",
                            "record_type": "summary"
                        },
                        "source_handling": "retain",
                        "preserve_provenance": true
                    }
                }
            }
        }));

        assert!(
            issues.iter().any(|issue| {
                issue.instance_path == "/memory" && issue.schema_path == "/properties/memory/oneOf"
            }),
            "expected transform input-count rejection, got: {issues:#?}"
        );
    }

    #[test]
    fn memory_manifest_rejects_invalid_trigger_type() {
        let issues = assert_manifest_invalid(json!({
            "kind": "memory",
            "name": "bad-trigger",
            "version": "0.1.0",
            "description": "Invalid trigger type should fail.",
            "memory": {
                "scopes": {
                    "user": { "description": "User scope." }
                },
                "record_types": {
                    "preference": {
                        "version": "1.0.0",
                        "description": "Preference record.",
                        "schema": "schemas/preference.schema.json"
                    }
                },
                "spaces": {
                    "profile": {
                        "description": "Profile document.",
                        "model": "document",
                        "record_types": ["preference"],
                        "scope": ["user"],
                        "retrieval": { "modes": ["key"] }
                    }
                },
                "operations": {
                    "delete_profile": {
                        "type": "delete",
                        "description": "Bad trigger type.",
                        "trigger": { "type": "cron" },
                        "targets": [
                            { "space": "profile" }
                        ],
                        "cascade_derived_records": false
                    }
                }
            }
        }));

        assert!(
            issues.iter().any(|issue| {
                issue.instance_path == "/memory" && issue.schema_path == "/properties/memory/oneOf"
            }),
            "expected invalid trigger type failure, got: {issues:#?}"
        );
    }

    #[test]
    fn memory_manifest_rejects_unsafe_schema_path() {
        let issues = assert_manifest_invalid(json!({
            "kind": "memory",
            "name": "unsafe-schema",
            "version": "0.1.0",
            "description": "Unsafe schema path should fail.",
            "memory": {
                "scopes": {
                    "user": { "description": "User scope." }
                },
                "record_types": {
                    "preference": {
                        "version": "1.0.0",
                        "description": "Preference record.",
                        "schema": "../schemas/preference.schema.json"
                    }
                },
                "spaces": {
                    "profile": {
                        "description": "Profile document.",
                        "model": "document",
                        "record_types": ["preference"],
                        "scope": ["user"],
                        "retrieval": { "modes": ["key"] }
                    }
                }
            }
        }));

        assert!(
            issues.iter().any(|issue| {
                issue.instance_path == "/memory" && issue.schema_path == "/properties/memory/oneOf"
            }),
            "expected unsafe schema path failure, got: {issues:#?}"
        );
    }

    #[test]
    fn memory_manifest_rejects_invalid_key_names() {
        let issues = assert_manifest_invalid(json!({
            "kind": "memory",
            "name": "bad-keys",
            "version": "0.1.0",
            "description": "Invalid key names should fail.",
            "memory": {
                "scopes": {
                    "User": { "description": "User scope." }
                },
                "record_types": {
                    "preference": {
                        "version": "1.0.0",
                        "description": "Preference record.",
                        "schema": "schemas/preference.schema.json"
                    }
                },
                "spaces": {
                    "profile": {
                        "description": "Profile document.",
                        "model": "document",
                        "record_types": ["preference"],
                        "scope": ["User"],
                        "retrieval": { "modes": ["key"] }
                    }
                }
            }
        }));

        assert!(
            issues.iter().any(|issue| {
                issue.instance_path == "/memory" && issue.schema_path == "/properties/memory/oneOf"
            }),
            "expected invalid key-name failure, got: {issues:#?}"
        );
    }

    #[test]
    fn memory_manifest_rejects_unsupported_additional_properties() {
        let issues = assert_manifest_invalid(json!({
            "kind": "memory",
            "name": "extra-fields",
            "version": "0.1.0",
            "description": "Unsupported extra properties should fail.",
            "memory": {
                "scopes": {
                    "user": {
                        "description": "User scope.",
                        "label": "User"
                    }
                },
                "record_types": {
                    "preference": {
                        "version": "1.0.0",
                        "description": "Preference record.",
                        "schema": "schemas/preference.schema.json"
                    }
                },
                "spaces": {
                    "profile": {
                        "description": "Profile document.",
                        "model": "document",
                        "record_types": ["preference"],
                        "scope": ["user"],
                        "retrieval": { "modes": ["key"] }
                    }
                }
            }
        }));

        assert!(
            issues.iter().any(|issue| {
                issue.instance_path == "/memory" && issue.schema_path == "/properties/memory/oneOf"
            }),
            "expected unsupported extra property failure, got: {issues:#?}"
        );
    }

    #[test]
    fn knowledge_manifest_rejects_unsafe_document_path() {
        let issues = assert_manifest_invalid(json!({
            "kind": "knowledge",
            "name": "unsafe-context",
            "version": "0.1.0",
            "description": "Unsafe context document path should fail.",
            "knowledge": {
                "mode": "context",
                "documents": [
                    {
                        "path": "../secret.md"
                    }
                ]
            }
        }));

        assert!(
            issues.iter().any(|issue| {
                issue.instance_path == "/knowledge"
                    && issue.schema_path == "/properties/knowledge/oneOf"
            }),
            "expected unsafe knowledge document path failure, got: {issues:#?}"
        );
    }

    #[test]
    fn knowledge_manifest_chunking_strategy_accepts_open_ended_strings() {
        assert_manifest_ok(json!({
            "kind": "knowledge",
            "name": "custom-chunking",
            "version": "0.1.0",
            "description": "Custom chunking strategy labels should be accepted.",
            "knowledge": {
                "mode": "vector",
                "corpus": {
                    "chunks_path": "knowledge/chunks.jsonl",
                    "sources_path": "knowledge/sources.jsonl"
                },
                "chunking": {
                    "strategy": "my-org-custom-splitter"
                },
                "embedding": {
                    "id": "default",
                    "provider": "openai",
                    "model": "text-embedding-3-small",
                    "dimensions": 1536,
                    "metric": "cosine",
                    "normalized": true,
                    "vectors_path": "knowledge/embeddings/default.f32"
                }
            }
        }));
    }

    #[test]
    fn knowledge_manifest_retrieval_strategy_accepts_open_ended_strings() {
        assert_manifest_ok(json!({
            "kind": "knowledge",
            "name": "custom-retrieval",
            "version": "0.1.0",
            "description": "Custom retrieval strategy labels should be accepted.",
            "knowledge": {
                "mode": "vector",
                "corpus": {
                    "chunks_path": "knowledge/chunks.jsonl",
                    "sources_path": "knowledge/sources.jsonl"
                },
                "embedding": {
                    "id": "default",
                    "provider": "openai",
                    "model": "text-embedding-3-small",
                    "dimensions": 1536,
                    "metric": "cosine",
                    "normalized": true,
                    "vectors_path": "knowledge/embeddings/default.f32"
                },
                "retrieval": {
                    "strategy": "hybrid-bm25-vector"
                }
            }
        }));
    }

    #[test]
    fn context_mode_knowledge_does_not_require_vector_fields() {
        assert_manifest_ok(json!({
            "kind": "knowledge",
            "name": "context-only",
            "version": "0.1.0",
            "description": "Context mode should not require vector-only fields.",
            "knowledge": {
                "mode": "context",
                "documents": [
                    {
                        "path": "knowledge/docs/playbook.md"
                    }
                ]
            }
        }));
    }

    #[test]
    fn skill_manifest_rejects_missing_entrypoint() {
        let issues = assert_manifest_invalid(json!({
            "kind": "skill",
            "name": "missing-entrypoint",
            "version": "0.1.0",
            "description": "Missing skill.entrypoint should fail.",
            "skill": {}
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.message.contains("entrypoint")),
            "expected missing entrypoint validation error, got: {issues:#?}"
        );
    }

    #[test]
    fn skill_manifest_rejects_unsafe_entrypoint_path() {
        let issues = assert_manifest_invalid(json!({
            "kind": "skill",
            "name": "unsafe-entrypoint",
            "version": "0.1.0",
            "description": "Unsafe entrypoint path should fail.",
            "skill": {
                "entrypoint": "../SKILL.md"
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/skill/entrypoint"),
            "expected unsafe entrypoint path failure, got: {issues:#?}"
        );
    }

    #[test]
    fn skill_manifest_rejects_absolute_entrypoint_path() {
        let issues = assert_manifest_invalid(json!({
            "kind": "skill",
            "name": "absolute-entrypoint",
            "version": "0.1.0",
            "description": "Absolute entrypoint path should fail.",
            "skill": {
                "entrypoint": "/tmp/SKILL.md"
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/skill/entrypoint"),
            "expected absolute entrypoint path failure, got: {issues:#?}"
        );
    }

    #[test]
    fn skill_manifest_rejects_unsafe_reference_path() {
        let issues = assert_manifest_invalid(json!({
            "kind": "skill",
            "name": "unsafe-reference",
            "version": "0.1.0",
            "description": "Unsafe reference path should fail.",
            "skill": {
                "entrypoint": "SKILL.md",
                "references": ["references/../../secret.md"]
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/skill/references/0"),
            "expected unsafe reference path failure, got: {issues:#?}"
        );
    }

    #[test]
    fn skill_manifest_rejects_unsafe_script_path() {
        let issues = assert_manifest_invalid(json!({
            "kind": "skill",
            "name": "unsafe-script",
            "version": "0.1.0",
            "description": "Unsafe script path should fail.",
            "skill": {
                "entrypoint": "SKILL.md",
                "scripts": ["/tmp/run.sh"]
            }
        }));

        assert!(
            issues
                .iter()
                .any(|issue| issue.instance_path == "/skill/scripts/0"),
            "expected unsafe script path failure, got: {issues:#?}"
        );
    }

    #[test]
    fn template_manifest_rejects_unsupported_extra_properties() {
        let issues = assert_manifest_invalid(json!({
            "kind": "template",
            "name": "extra-properties-template",
            "version": "0.1.0",
            "description": "Unsupported template properties should fail.",
            "template": {
                "display_name": "Extra Properties Template",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": []
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ],
                "hooks": ["pre_generate"]
            }
        }));

        assert!(
            issues.iter().any(|issue| issue
                .message
                .contains("Additional properties are not allowed")),
            "expected unsupported extra properties failure, got: {issues:#?}"
        );
    }

    #[test]
    fn agent_template_matches_phase_three_shape() {
        let rendered = include_str!("../assets/templates/agent.json.tpl")
            .replace("{{AGENT_NAME}}", "support-agent")
            .replace(
                "{{AGENT_DESCRIPTION}}",
                "Triage support requests using installed tools.",
            );
        let mut manifest: Value = serde_json::from_str(&rendered).unwrap();
        let (ok, issues) =
            validate_manifest_value(&schema_path(), "agent.json", &mut manifest, false).unwrap();

        assert!(
            ok,
            "expected rendered template to validate, got: {issues:#?}"
        );
        assert_eq!(manifest["kind"], "agent");
        assert_eq!(manifest["tools"], json!([]));
        assert_eq!(manifest["skills"], json!([]));
        assert_eq!(manifest["knowledge"], json!([]));
        assert_eq!(manifest["memory"], json!([]));
        assert_eq!(manifest["profiles"], json!([]));
        assert_eq!(manifest["examples"][0]["title"], "Example prompt");
        assert!(manifest.get("entrypoint").is_none());
    }

    #[test]
    fn lint_fix_can_add_schema_to_agent_template() {
        let path = temp_path("agent-template");
        let raw = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "description": "Triage support requests using installed tools.",
            "tools": [],
            "skills": [],
            "knowledge": [],
            "memory": [],
            "profiles": [],
            "examples": [
                {
                    "title": "Example prompt",
                    "prompt": "Describe the user request this agent should handle."
                }
            ]
        });

        write_manifest_pretty(&path, &raw).unwrap();
        let (mut manifest, _) = load_manifest_value(&path).unwrap();
        let (ok, issues) =
            validate_manifest_value(&schema_path(), "agent.json", &mut manifest, true).unwrap();

        assert!(
            ok,
            "expected lint fix validation to succeed, got: {issues:#?}"
        );
        assert_eq!(
            manifest.get("$schema").and_then(Value::as_str),
            Some(schema_path().as_str())
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn parse_publish_manifest_dispatches_tool_kind() {
        let manifest = json!({
            "kind": "tool",
            "name": "summarize",
            "version": "1.2.3",
            "description": "Summarize text.",
            "runtime": {"type": "python", "version": "3.11"},
            "entrypoint": {
                "command": "python",
                "args": ["main.py"]
            },
            "files": ["main.py"],
            "inputs": {"type": "object"},
            "outputs": {"type": "object"}
        });

        match parse_publish_manifest(&manifest).unwrap() {
            PublishManifest::Tool(mf) => {
                assert_eq!(mf.kind, "tool");
                assert_eq!(mf.name, "summarize");
            }
            PublishManifest::Agent(_)
            | PublishManifest::Template(_)
            | PublishManifest::Skill(_)
            | PublishManifest::Knowledge(_)
            | PublishManifest::Memory(_)
            | PublishManifest::Profile(_) => {
                panic!("expected tool publish manifest")
            }
        }
    }

    #[test]
    fn parse_publish_manifest_dispatches_agent_kind() {
        let manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "description": "Support agent.",
            "tools": ["@zack/slack-post-message@0.1.0"],
            "skills": [],
            "knowledge": [],
            "memory": [],
            "profiles": [],
            "examples": [{"title": "Example", "prompt": "Help the user."}]
        });

        match parse_publish_manifest(&manifest).unwrap() {
            PublishManifest::Agent(mf) => {
                assert_eq!(mf.kind, "agent");
                assert_eq!(mf.name, "support-agent");
            }
            PublishManifest::Tool(_)
            | PublishManifest::Template(_)
            | PublishManifest::Skill(_)
            | PublishManifest::Knowledge(_)
            | PublishManifest::Memory(_)
            | PublishManifest::Profile(_) => {
                panic!("expected agent publish manifest")
            }
        }
    }

    #[test]
    fn parse_publish_manifest_dispatches_skill_kind() {
        let manifest = json!({
            "kind": "skill",
            "name": "incident-commander",
            "version": "0.1.0",
            "description": "Incident response coordination playbook.",
            "tools": [
                {
                    "name": "@zack/slack-post-message",
                    "version": "0.1.1"
                }
            ],
            "skill": {
                "entrypoint": "SKILL.md",
                "references": ["references/tool-contract.md"],
                "scripts": ["scripts/run.sh"]
            }
        });

        match parse_publish_manifest(&manifest).unwrap() {
            PublishManifest::Skill(mf) => {
                assert_eq!(mf.kind, "skill");
                assert_eq!(mf.name, "incident-commander");
                assert_eq!(mf.skill.entrypoint, "SKILL.md");
                assert_eq!(mf.tools.len(), 1);
            }
            PublishManifest::Tool(_)
            | PublishManifest::Agent(_)
            | PublishManifest::Template(_)
            | PublishManifest::Knowledge(_)
            | PublishManifest::Memory(_)
            | PublishManifest::Profile(_) => {
                panic!("expected skill publish manifest")
            }
        }
    }

    #[test]
    fn parse_publish_manifest_dispatches_template_kind() {
        let manifest = json!({
            "kind": "template",
            "name": "research-assistant",
            "version": "0.1.0",
            "description": "Research starter template.",
            "template": {
                "display_name": "Research Assistant",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": []
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ]
            }
        });

        match parse_publish_manifest(&manifest).unwrap() {
            PublishManifest::Template(mf) => {
                assert_eq!(mf.kind, "template");
                assert_eq!(mf.name, "research-assistant");
                assert_eq!(mf.template.files_root, "template");
                assert!(mf.template.dependencies.skills.is_empty());
                assert!(mf.template.dependencies.knowledge.is_empty());
                assert!(mf.template.dependencies.memory.is_empty());
            }
            PublishManifest::Tool(_)
            | PublishManifest::Agent(_)
            | PublishManifest::Skill(_)
            | PublishManifest::Knowledge(_)
            | PublishManifest::Memory(_)
            | PublishManifest::Profile(_) => {
                panic!("expected template publish manifest")
            }
        }
    }

    #[test]
    fn parse_publish_manifest_dispatches_knowledge_kind() {
        let manifest = json!({
            "kind": "knowledge",
            "name": "engineering-playbook",
            "version": "0.1.0",
            "description": "Engineering playbook intended for direct context loading.",
            "knowledge": {
                "mode": "context",
                "documents": [
                    {
                        "path": "knowledge/docs/playbook.md"
                    }
                ]
            }
        });

        match parse_publish_manifest(&manifest).unwrap() {
            PublishManifest::Knowledge(mf) => {
                assert_eq!(mf.kind, "knowledge");
                assert_eq!(mf.name, "engineering-playbook");
                assert_eq!(mf.knowledge.mode, "context");
                assert_eq!(mf.knowledge.documents.len(), 1);
            }
            PublishManifest::Tool(_)
            | PublishManifest::Agent(_)
            | PublishManifest::Template(_)
            | PublishManifest::Skill(_)
            | PublishManifest::Memory(_)
            | PublishManifest::Profile(_) => {
                panic!("expected knowledge publish manifest")
            }
        }
    }

    #[test]
    fn parse_publish_manifest_dispatches_memory_kind() {
        let manifest = json!({
            "kind": "memory",
            "name": "conversation-continuity",
            "version": "0.1.0",
            "description": "Portable structure for conversational continuity memory.",
            "memory": {
                "scopes": {
                    "user": {
                        "description": "The user whose memory is being retained."
                    }
                },
                "record_types": {
                    "user_preference": {
                        "version": "1.0.0",
                        "description": "Durable structured preferences for one user.",
                        "schema": "schemas/user-preference.schema.json"
                    }
                },
                "spaces": {
                    "profile": {
                        "description": "The current durable profile for one user.",
                        "model": "document",
                        "record_types": ["user_preference"],
                        "scope": ["user"],
                        "retrieval": {
                            "modes": ["key"]
                        }
                    }
                }
            }
        });

        match parse_publish_manifest(&manifest).unwrap() {
            PublishManifest::Memory(mf) => {
                assert_eq!(mf.kind, "memory");
                assert_eq!(mf.name, "conversation-continuity");
                assert_eq!(mf.memory.scopes.len(), 1);
                assert_eq!(mf.memory.record_types.len(), 1);
                assert_eq!(mf.memory.spaces.len(), 1);
            }
            PublishManifest::Tool(_)
            | PublishManifest::Agent(_)
            | PublishManifest::Template(_)
            | PublishManifest::Skill(_)
            | PublishManifest::Knowledge(_)
            | PublishManifest::Profile(_) => {
                panic!("expected memory publish manifest")
            }
        }
    }

    #[test]
    fn parse_publish_manifest_dispatches_profile_kind() {
        let manifest = json!({
            "kind": "profile",
            "name": "customer-success-advocate",
            "version": "1.0.0",
            "description": "Support behavior profile.",
            "profile": {
                "identity": {
                    "role": "Senior Customer Success Advocate"
                },
                "objectives": ["Help the customer reach a clear next step."],
                "communication": {
                    "tone": ["warm"],
                    "verbosity": "concise"
                }
            }
        });

        match parse_publish_manifest(&manifest).unwrap() {
            PublishManifest::Profile(mf) => {
                assert_eq!(mf.kind, "profile");
                assert_eq!(mf.name, "customer-success-advocate");
                assert_eq!(mf.profile.identity.role, "Senior Customer Success Advocate");
            }
            PublishManifest::Tool(_)
            | PublishManifest::Agent(_)
            | PublishManifest::Template(_)
            | PublishManifest::Skill(_)
            | PublishManifest::Knowledge(_)
            | PublishManifest::Memory(_) => {
                panic!("expected profile publish manifest")
            }
        }
    }

    #[test]
    fn parse_publish_manifest_rejects_unknown_kind() {
        let manifest = json!({
            "kind": "workflow",
            "name": "starter",
            "version": "0.1.0"
        });

        let err = parse_publish_manifest(&manifest).unwrap_err().to_string();
        assert!(
            err.contains(
                "supports kind=\"tool\", kind=\"agent\", kind=\"template\", kind=\"skill\", kind=\"knowledge\", kind=\"memory\", and kind=\"profile\""
            ),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_template_manifest_accepts_template_kind() {
        let manifest = json!({
            "kind": "template",
            "name": "starter-template",
            "version": "0.1.0",
            "description": "Starter template.",
            "template": {
                "display_name": "Starter Template",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": []
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ]
            }
        });

        let parsed = parse_template_manifest(&manifest).unwrap();
        assert_eq!(parsed.kind, "template");
        assert_eq!(parsed.name, "starter-template");
        assert!(parsed.template.dependencies.skills.is_empty());
        assert!(parsed.template.dependencies.knowledge.is_empty());
        assert!(parsed.template.dependencies.memory.is_empty());
        assert!(parsed.template.dependencies.profiles.is_empty());
    }

    #[test]
    fn parse_template_manifest_preserves_skill_dependencies() {
        let manifest = json!({
            "kind": "template",
            "name": "starter-template",
            "version": "0.1.0",
            "description": "Starter template.",
            "template": {
                "display_name": "Starter Template",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": [],
                    "skills": [
                        {
                            "name": "@zack/incident-commander",
                            "version": "0.1.0"
                        }
                    ]
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ]
            }
        });

        let parsed = parse_template_manifest(&manifest).unwrap();
        assert_eq!(parsed.template.dependencies.skills.len(), 1);
        match &parsed.template.dependencies.skills[0] {
            PackageReference::Object { name, version } => {
                assert_eq!(name, "@zack/incident-commander");
                assert_eq!(version.as_deref(), Some("0.1.0"));
            }
            PackageReference::String(_) => panic!("expected object dependency reference"),
        }
    }

    #[test]
    fn parse_template_manifest_preserves_knowledge_dependencies() {
        let manifest = json!({
            "kind": "template",
            "name": "starter-template",
            "version": "0.1.0",
            "description": "Starter template.",
            "template": {
                "display_name": "Starter Template",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": [],
                    "knowledge": [
                        {
                            "name": "@zack/python-docs",
                            "version": "0.1.0"
                        }
                    ]
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ]
            }
        });

        let parsed = parse_template_manifest(&manifest).unwrap();
        assert_eq!(parsed.template.dependencies.knowledge.len(), 1);
        match &parsed.template.dependencies.knowledge[0] {
            PackageReference::Object { name, version } => {
                assert_eq!(name, "@zack/python-docs");
                assert_eq!(version.as_deref(), Some("0.1.0"));
            }
            PackageReference::String(_) => panic!("expected object dependency reference"),
        }
    }

    #[test]
    fn parse_template_manifest_preserves_memory_dependencies() {
        let manifest = json!({
            "kind": "template",
            "name": "starter-template",
            "version": "0.1.0",
            "description": "Starter template.",
            "template": {
                "display_name": "Starter Template",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": [],
                    "memory": [
                        {
                            "name": "@zack/conversation-continuity",
                            "version": "0.1.0"
                        }
                    ]
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ]
            }
        });

        let parsed = parse_template_manifest(&manifest).unwrap();
        assert_eq!(parsed.template.dependencies.memory.len(), 1);
        match &parsed.template.dependencies.memory[0] {
            PackageReference::Object { name, version } => {
                assert_eq!(name, "@zack/conversation-continuity");
                assert_eq!(version.as_deref(), Some("0.1.0"));
            }
            PackageReference::String(_) => panic!("expected object dependency reference"),
        }
    }

    #[test]
    fn parse_template_manifest_preserves_profile_dependencies() {
        let manifest = json!({
            "kind": "template",
            "name": "starter-template",
            "version": "0.1.0",
            "description": "Starter template.",
            "template": {
                "display_name": "Starter Template",
                "use_case": "research",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": [],
                    "profiles": [
                        "@zack/customer-success-advocate@1.0.0",
                        {
                            "name": "@zack/escalation-manager",
                            "version": "1.2.0"
                        }
                    ]
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ]
            }
        });

        let parsed = parse_template_manifest(&manifest).unwrap();
        assert_eq!(parsed.template.dependencies.profiles.len(), 2);
        match &parsed.template.dependencies.profiles[0] {
            PackageReference::String(value) => {
                assert_eq!(value, "@zack/customer-success-advocate@1.0.0");
            }
            PackageReference::Object { .. } => panic!("expected string dependency reference"),
        }
        match &parsed.template.dependencies.profiles[1] {
            PackageReference::Object { name, version } => {
                assert_eq!(name, "@zack/escalation-manager");
                assert_eq!(version.as_deref(), Some("1.2.0"));
            }
            PackageReference::String(_) => panic!("expected object dependency reference"),
        }
    }

    #[test]
    fn parse_template_manifest_preserves_loop_dependencies() {
        let manifest = json!({
            "kind": "template",
            "name": "incident-template",
            "version": "0.1.0",
            "description": "Incident template.",
            "template": {
                "display_name": "Incident Template",
                "use_case": "incident-response",
                "execution_surfaces": ["python-sdk"],
                "files_root": "template",
                "variables": [],
                "dependencies": {
                    "tools": [],
                    "agents": [],
                    "loop": "@zack/incident-response-loop@1.0.0"
                },
                "entrypoints": [
                    {
                        "label": "Run",
                        "command": "python main.py"
                    }
                ]
            }
        });

        let parsed = parse_template_manifest(&manifest).unwrap();
        match parsed.template.dependencies.r#loop.as_ref() {
            Some(PackageReference::String(value)) => {
                assert_eq!(value, "@zack/incident-response-loop@1.0.0");
            }
            Some(PackageReference::Object { .. }) => panic!("expected string dependency reference"),
            None => panic!("expected loop dependency"),
        }
    }

    #[test]
    fn parse_loop_manifest_accepts_loop_kind() {
        let manifest = json!({
            "kind": "loop",
            "name": "incident-response-loop",
            "version": "1.0.0",
            "description": "Incident loop.",
            "loop": {
                "entry_phase": "triage",
                "phases": [
                    {
                        "id": "triage",
                        "objective": "Assess the incident."
                    }
                ],
                "transitions": [
                    {
                        "from": "triage",
                        "on": "complete",
                        "to": "$end"
                    }
                ]
            }
        });

        let parsed = parse_loop_manifest(&manifest).unwrap();
        assert_eq!(parsed.kind, "loop");
        assert_eq!(parsed.name, "incident-response-loop");
        assert_eq!(parsed.r#loop.entry_phase, "triage");
        assert_eq!(parsed.r#loop.phases.len(), 1);
        assert_eq!(parsed.r#loop.transitions.len(), 1);
    }

    #[test]
    fn parse_loop_manifest_rejects_wrong_kind() {
        let manifest = json!({
            "kind": "profile",
            "name": "incident-response-loop",
            "version": "1.0.0",
            "description": "Incident loop with the wrong kind.",
            "loop": {
                "entry_phase": "triage",
                "phases": [
                    {
                        "id": "triage",
                        "objective": "Assess the incident."
                    }
                ],
                "transitions": [
                    {
                        "from": "triage",
                        "on": "complete",
                        "to": "$end"
                    }
                ]
            }
        });

        let err = parse_loop_manifest(&manifest).unwrap_err().to_string();
        assert!(
            err.contains("expected kind=\"loop\" manifest"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn agent_manifest_typed_parsing_preserves_loop_and_bindings() {
        let manifest = json!({
            "kind": "agent",
            "name": "incident-response-agent",
            "version": "1.0.0",
            "description": "Incident response agent.",
            "tools": ["@acme/get-incident-context@1.0.0"],
            "skills": ["@acme/incident-investigation@1.0.0"],
            "knowledge": ["@acme/incident-runbooks@1.0.0"],
            "memory": ["@acme/incident-memory@1.0.0"],
            "profiles": ["@acme/incident-responder@1.0.0"],
            "loop": "@acme/incident-response-loop@1.0.0",
            "bindings": {
                "global": {
                    "tools": ["@acme/get-incident-context"],
                    "memory": [
                        {
                            "package": "@acme/incident-memory",
                            "operations": ["compact_evidence"]
                        }
                    ]
                },
                "phases": {
                    "review": {
                        "profiles": ["@acme/incident-responder"]
                    }
                },
                "mcp": [
                    {
                        "id": "investigation-tools",
                        "tools": ["@acme/get-incident-context"]
                    }
                ],
                "consumer_context": {
                    "file": "INCIDENT_AGENT.md"
                }
            }
        });

        let parsed: AgentManifest = serde_json::from_value(manifest).unwrap();
        match parsed.loop_ref {
            Some(PackageReference::String(value)) => {
                assert_eq!(value, "@acme/incident-response-loop@1.0.0");
            }
            Some(PackageReference::Object { .. }) => panic!("expected string loop reference"),
            None => panic!("expected loop reference"),
        }
        let bindings = parsed.bindings.expect("expected bindings");
        assert_eq!(
            bindings
                .global
                .as_ref()
                .expect("expected global bindings")
                .tools,
            vec!["@acme/get-incident-context".to_string()]
        );
        assert_eq!(bindings.phases.len(), 1);
        assert_eq!(bindings.mcp.len(), 1);
        assert_eq!(
            bindings
                .consumer_context
                .as_ref()
                .expect("expected consumer context")
                .file,
            "INCIDENT_AGENT.md"
        );
    }

    #[test]
    fn parse_profile_manifest_accepts_profile_kind() {
        let manifest = json!({
            "kind": "profile",
            "name": "customer-success-advocate",
            "version": "1.0.0",
            "description": "Support behavior profile.",
            "profile": {
                "identity": {
                    "role": "Senior Customer Success Advocate"
                },
                "objectives": ["Help the customer reach a clear next step."],
                "communication": {
                    "tone": ["warm"],
                    "verbosity": "concise"
                }
            }
        });

        let parsed = parse_profile_manifest(&manifest).unwrap();
        assert_eq!(parsed.kind, "profile");
        assert_eq!(parsed.name, "customer-success-advocate");
        assert_eq!(
            parsed.profile.identity.role,
            "Senior Customer Success Advocate"
        );
        assert_eq!(
            parsed.profile.communication.verbosity,
            ProfileVerbosity::Concise
        );
    }

    #[test]
    fn parse_profile_manifest_rejects_wrong_kind() {
        let manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "description": "Support agent.",
            "profile": {
                "identity": {
                    "role": "Support agent"
                },
                "objectives": ["Help."],
                "communication": {
                    "tone": ["warm"],
                    "verbosity": "concise"
                }
            }
        });

        let err = parse_profile_manifest(&manifest).unwrap_err().to_string();
        assert!(
            err.contains("expected kind=\"profile\" manifest"),
            "unexpected error: {err}"
        );
    }
}
