#![allow(dead_code)]

use anyhow::{Context, Result, anyhow, bail};
use jsonschema::{Draft, JSONSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Component, Path, PathBuf},
};

const DEFAULT_HARNESS_CONFIG_FILE: &str = "agentpm.harness.json";
const EMBEDDED_HARNESS_CONFIG_SCHEMA_JSON: &str =
    include_str!("../../../schemas/agentpm.harness.schema.json");

const BUILT_IN_MODEL_PROVIDER_IDS: &[&str] = &["openai", "anthropic", "ollama"];

fn default_startup_timeout_ms() -> u64 {
    15_000
}

fn default_request_timeout_ms() -> u64 {
    120_000
}

fn default_restart_max_attempts() -> u32 {
    1
}

fn default_restart_backoff_ms() -> u64 {
    250
}

fn default_state_dir() -> String {
    ".agentpm-state".into()
}

fn default_max_steps() -> u64 {
    100
}

fn default_max_model_calls_per_phase() -> u64 {
    24
}

fn default_max_tool_calls_per_phase() -> u64 {
    16
}

fn default_max_actions_per_phase() -> u64 {
    64
}

fn default_max_repairs() -> u64 {
    2
}

fn default_trace_enabled() -> bool {
    true
}

fn default_branding_name() -> String {
    "AgentPM Harness".into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessConfigSourceKind {
    HarnessDefault,
    ConfigFile,
    CliOverride,
    SdkOverride,
    Environment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessConfigSource {
    pub kind: HarnessConfigSourceKind,
    pub path: Option<PathBuf>,
}

impl HarnessConfigSource {
    fn defaulted() -> Self {
        Self {
            kind: HarnessConfigSourceKind::HarnessDefault,
            path: None,
        }
    }

    fn config_file(path: PathBuf) -> Self {
        Self {
            kind: HarnessConfigSourceKind::ConfigFile,
            path: Some(path),
        }
    }

    pub fn cli_override() -> Self {
        Self {
            kind: HarnessConfigSourceKind::CliOverride,
            path: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedHarnessConfig {
    pub workspace_root: PathBuf,
    pub config_path: Option<PathBuf>,
    pub config: HarnessConfig,
    pub state_dir: PathBuf,
    pub state_dir_source: HarnessConfigSource,
}

#[derive(Debug, Clone, Default)]
pub struct HarnessConfigOverrides {
    pub state_dir: Option<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, deny_unknown_fields)]
pub struct HarnessConfig {
    pub version: u8,
    pub model: Option<HarnessModelConfig>,
    pub providers: HarnessProvidersConfig,
    pub scopes: HashMap<String, String>,
    pub runtime: HarnessRuntimeConfig,
    pub hooks: HarnessHooksConfig,
    pub knowledge: HarnessKnowledgeConfig,
    pub memory: HarnessMemoryConfig,
    pub mcp: HarnessMcpConfig,
    pub approvals: HarnessApprovalsConfig,
    pub trace: HarnessTraceConfig,
    pub ui: HarnessUiConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct HarnessModelConfig {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub options: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, deny_unknown_fields)]
pub struct HarnessProvidersConfig {
    pub models: HashMap<String, HarnessImplementationEntry>,
    pub embeddings: HashMap<String, HarnessImplementationEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct HarnessImplementationEntry {
    pub implementation: HarnessImplementation,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HarnessImplementation {
    Process {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        cwd: Option<String>,
        #[serde(default)]
        env: Vec<String>,
        #[serde(default = "default_startup_timeout_ms")]
        startup_timeout_ms: u64,
        #[serde(default = "default_request_timeout_ms")]
        request_timeout_ms: u64,
        #[serde(default)]
        restart: HarnessRestartPolicy,
    },
    Host {
        #[serde(default = "default_request_timeout_ms")]
        request_timeout_ms: u64,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct HarnessRestartPolicy {
    pub max_attempts: u32,
    pub backoff_ms: u64,
}

impl Default for HarnessRestartPolicy {
    fn default() -> Self {
        Self {
            max_attempts: default_restart_max_attempts(),
            backoff_ms: default_restart_backoff_ms(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct HarnessRuntimeConfig {
    pub state_dir: String,
    pub limits: HarnessRuntimeLimits,
}

impl Default for HarnessRuntimeConfig {
    fn default() -> Self {
        Self {
            state_dir: default_state_dir(),
            limits: HarnessRuntimeLimits::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct HarnessRuntimeLimits {
    pub max_steps: u64,
    pub max_model_calls_per_phase: u64,
    pub max_tool_calls_per_phase: u64,
    pub max_actions_per_phase: u64,
    pub max_tool_call_repairs: u64,
    pub max_structured_output_repairs: u64,
    pub max_memory_operation_repairs: u64,
}

impl Default for HarnessRuntimeLimits {
    fn default() -> Self {
        Self {
            max_steps: default_max_steps(),
            max_model_calls_per_phase: default_max_model_calls_per_phase(),
            max_tool_calls_per_phase: default_max_tool_calls_per_phase(),
            max_actions_per_phase: default_max_actions_per_phase(),
            max_tool_call_repairs: default_max_repairs(),
            max_structured_output_repairs: default_max_repairs(),
            max_memory_operation_repairs: default_max_repairs(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, deny_unknown_fields)]
pub struct HarnessHooksConfig {
    pub implementations: HashMap<String, HarnessImplementationEntry>,
    pub bindings: Vec<HarnessHookBinding>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct HarnessHookBinding {
    pub hook: HarnessHookId,
    pub implementation: String,
    #[serde(default)]
    pub failure_policy: HarnessHookFailurePolicy,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HarnessHookId {
    BeforeModelRequest,
    BeforeToolSelection,
    BeforeToolCall,
    BeforeKnowledgeRequest,
    AfterKnowledgeRetrieval,
    BeforeMemoryRead,
    BeforeMemoryWrite,
    BeforeMemoryOperation,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum HarnessHookFailurePolicy {
    #[default]
    Closed,
    Continue,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, deny_unknown_fields)]
pub struct HarnessKnowledgeConfig {
    pub runtimes: HashMap<String, HarnessImplementationEntry>,
    pub packages: HashMap<String, HarnessRuntimeMapping>,
    pub embedding_matches: Vec<HarnessKnowledgeEmbeddingMatch>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct HarnessRuntimeMapping {
    pub runtime: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct HarnessKnowledgeEmbeddingMatch {
    pub r#match: HarnessEmbeddingMatchKey,
    pub embedding_provider: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct HarnessEmbeddingMatchKey {
    pub provider: String,
    pub model: String,
    pub dimensions: u64,
    pub normalized: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, deny_unknown_fields)]
pub struct HarnessMemoryConfig {
    pub local: HarnessMemoryLocalConfig,
    pub runtimes: HashMap<String, HarnessImplementationEntry>,
    pub packages: HashMap<String, HarnessRuntimeMapping>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, deny_unknown_fields)]
pub struct HarnessMemoryLocalConfig {
    pub semantic: Option<HarnessMemorySemanticConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct HarnessMemorySemanticConfig {
    pub embedding_provider: String,
    pub model: String,
    pub dimensions: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, deny_unknown_fields)]
pub struct HarnessMcpConfig {
    pub imports: HashMap<String, HarnessMcpImport>,
    pub exports: HarnessMcpExports,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum HarnessMcpImport {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        cwd: Option<String>,
        #[serde(default)]
        env: Vec<String>,
        scope: HarnessMcpScope,
        tools: Option<Vec<String>>,
        #[serde(default = "default_startup_timeout_ms")]
        startup_timeout_ms: u64,
        #[serde(default = "default_request_timeout_ms")]
        request_timeout_ms: u64,
        #[serde(default)]
        restart: HarnessRestartPolicy,
    },
    Http {
        url: String,
        #[serde(default)]
        headers: HashMap<String, HarnessMcpHeaderValue>,
        scope: HarnessMcpScope,
        tools: Option<Vec<String>>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum HarnessMcpScope {
    Global,
    Phases { phases: Vec<String> },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum HarnessMcpHeaderValue {
    Value { value: String },
    Env { env: String },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct HarnessMcpExports {
    pub enabled: bool,
    pub host: String,
}

impl Default for HarnessMcpExports {
    fn default() -> Self {
        Self {
            enabled: true,
            host: "127.0.0.1".into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, deny_unknown_fields)]
pub struct HarnessApprovalsConfig {
    pub controller: Option<HarnessApprovalController>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct HarnessApprovalController {
    pub implementation: HarnessImplementation,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct HarnessTraceConfig {
    pub enabled: bool,
    pub level: HarnessTraceLevel,
    pub content: HarnessTraceContent,
}

impl Default for HarnessTraceConfig {
    fn default() -> Self {
        Self {
            enabled: default_trace_enabled(),
            level: HarnessTraceLevel::Normal,
            content: HarnessTraceContent::Redacted,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HarnessTraceLevel {
    Minimal,
    Normal,
    Verbose,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HarnessTraceContent {
    None,
    Redacted,
    Full,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default, deny_unknown_fields)]
pub struct HarnessUiConfig {
    pub branding: HarnessBrandingConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct HarnessBrandingConfig {
    pub name: String,
    pub subtitle: Option<String>,
    pub accent: Option<String>,
}

impl Default for HarnessBrandingConfig {
    fn default() -> Self {
        Self {
            name: default_branding_name(),
            subtitle: None,
            accent: None,
        }
    }
}

pub fn load_harness_config(
    workspace_root: &Path,
    config_path_override: Option<&Path>,
) -> Result<ResolvedHarnessConfig> {
    let workspace_root = workspace_root
        .canonicalize()
        .with_context(|| format!("resolving workspace root {}", workspace_root.display()))?;
    let config_path = match config_path_override {
        Some(path) => path.to_path_buf(),
        None => workspace_root.join(DEFAULT_HARNESS_CONFIG_FILE),
    };

    if !config_path.exists() {
        if config_path_override.is_some() {
            bail!("Harness config file not found: {}", config_path.display());
        }
        let config = HarnessConfig {
            version: 1,
            ..HarnessConfig::default()
        };
        let state_dir = resolve_state_dir(&workspace_root, &config.runtime.state_dir)?;
        return Ok(ResolvedHarnessConfig {
            workspace_root,
            config_path: None,
            config,
            state_dir,
            state_dir_source: HarnessConfigSource::defaulted(),
        });
    }

    let bytes = fs::read(&config_path)
        .with_context(|| format!("reading Harness config {}", config_path.display()))?;
    let value: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("parsing Harness config {}", config_path.display()))?;
    validate_harness_config_value(&value)?;
    let config: HarnessConfig = serde_json::from_value(value)
        .with_context(|| format!("deserializing Harness config {}", config_path.display()))?;
    validate_harness_config_semantics(&workspace_root, &config)?;
    let state_dir = resolve_state_dir(&workspace_root, &config.runtime.state_dir)?;

    Ok(ResolvedHarnessConfig {
        workspace_root,
        config_path: Some(config_path.clone()),
        config,
        state_dir,
        state_dir_source: HarnessConfigSource::config_file(config_path),
    })
}

pub fn load_harness_config_with_overrides(
    workspace_root: &Path,
    config_path_override: Option<&Path>,
    overrides: &HarnessConfigOverrides,
) -> Result<ResolvedHarnessConfig> {
    let mut resolved = load_harness_config(workspace_root, config_path_override)?;
    if let Some(state_dir) = &overrides.state_dir {
        resolved.state_dir = resolve_state_dir(&resolved.workspace_root, state_dir)?;
        resolved.state_dir_source = HarnessConfigSource::cli_override();
    }
    Ok(resolved)
}

pub fn validate_harness_config_value(value: &Value) -> Result<()> {
    let schema_value: Value =
        serde_json::from_str(EMBEDDED_HARNESS_CONFIG_SCHEMA_JSON).context("parsing schema")?;
    let schema_static: &'static Value = Box::leak(Box::new(schema_value));
    let compiled = JSONSchema::options()
        .with_draft(Draft::Draft202012)
        .compile(schema_static)
        .context("compiling Harness config schema")?;

    if let Err(errors) = compiled.validate(value) {
        let messages = errors
            .map(|error| {
                format!(
                    "{} at instance {} vs schema {}",
                    error, error.instance_path, error.schema_path
                )
            })
            .collect::<Vec<_>>();
        bail!(
            "Harness config validation failed:\n- {}",
            messages.join("\n- ")
        );
    }

    Ok(())
}

pub fn validate_harness_config_semantics(
    workspace_root: &Path,
    config: &HarnessConfig,
) -> Result<()> {
    let workspace_root = workspace_root
        .canonicalize()
        .with_context(|| format!("resolving workspace root {}", workspace_root.display()))?;

    for provider_id in config.providers.models.keys() {
        if BUILT_IN_MODEL_PROVIDER_IDS.contains(&provider_id.as_str()) {
            bail!("built-in model provider `{provider_id}` cannot be redefined");
        }
    }

    if let Some(model) = &config.model
        && !BUILT_IN_MODEL_PROVIDER_IDS.contains(&model.provider.as_str())
        && !config.providers.models.contains_key(&model.provider)
    {
        bail!(
            "model.provider `{}` must reference a configured model provider",
            model.provider
        );
    }

    validate_implementation_map(
        &workspace_root,
        "providers.models",
        &config.providers.models,
    )?;
    validate_implementation_map(
        &workspace_root,
        "providers.embeddings",
        &config.providers.embeddings,
    )?;
    validate_implementation_map(
        &workspace_root,
        "hooks.implementations",
        &config.hooks.implementations,
    )?;
    validate_implementation_map(
        &workspace_root,
        "knowledge.runtimes",
        &config.knowledge.runtimes,
    )?;
    validate_implementation_map(&workspace_root, "memory.runtimes", &config.memory.runtimes)?;

    for binding in &config.hooks.bindings {
        if !config
            .hooks
            .implementations
            .contains_key(&binding.implementation)
        {
            bail!(
                "hook binding for `{:?}` references undefined implementation `{}`",
                binding.hook,
                binding.implementation
            );
        }
    }

    for (package, mapping) in &config.knowledge.packages {
        if !config.knowledge.runtimes.contains_key(&mapping.runtime) {
            bail!(
                "knowledge package mapping `{package}` references undefined runtime `{}`",
                mapping.runtime
            );
        }
    }

    let mut embedding_matches = HashSet::new();
    for item in &config.knowledge.embedding_matches {
        if !config
            .providers
            .embeddings
            .contains_key(&item.embedding_provider)
        {
            bail!(
                "knowledge embedding match references undefined embedding provider `{}`",
                item.embedding_provider
            );
        }
        if !embedding_matches.insert(item.r#match.clone()) {
            bail!("duplicate knowledge.embedding_matches tuple is ambiguous");
        }
    }

    for (package, mapping) in &config.memory.packages {
        if !config.memory.runtimes.contains_key(&mapping.runtime) {
            bail!(
                "memory package mapping `{package}` references undefined runtime `{}`",
                mapping.runtime
            );
        }
    }

    if let Some(semantic) = &config.memory.local.semantic
        && !config
            .providers
            .embeddings
            .contains_key(&semantic.embedding_provider)
    {
        bail!(
            "memory.local.semantic references undefined embedding provider `{}`",
            semantic.embedding_provider
        );
    }

    validate_state_dir(&config.runtime.state_dir)?;

    for (import_id, import) in &config.mcp.imports {
        validate_mcp_import(&workspace_root, import_id, import)?;
    }

    if let Some(controller) = &config.approvals.controller {
        validate_implementation(
            &workspace_root,
            "approvals.controller",
            &controller.implementation,
        )?;
    }

    Ok(())
}

fn validate_implementation_map(
    workspace_root: &Path,
    section: &str,
    implementations: &HashMap<String, HarnessImplementationEntry>,
) -> Result<()> {
    for (id, entry) in implementations {
        validate_implementation(
            workspace_root,
            &format!("{section}.{id}"),
            &entry.implementation,
        )?;
    }
    Ok(())
}

fn validate_implementation(
    workspace_root: &Path,
    label: &str,
    implementation: &HarnessImplementation,
) -> Result<()> {
    match implementation {
        HarnessImplementation::Process {
            command,
            cwd,
            startup_timeout_ms,
            request_timeout_ms,
            ..
        } => {
            if command.trim().is_empty() {
                bail!("{label} process command must not be empty");
            }
            if *startup_timeout_ms == 0 {
                bail!("{label} startup_timeout_ms must be positive");
            }
            if *request_timeout_ms == 0 {
                bail!("{label} request_timeout_ms must be positive");
            }
            if let Some(cwd) = cwd {
                resolve_existing_relative_dir(workspace_root, cwd)
                    .with_context(|| format!("validating {label} cwd"))?;
            }
        }
        HarnessImplementation::Host { request_timeout_ms } => {
            if *request_timeout_ms == 0 {
                bail!("{label} request_timeout_ms must be positive");
            }
        }
    }
    Ok(())
}

fn validate_mcp_import(
    workspace_root: &Path,
    import_id: &str,
    import: &HarnessMcpImport,
) -> Result<()> {
    match import {
        HarnessMcpImport::Stdio {
            command,
            cwd,
            scope,
            tools,
            startup_timeout_ms,
            request_timeout_ms,
            ..
        } => {
            if command.trim().is_empty() {
                bail!("mcp.imports.{import_id} command must not be empty");
            }
            if *startup_timeout_ms == 0 {
                bail!("mcp.imports.{import_id} startup_timeout_ms must be positive");
            }
            if *request_timeout_ms == 0 {
                bail!("mcp.imports.{import_id} request_timeout_ms must be positive");
            }
            if let Some(cwd) = cwd {
                resolve_existing_relative_dir(workspace_root, cwd)
                    .with_context(|| format!("validating mcp.imports.{import_id} cwd"))?;
            }
            validate_mcp_scope(import_id, scope)?;
            validate_optional_unique_list(import_id, "tools", tools.as_deref())?;
        }
        HarnessMcpImport::Http {
            url, scope, tools, ..
        } => {
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                bail!("mcp.imports.{import_id} url must be an absolute http(s) URL");
            }
            validate_mcp_scope(import_id, scope)?;
            validate_optional_unique_list(import_id, "tools", tools.as_deref())?;
        }
    }
    Ok(())
}

fn validate_mcp_scope(import_id: &str, scope: &HarnessMcpScope) -> Result<()> {
    if let HarnessMcpScope::Phases { phases } = scope {
        if phases.is_empty() {
            bail!("mcp.imports.{import_id} phases scope must list at least one phase");
        }
        validate_unique_list(import_id, "phases", phases)?;
    }
    Ok(())
}

fn validate_optional_unique_list(
    import_id: &str,
    field: &str,
    values: Option<&[String]>,
) -> Result<()> {
    if let Some(values) = values {
        validate_unique_list(import_id, field, values)?;
    }
    Ok(())
}

fn validate_unique_list(import_id: &str, field: &str, values: &[String]) -> Result<()> {
    let mut seen = HashSet::new();
    for value in values {
        if !seen.insert(value) {
            bail!("mcp.imports.{import_id}.{field} contains duplicate `{value}`");
        }
    }
    Ok(())
}

fn validate_state_dir<P: AsRef<Path>>(state_dir: P) -> Result<()> {
    let path = state_dir.as_ref();
    if path.is_absolute() {
        return Ok(());
    }
    let state_dir = path
        .to_str()
        .ok_or_else(|| anyhow!("path must be valid UTF-8"))?;
    parse_safe_workspace_relative_path(state_dir)
        .map(|_| ())
        .context("validating runtime.state_dir")
}

fn resolve_state_dir<P: AsRef<Path>>(workspace_root: &Path, state_dir: P) -> Result<PathBuf> {
    let path = state_dir.as_ref();
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        let state_dir = path
            .to_str()
            .ok_or_else(|| anyhow!("path must be valid UTF-8"))?;
        Ok(workspace_root.join(parse_safe_workspace_relative_path(state_dir)?))
    }
}

fn resolve_existing_relative_dir(root: &Path, relative: &str) -> Result<PathBuf> {
    let safe_rel = parse_safe_workspace_relative_path(relative)?;
    let candidate = root.join(safe_rel);
    let resolved = candidate
        .canonicalize()
        .with_context(|| format!("reading {}", candidate.display()))?;
    if !resolved.starts_with(root) {
        return Err(anyhow!(
            "resolved path escapes the workspace root: {}",
            candidate.display()
        ));
    }
    if !resolved.is_dir() {
        return Err(anyhow!("not a directory: {}", candidate.display()));
    }
    Ok(resolved)
}

fn parse_safe_workspace_relative_path(path: &str) -> Result<PathBuf> {
    if path.trim().is_empty() {
        bail!("path must not be empty");
    }

    let parsed = PathBuf::from(path);
    if parsed.is_absolute() {
        bail!("path must be workspace-relative");
    }

    for component in parsed.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir => bail!("path must not contain `..`"),
            Component::RootDir | Component::Prefix(_) => bail!("path must be workspace-relative"),
        }
    }

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_workspace(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("agentpm-harness-{label}-{nanos}"));
        fs::create_dir_all(dir.join("runtime")).unwrap();
        dir
    }

    fn write_config(workspace: &Path, value: Value) -> PathBuf {
        let path = workspace.join(DEFAULT_HARNESS_CONFIG_FILE);
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        path
    }

    fn assert_config_valid(value: Value) {
        validate_harness_config_value(&value).expect("expected config schema validation to pass");
        let config: HarnessConfig = serde_json::from_value(value).unwrap();
        let workspace = temp_workspace("valid");
        validate_harness_config_semantics(&workspace, &config)
            .expect("expected config semantics to pass");
        let _ = fs::remove_dir_all(workspace);
    }

    fn assert_config_invalid(value: Value, expected: &str) {
        let result = validate_harness_config_value(&value).and_then(|_| {
            let config: HarnessConfig = serde_json::from_value(value).unwrap();
            let workspace = temp_workspace("invalid");
            let result = validate_harness_config_semantics(&workspace, &config);
            let _ = fs::remove_dir_all(workspace);
            result
        });
        let err = result.expect_err("expected config validation to fail");
        assert!(
            format!("{err:#}").contains(expected),
            "expected error containing `{expected}`, got: {err:#}"
        );
    }

    fn complete_config() -> Value {
        json!({
            "version": 1,
            "model": {
                "provider": "openai",
                "model": "gpt-5",
                "options": { "temperature": 0.2 }
            },
            "providers": {
                "models": {
                    "company-model": {
                        "implementation": {
                            "type": "process",
                            "command": "python",
                            "args": ["runtime/company_model_provider.py"],
                            "cwd": ".",
                            "env": ["COMPANY_MODEL_API_KEY"],
                            "startup_timeout_ms": 15000,
                            "request_timeout_ms": 120000,
                            "restart": {
                                "max_attempts": 1,
                                "backoff_ms": 250
                            }
                        }
                    }
                },
                "embeddings": {
                    "example-4d": {
                        "implementation": {
                            "type": "host",
                            "request_timeout_ms": 120000
                        }
                    }
                }
            },
            "scopes": {
                "user": "user-42",
                "conversation": "incident-abc"
            },
            "runtime": {
                "state_dir": ".agentpm-state",
                "limits": {
                    "max_steps": 100,
                    "max_model_calls_per_phase": 24,
                    "max_tool_calls_per_phase": 16,
                    "max_actions_per_phase": 64,
                    "max_tool_call_repairs": 2,
                    "max_structured_output_repairs": 2,
                    "max_memory_operation_repairs": 2
                }
            },
            "hooks": {
                "implementations": {
                    "workspace-policy": {
                        "implementation": {
                            "type": "process",
                            "command": "python",
                            "args": ["runtime/hooks.py"],
                            "env": ["POLICY_SERVICE_TOKEN"]
                        }
                    }
                },
                "bindings": [
                    {
                        "hook": "before_model_request",
                        "implementation": "workspace-policy",
                        "failure_policy": "closed"
                    },
                    {
                        "hook": "before_tool_call",
                        "implementation": "workspace-policy",
                        "failure_policy": "continue"
                    }
                ]
            },
            "knowledge": {
                "runtimes": {
                    "pinecone-prod": {
                        "implementation": {
                            "type": "host"
                        }
                    }
                },
                "packages": {
                    "@zack/agentpm-docs": {
                        "runtime": "pinecone-prod"
                    }
                },
                "embedding_matches": [
                    {
                        "match": {
                            "provider": "bring-your-own",
                            "model": "example-normalized-4d",
                            "dimensions": 4,
                            "normalized": true
                        },
                        "embedding_provider": "example-4d"
                    }
                ]
            },
            "memory": {
                "local": {
                    "semantic": {
                        "embedding_provider": "example-4d",
                        "model": "example-normalized-4d",
                        "dimensions": 4
                    }
                },
                "runtimes": {
                    "postgres-prod": {
                        "implementation": {
                            "type": "process",
                            "command": "python",
                            "args": ["runtime/postgres_memory.py"],
                            "env": ["DATABASE_URL"]
                        }
                    }
                },
                "packages": {
                    "@zack/conversation-continuity": {
                        "runtime": "postgres-prod"
                    }
                }
            },
            "mcp": {
                "imports": {
                    "github": {
                        "transport": "stdio",
                        "command": "github-mcp-server",
                        "args": [],
                        "env": ["GITHUB_TOKEN"],
                        "scope": {
                            "mode": "phases",
                            "phases": ["assess", "execute"]
                        },
                        "tools": ["get_issue", "search_issues"]
                    },
                    "company-search": {
                        "transport": "http",
                        "url": "https://mcp.example.com/mcp",
                        "headers": {
                            "Authorization": {
                                "env": "COMPANY_MCP_AUTHORIZATION"
                            },
                            "X-Workspace": {
                                "value": "support"
                            }
                        },
                        "scope": {
                            "mode": "global"
                        }
                    }
                },
                "exports": {
                    "enabled": true,
                    "host": "127.0.0.1"
                }
            },
            "approvals": {
                "controller": {
                    "implementation": {
                        "type": "host"
                    }
                },
                "timeout_ms": 300000
            },
            "trace": {
                "enabled": true,
                "level": "normal",
                "content": "redacted"
            },
            "ui": {
                "branding": {
                    "name": "AgentPM Harness",
                    "subtitle": null,
                    "accent": null
                }
            }
        })
    }

    #[test]
    fn missing_harness_config_uses_version_one_defaults() {
        let workspace = temp_workspace("missing");
        let canonical_workspace = workspace.canonicalize().unwrap();
        let resolved = load_harness_config(&workspace, None).unwrap();

        assert!(resolved.config_path.is_none());
        assert_eq!(resolved.config.version, 1);
        assert_eq!(resolved.config.runtime.limits.max_steps, 100);
        assert_eq!(
            resolved.state_dir,
            canonical_workspace.join(".agentpm-state")
        );
        assert_eq!(
            resolved.state_dir_source.kind,
            HarnessConfigSourceKind::HarnessDefault
        );
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn minimal_harness_config_loads_with_defaults() {
        let workspace = temp_workspace("minimal");
        let path = write_config(&workspace, json!({ "version": 1 }));
        let canonical_path = path.canonicalize().unwrap();
        let resolved = load_harness_config(&workspace, None).unwrap();

        assert_eq!(resolved.config_path.as_ref(), Some(&canonical_path));
        assert_eq!(resolved.config.trace.level, HarnessTraceLevel::Normal);
        assert_eq!(resolved.config.ui.branding.name, "AgentPM Harness");
        assert_eq!(
            resolved.state_dir_source.kind,
            HarnessConfigSourceKind::ConfigFile
        );
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn complete_harness_config_validates_and_deserializes() {
        assert_config_valid(complete_config());
    }

    #[test]
    fn harness_config_rejects_unknown_fields_and_missing_version() {
        assert_config_invalid(json!({}), "version");
        assert_config_invalid(
            json!({ "version": 1, "extra": true }),
            "Additional properties",
        );
    }

    #[test]
    fn harness_config_rejects_invalid_shared_implementation_descriptors() {
        assert_config_invalid(
            json!({
                "version": 1,
                "providers": {
                    "models": {
                        "bad": {
                            "implementation": {
                                "type": "process",
                                "args": []
                            }
                        }
                    }
                }
            }),
            "oneOf",
        );
        assert_config_invalid(
            json!({
                "version": 1,
                "hooks": {
                    "implementations": {
                        "bad id": {
                            "implementation": {
                                "type": "host"
                            }
                        }
                    }
                }
            }),
            "does not match",
        );
    }

    #[test]
    fn harness_config_rejects_undefined_registry_references() {
        assert_config_invalid(
            json!({
                "version": 1,
                "model": {
                    "provider": "missing-model-provider",
                    "model": "example"
                }
            }),
            "missing-model-provider",
        );
        assert_config_invalid(
            json!({
                "version": 1,
                "hooks": {
                    "bindings": [
                        {
                            "hook": "before_model_request",
                            "implementation": "missing"
                        }
                    ]
                }
            }),
            "undefined implementation",
        );
        assert_config_invalid(
            json!({
                "version": 1,
                "knowledge": {
                    "packages": {
                        "@zack/docs": {
                            "runtime": "missing"
                        }
                    }
                }
            }),
            "undefined runtime",
        );
        assert_config_invalid(
            json!({
                "version": 1,
                "memory": {
                    "packages": {
                        "@zack/conversation-continuity@0.1.0": {
                            "runtime": "local"
                        }
                    }
                }
            }),
            "does not match",
        );
        assert_config_invalid(
            json!({
                "version": 1,
                "memory": {
                    "local": {
                        "semantic": {
                            "embedding_provider": "missing",
                            "model": "embed",
                            "dimensions": 4
                        }
                    }
                }
            }),
            "undefined embedding provider",
        );
    }

    #[test]
    fn harness_config_rejects_reserved_provider_ids_and_duplicate_embedding_matches() {
        assert_config_invalid(
            json!({
                "version": 1,
                "providers": {
                    "models": {
                        "openai": {
                            "implementation": {
                                "type": "host"
                            }
                        }
                    }
                }
            }),
            "cannot be redefined",
        );

        assert_config_invalid(
            json!({
                "version": 1,
                "providers": {
                    "embeddings": {
                        "embedder": {
                            "implementation": {
                                "type": "host"
                            }
                        }
                    }
                },
                "knowledge": {
                    "embedding_matches": [
                        {
                            "match": {
                                "provider": "custom",
                                "model": "embed",
                                "dimensions": 4,
                                "normalized": true
                            },
                            "embedding_provider": "embedder"
                        },
                        {
                            "match": {
                                "provider": "custom",
                                "model": "embed",
                                "dimensions": 4,
                                "normalized": true
                            },
                            "embedding_provider": "embedder"
                        }
                    ]
                }
            }),
            "duplicate knowledge.embedding_matches",
        );
    }

    #[test]
    fn harness_config_rejects_unsafe_paths_and_invalid_limits_or_branding() {
        assert_config_invalid(
            json!({
                "version": 1,
                "runtime": {
                    "state_dir": "../state"
                }
            }),
            "validating runtime.state_dir",
        );
        assert_config_invalid(
            json!({
                "version": 1,
                "runtime": {
                    "limits": {
                        "max_steps": 0
                    }
                }
            }),
            "minimum",
        );
        assert_config_invalid(
            json!({
                "version": 1,
                "ui": {
                    "branding": {
                        "accent": "#12345G"
                    }
                }
            }),
            "pattern",
        );
    }

    #[test]
    fn harness_config_rejects_malformed_mcp_imports_and_invalid_approval_controller() {
        assert_config_invalid(
            json!({
                "version": 1,
                "mcp": {
                    "imports": {
                        "github": {
                            "transport": "stdio",
                            "command": "github-mcp-server"
                        }
                    }
                }
            }),
            "oneOf",
        );
        assert_config_invalid(
            json!({
                "version": 1,
                "mcp": {
                    "imports": {
                        "github": {
                            "transport": "stdio",
                            "command": "github-mcp-server",
                            "scope": {
                                "mode": "phases",
                                "phases": ["Review_Phase"]
                            }
                        }
                    }
                }
            }),
            "oneOf",
        );
        assert_config_invalid(
            json!({
                "version": 1,
                "mcp": {
                    "imports": {
                        "company-search": {
                            "transport": "http",
                            "url": "/mcp",
                            "scope": {
                                "mode": "global"
                            }
                        }
                    }
                }
            }),
            "oneOf",
        );
        assert_config_invalid(
            json!({
                "version": 1,
                "approvals": {
                    "controller": {
                        "implementation": {
                            "type": "process"
                        }
                    }
                }
            }),
            "oneOf",
        );
    }
}
