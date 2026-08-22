use crate::harness_config::{
    HarnessConfigOverrides, HarnessImplementation, HarnessMcpImport, HarnessMcpScope,
    ResolvedHarnessConfig, load_harness_config_with_overrides,
};
use crate::manifest::{
    AgentBindingScope, AgentBindings, AgentManifest, KnowledgeManifest, MemoryManifest,
    MemoryOperation, PackageReference, SkillManifest, load_manifest_value,
    parse_knowledge_manifest, parse_loop_manifest, parse_memory_manifest, parse_skill_manifest,
};
use crate::prelude::*;
use crate::semver::types::{
    Lock, LockV2, LockedPackage, LockedRoot, PackageKind, split_package_ref,
};
use anyhow::bail;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessExecutionSurface {
    Headless,
    Machine,
    Tui,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum PreflightDiagnosticSeverity {
    Fatal,
    Warning,
    Suppressed,
    Pending,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreflightDiagnostic {
    pub severity: PreflightDiagnosticSeverity,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreflightReport {
    pub status: PreflightStatus,
    pub diagnostics: Vec<PreflightDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreflightStatus {
    Ready,
    ReadyWithWarnings,
    SelectionRequired,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedPackageInfo {
    pub key: String,
    pub kind: PackageKind,
    pub name: String,
    pub version: String,
    pub root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedAgentRoot {
    pub root_key: String,
    pub name: String,
    pub version: String,
    pub manifest_path: PathBuf,
    pub package_key: Option<String>,
    pub tools: Vec<String>,
    pub skills: Vec<String>,
    pub knowledge: Vec<String>,
    pub memory: Vec<String>,
    pub profiles: Vec<String>,
    pub loop_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsumerContextReadiness {
    pub state: CapabilityState,
    pub file: Option<String>,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StaticCapabilityCandidate {
    pub kind: String,
    pub identity: String,
    pub scope: String,
    pub source: String,
    pub state: CapabilityState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    Available,
    Pending,
    Unavailable,
    Suppressed,
    NotConfigured,
}

#[derive(Debug, Clone)]
pub struct ResolvedHarnessPlan {
    pub workspace_root: PathBuf,
    pub lock_path: PathBuf,
    pub state_dir: PathBuf,
    pub config: ResolvedHarnessConfig,
    pub selected_agent: Option<ResolvedAgentRoot>,
    pub loop_package: Option<ResolvedPackageInfo>,
    pub package_graph: BTreeMap<String, ResolvedPackageInfo>,
    pub runtime_scopes: BTreeMap<String, String>,
    pub consumer_context: ConsumerContextReadiness,
    pub capabilities: Vec<StaticCapabilityCandidate>,
    pub report: PreflightReport,
}

#[derive(Debug, Clone, Default)]
pub struct HarnessBootstrapOptions {
    pub agent_selector: Option<String>,
    pub config_path: Option<PathBuf>,
    pub state_dir_override: Option<PathBuf>,
    pub runtime_scopes: BTreeMap<String, String>,
    pub surface: HarnessExecutionSurface,
}

impl Default for HarnessExecutionSurface {
    fn default() -> Self {
        Self::Tui
    }
}

#[derive(Debug)]
struct AgentCandidate {
    root: ResolvedAgentRoot,
    manifest: Option<AgentManifest>,
}

struct PlanParts {
    workspace_root: PathBuf,
    lock_path: PathBuf,
    config: ResolvedHarnessConfig,
    selected_agent: Option<ResolvedAgentRoot>,
    loop_package: Option<ResolvedPackageInfo>,
    package_graph: BTreeMap<String, ResolvedPackageInfo>,
    runtime_scopes: BTreeMap<String, String>,
    consumer_context: ConsumerContextReadiness,
    capabilities: Vec<StaticCapabilityCandidate>,
    diagnostics: Vec<PreflightDiagnostic>,
}

pub fn resolve_harness_plan(
    workspace_root: &Path,
    options: &HarnessBootstrapOptions,
) -> Result<ResolvedHarnessPlan> {
    let workspace_root = workspace_root
        .canonicalize()
        .with_context(|| format!("resolving workspace root {}", workspace_root.display()))?;
    let config = load_harness_config_with_overrides(
        &workspace_root,
        options.config_path.as_deref(),
        &HarnessConfigOverrides {
            state_dir: options.state_dir_override.clone(),
        },
    )?;

    let lock_path = workspace_root.join("agent.lock");
    let mut diagnostics = Vec::new();
    let mut package_graph = BTreeMap::new();
    let mut capabilities = Vec::new();
    let mut selected_manifest = None;
    let mut loop_package = None;
    let mut consumer_context = ConsumerContextReadiness {
        state: CapabilityState::NotConfigured,
        file: None,
        path: None,
    };

    let Some(lock) = read_required_lock(&lock_path, &mut diagnostics)? else {
        return Ok(build_plan(PlanParts {
            workspace_root,
            lock_path,
            config,
            selected_agent: None,
            loop_package: None,
            package_graph,
            runtime_scopes: options.runtime_scopes.clone(),
            consumer_context,
            capabilities,
            diagnostics,
        }));
    };

    let Some(lock_v2) = lock_v2(lock, &mut diagnostics) else {
        return Ok(build_plan(PlanParts {
            workspace_root,
            lock_path,
            config,
            selected_agent: None,
            loop_package: None,
            package_graph,
            runtime_scopes: options.runtime_scopes.clone(),
            consumer_context,
            capabilities,
            diagnostics,
        }));
    };

    package_graph = package_graph_from_lock(&workspace_root, &lock_v2.packages)?;
    let mut candidates = load_agent_candidates(&workspace_root, &lock_v2, &mut diagnostics)?;
    let selected_agent = select_agent_candidate(
        &mut candidates,
        options.agent_selector.as_deref(),
        &mut diagnostics,
    );

    if let Some(agent) = &selected_agent {
        selected_manifest = candidates
            .into_iter()
            .find(|candidate| candidate.root.root_key == agent.root_key)
            .and_then(|candidate| candidate.manifest);
        loop_package = resolve_loop_package(agent, &package_graph, &mut diagnostics);
    }

    if let (Some(agent), Some(agent_manifest), Some(loop_info)) = (
        &selected_agent,
        selected_manifest.as_ref(),
        loop_package.as_ref(),
    ) {
        validate_selected_agent(
            &workspace_root,
            agent,
            agent_manifest,
            loop_info,
            &package_graph,
            &config,
            &options.runtime_scopes,
            options.surface,
            &mut consumer_context,
            &mut capabilities,
            &mut diagnostics,
        )?;
    }

    Ok(build_plan(PlanParts {
        workspace_root,
        lock_path,
        config,
        selected_agent,
        loop_package,
        package_graph,
        runtime_scopes: options.runtime_scopes.clone(),
        consumer_context,
        capabilities,
        diagnostics,
    }))
}

fn build_plan(parts: PlanParts) -> ResolvedHarnessPlan {
    let report = PreflightReport {
        status: preflight_status(&parts.diagnostics),
        diagnostics: parts.diagnostics,
    };
    ResolvedHarnessPlan {
        workspace_root: parts.workspace_root,
        lock_path: parts.lock_path,
        state_dir: parts.config.state_dir.clone(),
        config: parts.config,
        selected_agent: parts.selected_agent,
        loop_package: parts.loop_package,
        package_graph: parts.package_graph,
        runtime_scopes: parts.runtime_scopes,
        consumer_context: parts.consumer_context,
        capabilities: parts.capabilities,
        report,
    }
}

fn preflight_status(diagnostics: &[PreflightDiagnostic]) -> PreflightStatus {
    if diagnostics
        .iter()
        .any(|diag| diag.code == "agent_selection_required")
    {
        return PreflightStatus::SelectionRequired;
    }
    if diagnostics
        .iter()
        .any(|diag| diag.severity == PreflightDiagnosticSeverity::Fatal)
    {
        return PreflightStatus::Failed;
    }
    if diagnostics
        .iter()
        .any(|diag| diag.severity == PreflightDiagnosticSeverity::Warning)
    {
        return PreflightStatus::ReadyWithWarnings;
    }
    PreflightStatus::Ready
}

fn read_required_lock(
    lock_path: &Path,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) -> Result<Option<Lock>> {
    if !lock_path.exists() {
        push_diag(
            diagnostics,
            PreflightDiagnosticSeverity::Fatal,
            "missing_lock",
            "agent.lock is required before running the Harness; run `agentpm install` first.",
            Some("agent.lock"),
        );
        return Ok(None);
    }
    let bytes = fs::read(lock_path).with_context(|| format!("reading {}", lock_path.display()))?;
    match serde_json::from_slice(&bytes) {
        Ok(lock) => Ok(Some(lock)),
        Err(err) => {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Fatal,
                "invalid_lock",
                format!("agent.lock could not be parsed: {err}"),
                Some("agent.lock"),
            );
            Ok(None)
        }
    }
}

fn lock_v2(lock: Lock, diagnostics: &mut Vec<PreflightDiagnostic>) -> Option<LockV2> {
    match lock {
        Lock::V2(lock) if lock.lockfile_version >= 2 => Some(lock),
        Lock::V2(lock) => {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Fatal,
                "unsupported_lock_version",
                format!(
                    "agent.lock version {} cannot describe runnable Agent roots for Harness.",
                    lock.lockfile_version
                ),
                Some("agent.lock"),
            );
            None
        }
        Lock::V1(lock) => {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Fatal,
                "unsupported_lock_version",
                format!(
                    "agent.lock version {} cannot describe runnable Agent roots for Harness.",
                    lock.lockfile_version
                ),
                Some("agent.lock"),
            );
            None
        }
    }
}

fn package_graph_from_lock(
    workspace_root: &Path,
    packages: &BTreeMap<String, LockedPackage>,
) -> Result<BTreeMap<String, ResolvedPackageInfo>> {
    let mut graph = BTreeMap::new();
    for (key, package) in packages {
        graph.insert(
            key.clone(),
            ResolvedPackageInfo {
                key: key.clone(),
                kind: package.kind,
                name: package.name.clone(),
                version: package.version.clone(),
                root: installed_package_root(
                    workspace_root,
                    package.kind,
                    &package.name,
                    &package.version,
                )?,
            },
        );
    }
    Ok(graph)
}

fn load_agent_candidates(
    workspace_root: &Path,
    lock: &LockV2,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) -> Result<Vec<AgentCandidate>> {
    let mut candidates = Vec::new();
    for (root_key, root) in &lock.roots {
        let Some(agent_root) =
            agent_root_from_locked_root(workspace_root, root_key, root, &lock.packages)?
        else {
            continue;
        };

        let manifest = match load_agent_manifest(&agent_root.manifest_path) {
            Ok(manifest) => Some(manifest),
            Err(err) => {
                push_diag(
                    diagnostics,
                    PreflightDiagnosticSeverity::Fatal,
                    "missing_agent_manifest",
                    format!(
                        "resolved Agent manifest {} could not be loaded: {err}",
                        agent_root.manifest_path.display()
                    ),
                    Some(path_for_report(workspace_root, &agent_root.manifest_path)),
                );
                None
            }
        };

        candidates.push(AgentCandidate {
            root: agent_root,
            manifest,
        });
    }
    Ok(candidates)
}

fn agent_root_from_locked_root(
    workspace_root: &Path,
    root_key: &str,
    root: &LockedRoot,
    packages: &BTreeMap<String, LockedPackage>,
) -> Result<Option<ResolvedAgentRoot>> {
    if root_key.starts_with("local:agent") {
        let rel_manifest = root_key
            .strip_prefix("local:agent:")
            .unwrap_or("agent.json")
            .to_string();
        let manifest_path = workspace_root.join(rel_manifest);
        return Ok(Some(ResolvedAgentRoot {
            root_key: root_key.to_string(),
            name: root
                .name
                .clone()
                .unwrap_or_else(|| "local-agent".to_string()),
            version: root.version.clone().unwrap_or_else(|| "0.0.0".to_string()),
            manifest_path,
            package_key: None,
            tools: root.tools.clone(),
            skills: root.skills.clone(),
            knowledge: root.knowledge.clone(),
            memory: root.memory.clone(),
            profiles: root.profiles.clone(),
            loop_key: root.r#loop.clone().unwrap_or_default(),
        }));
    }

    let Some(package) = packages.get(root_key) else {
        return Ok(None);
    };
    if package.kind != PackageKind::Agent {
        return Ok(None);
    }
    Ok(Some(ResolvedAgentRoot {
        root_key: root_key.to_string(),
        name: package.name.clone(),
        version: package.version.clone(),
        manifest_path: installed_manifest_path(
            workspace_root,
            PackageKind::Agent,
            &package.name,
            &package.version,
        )?,
        package_key: Some(root_key.to_string()),
        tools: root.tools.clone(),
        skills: root.skills.clone(),
        knowledge: root.knowledge.clone(),
        memory: root.memory.clone(),
        profiles: root.profiles.clone(),
        loop_key: root.r#loop.clone().unwrap_or_default(),
    }))
}

fn load_agent_manifest(path: &Path) -> Result<AgentManifest> {
    let (value, _) = load_manifest_value(path)?;
    let manifest: AgentManifest =
        serde_json::from_value(value.clone()).context("parsing manifest into AgentManifest")?;
    if manifest.kind != "agent" {
        bail!(
            "expected kind=\"agent\" manifest (got kind=\"{}\")",
            manifest.kind
        );
    }
    Ok(manifest)
}

fn select_agent_candidate(
    candidates: &mut [AgentCandidate],
    selector: Option<&str>,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) -> Option<ResolvedAgentRoot> {
    let runnable: Vec<&AgentCandidate> = candidates
        .iter()
        .filter(|candidate| candidate.manifest.is_some())
        .filter(|candidate| !candidate.root.loop_key.is_empty())
        .collect();

    if let Some(selector) = selector {
        let parsed = parse_agent_selector(selector);
        let matches: Vec<&AgentCandidate> = runnable
            .into_iter()
            .filter(|candidate| selector_matches(candidate, &parsed))
            .collect();
        return match matches.len() {
            0 => {
                push_diag(
                    diagnostics,
                    PreflightDiagnosticSeverity::Fatal,
                    "agent_not_found",
                    format!("no runnable Agent in agent.lock/install state matches `{selector}`."),
                    None::<String>,
                );
                None
            }
            1 => Some(matches[0].root.clone()),
            _ => {
                push_diag(
                    diagnostics,
                    PreflightDiagnosticSeverity::Fatal,
                    "ambiguous_agent_selector",
                    format!("more than one runnable Agent matches `{selector}`."),
                    None::<String>,
                );
                None
            }
        };
    }

    if runnable.len() == 1 {
        return Some(runnable[0].root.clone());
    }

    if runnable.is_empty() {
        let loaded_agents = candidates
            .iter()
            .filter(|candidate| candidate.manifest.is_some())
            .count();
        let code = if loaded_agents > 0 {
            "agent_missing_loop"
        } else {
            "no_runnable_agent"
        };
        let message = if loaded_agents > 0 {
            "Agent packages without a resolved Loop are valid artifacts but are not runnable by Harness."
        } else {
            "agent.lock does not contain an installed or local runnable Agent root."
        };
        push_diag(
            diagnostics,
            PreflightDiagnosticSeverity::Fatal,
            code,
            message,
            Some("agent.lock"),
        );
        return None;
    }

    push_diag(
        diagnostics,
        PreflightDiagnosticSeverity::Fatal,
        "agent_selection_required",
        "multiple runnable Agents are available; pass `agentpm harness <agent>` to select one.",
        Some("agent.lock"),
    );
    None
}

#[derive(Debug)]
struct ParsedAgentSelector {
    name: String,
    version: Option<String>,
}

fn parse_agent_selector(selector: &str) -> ParsedAgentSelector {
    let raw = selector.strip_prefix("agent:").unwrap_or(selector);
    if let Some(last_at) = raw.rfind('@')
        && last_at > 0
    {
        return ParsedAgentSelector {
            name: raw[..last_at].to_string(),
            version: Some(raw[last_at + 1..].to_string()),
        };
    }
    ParsedAgentSelector {
        name: raw.to_string(),
        version: None,
    }
}

fn selector_matches(candidate: &AgentCandidate, selector: &ParsedAgentSelector) -> bool {
    if candidate.root.name != selector.name {
        return false;
    }
    selector
        .version
        .as_ref()
        .is_none_or(|version| candidate.root.version == *version)
}

fn resolve_loop_package(
    agent: &ResolvedAgentRoot,
    package_graph: &BTreeMap<String, ResolvedPackageInfo>,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) -> Option<ResolvedPackageInfo> {
    if let Some(package) = package_graph.get(&agent.loop_key)
        && package.kind == PackageKind::Loop
    {
        return Some(package.clone());
    }
    push_diag(
        diagnostics,
        PreflightDiagnosticSeverity::Fatal,
        "missing_loop_package",
        format!(
            "Agent `{}` references Loop `{}`, but that package is missing from agent.lock/install state.",
            agent.name, agent.loop_key
        ),
        Some("agent.lock"),
    );
    None
}

#[allow(clippy::too_many_arguments)]
fn validate_selected_agent(
    workspace_root: &Path,
    agent: &ResolvedAgentRoot,
    manifest: &AgentManifest,
    loop_info: &ResolvedPackageInfo,
    package_graph: &BTreeMap<String, ResolvedPackageInfo>,
    config: &ResolvedHarnessConfig,
    runtime_scopes: &BTreeMap<String, String>,
    surface: HarnessExecutionSurface,
    consumer_context: &mut ConsumerContextReadiness,
    capabilities: &mut Vec<StaticCapabilityCandidate>,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) -> Result<()> {
    let loop_manifest = load_loop_manifest(workspace_root, loop_info, diagnostics)?;
    let Some(loop_manifest) = loop_manifest else {
        return Ok(());
    };
    let loop_phase_ids =
        validate_loop_graph(workspace_root, loop_info, &loop_manifest, diagnostics);

    let Some(bindings) = manifest.bindings.as_ref() else {
        validate_config_mappings(config, agent, package_graph, diagnostics);
        validate_provider_readiness(config, surface, capabilities, diagnostics);
        return Ok(());
    };

    let mut tool_readiness = BTreeMap::new();
    validate_consumer_context(workspace_root, bindings, consumer_context, diagnostics);
    validate_phase_bindings(bindings, &loop_phase_ids, diagnostics);
    validate_bound_scopes(
        workspace_root,
        agent,
        bindings,
        &loop_phase_ids,
        package_graph,
        &mut tool_readiness,
        capabilities,
        diagnostics,
    )?;
    validate_mcp_exports(agent, bindings, package_graph, diagnostics);
    validate_config_mappings(config, agent, package_graph, diagnostics);
    validate_mcp_import_scopes(config, &loop_phase_ids, capabilities, diagnostics);
    validate_provider_readiness(config, surface, capabilities, diagnostics);
    validate_runtime_scopes(
        workspace_root,
        bindings,
        &loop_phase_ids,
        agent,
        package_graph,
        runtime_scopes,
        diagnostics,
    );
    Ok(())
}

fn load_loop_manifest(
    workspace_root: &Path,
    loop_info: &ResolvedPackageInfo,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) -> Result<Option<crate::manifest::LoopManifest>> {
    let path = loop_info.root.join("agent.json");
    match load_manifest_value(&path).and_then(|(value, _)| parse_loop_manifest(&value)) {
        Ok(manifest) => Ok(Some(manifest)),
        Err(err) => {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Fatal,
                "invalid_loop_manifest",
                format!(
                    "resolved Loop manifest {} could not be loaded: {err}",
                    path.display()
                ),
                Some(path_for_report(workspace_root, &path)),
            );
            Ok(None)
        }
    }
}

fn validate_loop_graph(
    workspace_root: &Path,
    loop_info: &ResolvedPackageInfo,
    manifest: &crate::manifest::LoopManifest,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) -> BTreeSet<String> {
    let mut phase_ids = BTreeSet::new();
    let mut phase_outcomes: HashMap<&str, HashSet<&str>> = HashMap::new();
    let mut transition_keys = HashSet::new();
    for phase in &manifest.r#loop.phases {
        phase_ids.insert(phase.id.clone());
        phase_outcomes.insert(
            &phase.id,
            phase
                .outcomes
                .iter()
                .map(|outcome| outcome.id.as_str())
                .collect(),
        );
    }
    let manifest_path = path_for_report(workspace_root, &loop_info.root.join("agent.json"));
    if !phase_ids.contains(&manifest.r#loop.entry_phase) {
        push_diag(
            diagnostics,
            PreflightDiagnosticSeverity::Fatal,
            "invalid_loop_entry_phase",
            format!(
                "Loop entry_phase `{}` does not reference a declared phase.",
                manifest.r#loop.entry_phase
            ),
            Some(format!("{manifest_path}#/loop/entry_phase")),
        );
    }
    for (idx, transition) in manifest.r#loop.transitions.iter().enumerate() {
        if !phase_ids.contains(&transition.from) {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Fatal,
                "invalid_loop_transition_from",
                format!(
                    "Loop transition {} references unknown from phase `{}`.",
                    idx, transition.from
                ),
                Some(format!("{manifest_path}#/loop/transitions/{idx}/from")),
            );
        }
        if !transition.to.starts_with('$') && !phase_ids.contains(&transition.to) {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Fatal,
                "invalid_loop_transition_to",
                format!(
                    "Loop transition {} references unknown target `{}`.",
                    idx, transition.to
                ),
                Some(format!("{manifest_path}#/loop/transitions/{idx}/to")),
            );
        }
        if let Some(outcomes) = phase_outcomes.get(transition.from.as_str())
            && !outcomes.is_empty()
            && !outcomes.contains(transition.on.as_str())
        {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Fatal,
                "invalid_loop_transition_outcome",
                format!(
                    "Loop transition {} references outcome `{}` not declared by phase `{}`.",
                    idx, transition.on, transition.from
                ),
                Some(format!("{manifest_path}#/loop/transitions/{idx}/on")),
            );
        }
        if !transition_keys.insert((&transition.from, &transition.on)) {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Fatal,
                "duplicate_loop_transition",
                format!(
                    "Loop has more than one transition for phase `{}` outcome `{}`.",
                    transition.from, transition.on
                ),
                Some(format!("{manifest_path}#/loop/transitions/{idx}")),
            );
        }
    }
    for (idx, checkpoint) in manifest.r#loop.checkpoints.iter().enumerate() {
        if !phase_ids.contains(&checkpoint.before_phase) {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Fatal,
                "invalid_loop_checkpoint_phase",
                format!(
                    "Loop checkpoint `{}` targets unknown phase `{}`.",
                    checkpoint.id, checkpoint.before_phase
                ),
                Some(format!(
                    "{manifest_path}#/loop/checkpoints/{idx}/before_phase"
                )),
            );
        }
        if !checkpoint.on_reject.starts_with('$') && !phase_ids.contains(&checkpoint.on_reject) {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Fatal,
                "invalid_loop_checkpoint_reject_target",
                format!(
                    "Loop checkpoint `{}` has unknown on_reject target `{}`.",
                    checkpoint.id, checkpoint.on_reject
                ),
                Some(format!("{manifest_path}#/loop/checkpoints/{idx}/on_reject")),
            );
        }
    }
    phase_ids
}

fn validate_phase_bindings(
    bindings: &AgentBindings,
    loop_phase_ids: &BTreeSet<String>,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) {
    for phase in bindings.phases.keys() {
        if !loop_phase_ids.contains(phase) {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Warning,
                "unknown_phase_binding",
                format!(
                    "Agent binding phase `{phase}` does not exist in the resolved Loop and will be ignored."
                ),
                Some(format!("/bindings/phases/{phase}")),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_bound_scopes(
    workspace_root: &Path,
    agent: &ResolvedAgentRoot,
    bindings: &AgentBindings,
    loop_phase_ids: &BTreeSet<String>,
    package_graph: &BTreeMap<String, ResolvedPackageInfo>,
    tool_readiness: &mut BTreeMap<String, CapabilityState>,
    capabilities: &mut Vec<StaticCapabilityCandidate>,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) -> Result<()> {
    if let Some(global) = &bindings.global {
        validate_binding_scope(
            workspace_root,
            agent,
            "global",
            global,
            package_graph,
            tool_readiness,
            capabilities,
            diagnostics,
        )?;
    }
    let mut phases: Vec<_> = bindings.phases.iter().collect();
    phases.sort_by(|left, right| left.0.cmp(right.0));
    for (phase, scope) in phases {
        if !loop_phase_ids.contains(phase) {
            continue;
        }
        validate_binding_scope(
            workspace_root,
            agent,
            &format!("phase:{phase}"),
            scope,
            package_graph,
            tool_readiness,
            capabilities,
            diagnostics,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_binding_scope(
    workspace_root: &Path,
    agent: &ResolvedAgentRoot,
    scope_label: &str,
    scope: &AgentBindingScope,
    package_graph: &BTreeMap<String, ResolvedPackageInfo>,
    tool_readiness: &mut BTreeMap<String, CapabilityState>,
    capabilities: &mut Vec<StaticCapabilityCandidate>,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) -> Result<()> {
    let declared_tools = package_names(package_graph, &agent.tools);
    let declared_skills = package_names(package_graph, &agent.skills);
    let declared_knowledge = package_names(package_graph, &agent.knowledge);
    let declared_memory = package_names(package_graph, &agent.memory);
    let declared_profiles = package_names(package_graph, &agent.profiles);

    let direct_tools = validate_tool_bindings(
        workspace_root,
        &scope.tools,
        &declared_tools,
        package_graph,
        tool_readiness,
        scope_label,
        capabilities,
        diagnostics,
    );
    let active_skills = validate_string_bindings(
        &scope.skills,
        &declared_skills,
        "skill",
        scope_label,
        capabilities,
        diagnostics,
    );
    let active_knowledge = validate_string_bindings(
        &scope.knowledge,
        &declared_knowledge,
        "knowledge",
        scope_label,
        capabilities,
        diagnostics,
    );
    validate_knowledge_bindings(
        workspace_root,
        &active_knowledge,
        package_graph,
        diagnostics,
    );
    validate_string_bindings(
        &scope.profiles,
        &declared_profiles,
        "profile",
        scope_label,
        capabilities,
        diagnostics,
    );
    validate_memory_bindings(
        workspace_root,
        &scope.memory,
        &declared_memory,
        package_graph,
        scope_label,
        capabilities,
        diagnostics,
    );
    validate_skill_inheritance(
        workspace_root,
        &active_skills,
        &direct_tools,
        package_graph,
        tool_readiness,
        scope_label,
        capabilities,
        diagnostics,
    )?;
    Ok(())
}

fn validate_string_bindings(
    bindings: &[String],
    declared_names: &BTreeSet<String>,
    kind: &str,
    scope_label: &str,
    capabilities: &mut Vec<StaticCapabilityCandidate>,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) -> BTreeSet<String> {
    let mut active = BTreeSet::new();
    for binding in bindings {
        let name = package_identity(binding);
        if declared_names.contains(&name) {
            active.insert(name.clone());
            capabilities.push(StaticCapabilityCandidate {
                kind: kind.to_string(),
                identity: name,
                scope: scope_label.to_string(),
                source: "agent_binding".to_string(),
                state: CapabilityState::Available,
            });
        } else {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Suppressed,
                "missing_bound_package",
                format!(
                    "Bound {kind} `{binding}` in {scope_label} is not in the Agent dependency graph and will not be surfaced."
                ),
                Some(format!("/bindings/{scope_label}")),
            );
        }
    }
    active
}

#[allow(clippy::too_many_arguments)]
fn validate_tool_bindings(
    workspace_root: &Path,
    bindings: &[String],
    declared_names: &BTreeSet<String>,
    package_graph: &BTreeMap<String, ResolvedPackageInfo>,
    tool_readiness: &mut BTreeMap<String, CapabilityState>,
    scope_label: &str,
    capabilities: &mut Vec<StaticCapabilityCandidate>,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) -> BTreeSet<String> {
    let mut active = BTreeSet::new();
    for binding in bindings {
        let name = package_identity(binding);
        if declared_names.contains(&name) {
            active.insert(name.clone());
            let state = cached_tool_readiness_state(
                workspace_root,
                package_graph,
                &name,
                tool_readiness,
                diagnostics,
            );
            capabilities.push(StaticCapabilityCandidate {
                kind: "tool".to_string(),
                identity: name,
                scope: scope_label.to_string(),
                source: "agent_binding".to_string(),
                state,
            });
        } else {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Suppressed,
                "missing_bound_package",
                format!(
                    "Bound tool `{binding}` in {scope_label} is not in the Agent dependency graph and will not be surfaced."
                ),
                Some(format!("/bindings/{scope_label}")),
            );
        }
    }
    active
}

fn validate_memory_bindings(
    workspace_root: &Path,
    bindings: &[crate::manifest::AgentMemoryBinding],
    declared_names: &BTreeSet<String>,
    package_graph: &BTreeMap<String, ResolvedPackageInfo>,
    scope_label: &str,
    capabilities: &mut Vec<StaticCapabilityCandidate>,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) {
    for binding in bindings {
        let name = package_identity(&binding.package);
        if !declared_names.contains(&name) {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Suppressed,
                "missing_bound_package",
                format!(
                    "Bound memory package `{}` in {scope_label} is not in the Agent dependency graph and will not be surfaced.",
                    binding.package
                ),
                Some(format!("/bindings/{scope_label}/memory")),
            );
            continue;
        }
        let Some(package) = package_by_name(package_graph, PackageKind::Memory, &name) else {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Suppressed,
                "missing_installed_memory",
                format!("Memory package `{name}` is declared but missing from installed state."),
                Some("agent.lock"),
            );
            continue;
        };
        let manifest = load_memory_manifest(workspace_root, package, diagnostics);
        if let Some(manifest) = manifest.as_ref() {
            for space in &binding.spaces {
                if !manifest.memory.spaces.contains_key(space) {
                    push_diag(
                        diagnostics,
                        PreflightDiagnosticSeverity::Suppressed,
                        "bad_memory_selector",
                        format!(
                            "Memory binding `{}` selects unknown space `{space}`.",
                            binding.package
                        ),
                        Some(format!("/bindings/{scope_label}/memory")),
                    );
                }
            }
            for operation in &binding.operations {
                if !manifest.memory.operations.contains_key(operation) {
                    push_diag(
                        diagnostics,
                        PreflightDiagnosticSeverity::Suppressed,
                        "bad_memory_selector",
                        format!(
                            "Memory binding `{}` selects unknown operation `{operation}`.",
                            binding.package
                        ),
                        Some(format!("/bindings/{scope_label}/memory")),
                    );
                }
            }
        }
        validate_memory_generated_metadata(workspace_root, package, diagnostics);
        capabilities.push(StaticCapabilityCandidate {
            kind: "memory".to_string(),
            identity: name,
            scope: scope_label.to_string(),
            source: "agent_binding".to_string(),
            state: CapabilityState::Pending,
        });
    }
}

fn validate_knowledge_bindings(
    workspace_root: &Path,
    active_knowledge: &BTreeSet<String>,
    package_graph: &BTreeMap<String, ResolvedPackageInfo>,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) {
    for knowledge_name in active_knowledge {
        let Some(package) = package_by_name(package_graph, PackageKind::Knowledge, knowledge_name)
        else {
            continue;
        };
        if let Some(manifest) = load_knowledge_manifest(workspace_root, package, diagnostics) {
            validate_knowledge_generated_metadata(workspace_root, package, &manifest, diagnostics);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_skill_inheritance(
    workspace_root: &Path,
    active_skills: &BTreeSet<String>,
    direct_tools: &BTreeSet<String>,
    package_graph: &BTreeMap<String, ResolvedPackageInfo>,
    tool_readiness: &mut BTreeMap<String, CapabilityState>,
    scope_label: &str,
    capabilities: &mut Vec<StaticCapabilityCandidate>,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) -> Result<()> {
    for skill_name in active_skills {
        let Some(skill_package) = package_by_name(package_graph, PackageKind::Skill, skill_name)
        else {
            continue;
        };
        let Some(skill_manifest) = load_skill_manifest(workspace_root, skill_package, diagnostics)?
        else {
            continue;
        };
        for tool_ref in &skill_manifest.tools {
            let (tool_name, _) = package_ref_parts(tool_ref);
            if direct_tools.contains(&tool_name) {
                push_diag(
                    diagnostics,
                    PreflightDiagnosticSeverity::Warning,
                    "duplicate_tool_candidate",
                    format!(
                        "Tool `{tool_name}` is directly bound and inherited from Skill `{skill_name}` in {scope_label}; the duplicate candidate is de-duped."
                    ),
                    Some(format!("/bindings/{scope_label}/tools")),
                );
                continue;
            }
            let state = if package_by_name(package_graph, PackageKind::Tool, &tool_name).is_some() {
                cached_tool_readiness_state(
                    workspace_root,
                    package_graph,
                    &tool_name,
                    tool_readiness,
                    diagnostics,
                )
            } else {
                push_diag(
                    diagnostics,
                    PreflightDiagnosticSeverity::Suppressed,
                    "missing_inherited_tool",
                    format!(
                        "Skill `{skill_name}` inherits Tool `{tool_name}`, but that Tool is missing from installed state."
                    ),
                    Some("agent.lock"),
                );
                CapabilityState::Suppressed
            };
            capabilities.push(StaticCapabilityCandidate {
                kind: "tool".to_string(),
                identity: tool_name,
                scope: scope_label.to_string(),
                source: format!("skill:{skill_name}"),
                state,
            });
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
struct ToolReadinessManifest {
    entrypoint: crate::manifest::Entrypoint,
    #[serde(default)]
    runtime: Option<ToolRuntimeDecl>,
    #[serde(default)]
    environment: Option<ToolEnvironmentDecl>,
}

#[derive(Debug, Clone, Deserialize)]
struct ToolRuntimeDecl {
    #[serde(rename = "type")]
    runtime_type: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ToolEnvironmentDecl {
    #[serde(default)]
    vars: HashMap<String, ToolEnvVarDecl>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ToolEnvVarDecl {
    #[serde(default)]
    required: bool,
    #[serde(default)]
    default: Option<String>,
}

fn tool_readiness_state(
    workspace_root: &Path,
    package_graph: &BTreeMap<String, ResolvedPackageInfo>,
    tool_name: &str,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) -> CapabilityState {
    let Some(package) = package_by_name(package_graph, PackageKind::Tool, tool_name) else {
        return CapabilityState::Suppressed;
    };
    let manifest_path = package.root.join("agent.json");
    let manifest = match load_manifest_value(&manifest_path).and_then(|(value, _)| {
        serde_json::from_value::<ToolReadinessManifest>(value).map_err(Into::into)
    }) {
        Ok(manifest) => manifest,
        Err(err) => {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Warning,
                "tool_manifest_unavailable",
                format!(
                    "Tool `{tool_name}` is installed but its manifest could not be inspected: {err}"
                ),
                Some(path_for_report(workspace_root, &manifest_path)),
            );
            return CapabilityState::Unavailable;
        }
    };

    let mut state = CapabilityState::Available;
    if let Some(runtime) = &manifest.runtime
        && !tool_runtime_ready(tool_name, &manifest.entrypoint, runtime, diagnostics)
    {
        state = CapabilityState::Unavailable;
    }
    validate_tool_required_env(tool_name, &manifest, diagnostics);
    state
}

fn cached_tool_readiness_state(
    workspace_root: &Path,
    package_graph: &BTreeMap<String, ResolvedPackageInfo>,
    tool_name: &str,
    cache: &mut BTreeMap<String, CapabilityState>,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) -> CapabilityState {
    if let Some(state) = cache.get(tool_name) {
        return *state;
    }
    let state = tool_readiness_state(workspace_root, package_graph, tool_name, diagnostics);
    cache.insert(tool_name.to_string(), state);
    state
}

fn tool_runtime_ready(
    tool_name: &str,
    entrypoint: &crate::manifest::Entrypoint,
    runtime: &ToolRuntimeDecl,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) -> bool {
    let Some(expected) = interpreter_family(&runtime.runtime_type) else {
        push_diag(
            diagnostics,
            PreflightDiagnosticSeverity::Warning,
            "tool_runtime_unsupported",
            format!(
                "Tool `{tool_name}` declares unsupported runtime `{}` and will not be exposed.",
                runtime.runtime_type
            ),
            None::<String>,
        );
        return false;
    };
    let Some(requested) = interpreter_family(&entrypoint.command)
        .or_else(|| basename_interpreter_family(&entrypoint.command))
    else {
        push_diag(
            diagnostics,
            PreflightDiagnosticSeverity::Warning,
            "tool_runtime_unsupported",
            format!(
                "Tool `{tool_name}` uses unsupported entrypoint command `{}` and will not be exposed.",
                entrypoint.command
            ),
            None::<String>,
        );
        return false;
    };
    if expected != requested {
        push_diag(
            diagnostics,
            PreflightDiagnosticSeverity::Warning,
            "tool_runtime_mismatch",
            format!(
                "Tool `{tool_name}` declares runtime `{}` but entrypoint command `{}` resolves to `{requested}` and will not be exposed.",
                runtime.runtime_type, entrypoint.command
            ),
            None::<String>,
        );
        return false;
    }

    let command = interpreter_override(expected)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| entrypoint.command.clone());
    if !command_available(&command) {
        push_diag(
            diagnostics,
            PreflightDiagnosticSeverity::Warning,
            "tool_runtime_unavailable",
            format!(
                "Tool `{tool_name}` requires `{command}`, but that command is not available on PATH and the Tool will not be exposed."
            ),
            None::<String>,
        );
        return false;
    }
    true
}

fn validate_tool_required_env(
    tool_name: &str,
    manifest: &ToolReadinessManifest,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) {
    let Some(environment) = &manifest.environment else {
        return;
    };
    let missing: Vec<_> = environment
        .vars
        .iter()
        .filter(|(key, decl)| {
            decl.required
                && decl.default.is_none()
                && !manifest.entrypoint.env.contains_key(*key)
                && std::env::var_os(*key).is_none()
        })
        .map(|(key, _)| key.clone())
        .collect();
    if !missing.is_empty() {
        push_diag(
            diagnostics,
            PreflightDiagnosticSeverity::Warning,
            "tool_required_environment_missing",
            format!(
                "Tool `{tool_name}` requires environment variables that are not set for this Harness process: {}. Actual `agentpm run` invocation remains authoritative.",
                missing.join(", ")
            ),
            None::<String>,
        );
    }
}

fn interpreter_override(family: String) -> Option<String> {
    match family.as_str() {
        "node" => std::env::var("AGENTPM_NODE").ok(),
        "python" => std::env::var("AGENTPM_PYTHON").ok(),
        _ => None,
    }
}

fn interpreter_family(command: &str) -> Option<String> {
    match canonical_interpreter(command).as_str() {
        "node" | "nodejs" => Some("node".to_string()),
        "python" | "python3" => Some("python".to_string()),
        _ => None,
    }
}

fn basename_interpreter_family(command: &str) -> Option<String> {
    Path::new(command)
        .file_name()
        .and_then(OsStr::to_str)
        .and_then(interpreter_family)
}

fn canonical_interpreter(command: &str) -> String {
    let lowered = command.to_ascii_lowercase();
    lowered
        .strip_suffix(".exe")
        .or_else(|| lowered.strip_suffix(".cmd"))
        .or_else(|| lowered.strip_suffix(".bat"))
        .unwrap_or(&lowered)
        .to_string()
}

fn command_available(command: &str) -> bool {
    let path = Path::new(command);
    if path.is_absolute() || command.contains(std::path::MAIN_SEPARATOR) {
        return executable_file(path);
    }

    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .any(|dir| executable_file(&dir.join(command)))
}

#[cfg(unix)]
fn executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.is_file()
        && path
            .metadata()
            .is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn executable_file(path: &Path) -> bool {
    path.is_file()
}

fn validate_mcp_exports(
    agent: &ResolvedAgentRoot,
    bindings: &AgentBindings,
    package_graph: &BTreeMap<String, ResolvedPackageInfo>,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) {
    let declared_tools = package_names(package_graph, &agent.tools);
    for export in &bindings.mcp {
        for tool in &export.tools {
            let name = package_identity(tool);
            if !declared_tools.contains(&name) {
                push_diag(
                    diagnostics,
                    PreflightDiagnosticSeverity::Suppressed,
                    "invalid_mcp_export_tool",
                    format!(
                        "MCP export `{}` references Tool `{tool}` that is not a top-level Agent Tool dependency.",
                        export.id
                    ),
                    Some("/bindings/mcp"),
                );
            }
        }
    }
}

fn validate_config_mappings(
    config: &ResolvedHarnessConfig,
    agent: &ResolvedAgentRoot,
    package_graph: &BTreeMap<String, ResolvedPackageInfo>,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) {
    let agent_knowledge = package_names(package_graph, &agent.knowledge);
    for package in config.config.knowledge.packages.keys() {
        if !agent_knowledge.contains(package) {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Warning,
                "irrelevant_knowledge_runtime_mapping",
                format!(
                    "knowledge.packages maps `{package}`, but that package is not relevant to the selected Agent."
                ),
                Some("/knowledge/packages"),
            );
        }
    }
    let agent_memory = package_names(package_graph, &agent.memory);
    for package in config.config.memory.packages.keys() {
        if !agent_memory.contains(package) {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Warning,
                "irrelevant_memory_runtime_mapping",
                format!(
                    "memory.packages maps `{package}`, but that package is not relevant to the selected Agent."
                ),
                Some("/memory/packages"),
            );
        }
    }
}

fn validate_mcp_import_scopes(
    config: &ResolvedHarnessConfig,
    loop_phase_ids: &BTreeSet<String>,
    capabilities: &mut Vec<StaticCapabilityCandidate>,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) {
    for (id, import) in &config.config.mcp.imports {
        if let HarnessMcpScope::Phases { phases } = mcp_import_scope(import) {
            for phase in phases {
                if !loop_phase_ids.contains(phase) {
                    push_diag(
                        diagnostics,
                        PreflightDiagnosticSeverity::Warning,
                        "unknown_mcp_import_phase",
                        format!(
                            "MCP import `{id}` is scoped to unknown Loop phase `{phase}` and will be ignored there."
                        ),
                        Some(format!("/mcp/imports/{id}/scope/phases")),
                    );
                }
            }
        }
        capabilities.push(StaticCapabilityCandidate {
            kind: "mcp_import".to_string(),
            identity: id.clone(),
            scope: mcp_scope_label(mcp_import_scope(import)),
            source: "harness_config".to_string(),
            state: CapabilityState::Pending,
        });
    }
}

fn validate_provider_readiness(
    config: &ResolvedHarnessConfig,
    surface: HarnessExecutionSurface,
    capabilities: &mut Vec<StaticCapabilityCandidate>,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) {
    if let Some(model) = &config.config.model {
        capabilities.push(StaticCapabilityCandidate {
            kind: "model_provider".to_string(),
            identity: format!("{}/{}", model.provider, model.model),
            scope: "session".to_string(),
            source: "harness_config".to_string(),
            state: CapabilityState::Pending,
        });
    }
    for (id, entry) in &config.config.providers.models {
        implementation_capability(
            id,
            "model_provider",
            &entry.implementation,
            surface,
            capabilities,
            diagnostics,
        );
    }
    for (id, entry) in &config.config.hooks.implementations {
        implementation_capability(
            id,
            "hook",
            &entry.implementation,
            surface,
            capabilities,
            diagnostics,
        );
    }
    if let Some(controller) = &config.config.approvals.controller {
        implementation_capability(
            "approval_controller",
            "approval_controller",
            &controller.implementation,
            surface,
            capabilities,
            diagnostics,
        );
    }
}

fn implementation_capability(
    id: &str,
    kind: &str,
    implementation: &HarnessImplementation,
    surface: HarnessExecutionSurface,
    capabilities: &mut Vec<StaticCapabilityCandidate>,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) {
    let state = match implementation {
        HarnessImplementation::Host { .. } => {
            if surface == HarnessExecutionSurface::Machine {
                CapabilityState::Pending
            } else {
                push_diag(
                    diagnostics,
                    PreflightDiagnosticSeverity::Suppressed,
                    "host_implementation_unavailable",
                    format!(
                        "Host implementation `{id}` requires a machine/SDK host and is unavailable for this execution surface."
                    ),
                    None::<String>,
                );
                CapabilityState::Unavailable
            }
        }
        HarnessImplementation::Process { .. } => CapabilityState::Pending,
    };
    capabilities.push(StaticCapabilityCandidate {
        kind: kind.to_string(),
        identity: id.to_string(),
        scope: "session".to_string(),
        source: "harness_config".to_string(),
        state,
    });
}

fn validate_consumer_context(
    workspace_root: &Path,
    bindings: &AgentBindings,
    readiness: &mut ConsumerContextReadiness,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) {
    let Some(context) = &bindings.consumer_context else {
        return;
    };
    readiness.file = Some(context.file.clone());
    match resolve_workspace_file(workspace_root, &context.file) {
        Ok(path) if path.is_file() => {
            readiness.state = CapabilityState::Available;
            readiness.path = Some(path);
        }
        Ok(path) => {
            readiness.state = CapabilityState::Unavailable;
            readiness.path = Some(path);
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Warning,
                "missing_consumer_context",
                format!(
                    "Consumer context `{}` is not present; Harness will continue without injecting it.",
                    context.file
                ),
                Some("/bindings/consumer_context/file"),
            );
        }
        Err(err) => {
            readiness.state = CapabilityState::Unavailable;
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Warning,
                "unsafe_consumer_context",
                format!("Consumer context path `{}` is invalid: {err}", context.file),
                Some("/bindings/consumer_context/file"),
            );
        }
    }
}

fn validate_runtime_scopes(
    workspace_root: &Path,
    bindings: &AgentBindings,
    loop_phase_ids: &BTreeSet<String>,
    agent: &ResolvedAgentRoot,
    package_graph: &BTreeMap<String, ResolvedPackageInfo>,
    runtime_scopes: &BTreeMap<String, String>,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) {
    let mut required = BTreeSet::new();
    for scope in binding_scopes(bindings, loop_phase_ids) {
        for binding in &scope.memory {
            let name = package_identity(&binding.package);
            let Some(package) = package_by_name(package_graph, PackageKind::Memory, &name) else {
                continue;
            };
            if !agent.memory.iter().any(|key| {
                package_graph
                    .get(key)
                    .is_some_and(|package| package.name == name)
            }) {
                continue;
            }
            let Some(manifest) = load_memory_manifest(workspace_root, package, diagnostics) else {
                continue;
            };
            for space in selected_memory_spaces(binding, &manifest) {
                if let Some(space) = manifest.memory.spaces.get(&space) {
                    required.extend(space.scope.iter().cloned());
                }
            }
        }
    }
    for scope_key in required {
        if !runtime_scopes.contains_key(&scope_key) {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Warning,
                "unresolved_runtime_scope",
                format!(
                    "Memory runtime scope `{scope_key}` is required by active Memory bindings but has no configured value."
                ),
                None::<String>,
            );
        }
    }
}

fn binding_scopes<'a>(
    bindings: &'a AgentBindings,
    loop_phase_ids: &BTreeSet<String>,
) -> Vec<&'a AgentBindingScope> {
    let mut scopes = Vec::new();
    if let Some(global) = &bindings.global {
        scopes.push(global);
    }
    scopes.extend(
        bindings
            .phases
            .iter()
            .filter(|(phase, _)| loop_phase_ids.contains(*phase))
            .map(|(_, scope)| scope),
    );
    scopes
}

fn selected_memory_spaces(
    binding: &crate::manifest::AgentMemoryBinding,
    manifest: &MemoryManifest,
) -> BTreeSet<String> {
    let mut spaces = BTreeSet::new();
    spaces.extend(binding.spaces.iter().cloned());
    for operation in &binding.operations {
        if let Some(operation) = manifest.memory.operations.get(operation) {
            collect_operation_spaces(operation, &mut spaces);
        }
    }
    spaces
}

fn collect_operation_spaces(operation: &MemoryOperation, spaces: &mut BTreeSet<String>) {
    match operation {
        MemoryOperation::Consolidate { inputs, output, .. }
        | MemoryOperation::Transform { inputs, output, .. } => {
            spaces.extend(inputs.iter().map(|input| input.space.clone()));
            spaces.insert(output.space.clone());
        }
        MemoryOperation::Delete { targets, .. } => {
            spaces.extend(targets.iter().map(|target| target.space.clone()));
        }
    }
}

fn validate_memory_generated_metadata(
    workspace_root: &Path,
    package: &ResolvedPackageInfo,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) {
    let build_path = package.root.join("memory/build.json");
    read_json_required(
        workspace_root,
        &build_path,
        "missing_memory_build_metadata",
        "corrupt_memory_build_metadata",
        diagnostics,
    );
    let index_path = package.root.join("memory/contracts/index.json");
    let Some(index) = read_json_required(
        workspace_root,
        &index_path,
        "missing_memory_contract_index",
        "corrupt_memory_contract_index",
        diagnostics,
    ) else {
        return;
    };
    if let Some(contracts) = index.get("contracts").and_then(Value::as_array) {
        for contract in contracts {
            if let Some(path) = contract.get("path").and_then(Value::as_str) {
                match resolve_workspace_file(&package.root, path) {
                    Ok(path) if path.is_file() => {}
                    Ok(_) => push_diag(
                        diagnostics,
                        PreflightDiagnosticSeverity::Warning,
                        "missing_memory_contract",
                        format!("Memory contract `{path}` is missing."),
                        Some(path_for_report(workspace_root, &package.root.join(path))),
                    ),
                    Err(err) => push_diag(
                        diagnostics,
                        PreflightDiagnosticSeverity::Warning,
                        "unsafe_memory_contract_path",
                        format!("Memory contract path `{path}` is unsafe: {err}"),
                        Some(path_for_report(workspace_root, &index_path)),
                    ),
                }
            }
        }
    }
}

fn validate_knowledge_generated_metadata(
    workspace_root: &Path,
    package: &ResolvedPackageInfo,
    manifest: &KnowledgeManifest,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) {
    for document in &manifest.knowledge.documents {
        if !package.root.join(&document.path).is_file() {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Warning,
                "missing_knowledge_document",
                format!("Knowledge document `{}` is missing.", document.path),
                Some(path_for_report(
                    workspace_root,
                    &package.root.join(&document.path),
                )),
            );
        }
    }
    if let Some(corpus) = &manifest.knowledge.corpus {
        validate_optional_knowledge_file(
            workspace_root,
            package,
            corpus.chunks_path.as_deref(),
            "missing_knowledge_chunks",
            "Knowledge chunks file",
            diagnostics,
        );
    }
    if let Some(embedding) = &manifest.knowledge.embedding
        && !package.root.join(&embedding.vectors_path).is_file()
    {
        push_diag(
            diagnostics,
            PreflightDiagnosticSeverity::Warning,
            "missing_knowledge_vectors",
            format!(
                "Knowledge vectors file `{}` is missing.",
                embedding.vectors_path
            ),
            Some(path_for_report(
                workspace_root,
                &package.root.join(&embedding.vectors_path),
            )),
        );
    }
}

fn validate_optional_knowledge_file(
    workspace_root: &Path,
    package: &ResolvedPackageInfo,
    relative_path: Option<&str>,
    code: &str,
    label: &str,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) {
    let Some(relative_path) = relative_path else {
        return;
    };
    if package.root.join(relative_path).is_file() {
        return;
    }
    push_diag(
        diagnostics,
        PreflightDiagnosticSeverity::Warning,
        code,
        format!("{label} `{relative_path}` is missing."),
        Some(path_for_report(
            workspace_root,
            &package.root.join(relative_path),
        )),
    );
}

fn read_json_required(
    workspace_root: &Path,
    path: &Path,
    missing_code: &str,
    corrupt_code: &str,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) -> Option<Value> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) => {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Warning,
                missing_code,
                format!("{} could not be read: {err}", path.display()),
                Some(path_for_report(workspace_root, path)),
            );
            return None;
        }
    };
    match serde_json::from_slice(&bytes) {
        Ok(value) => Some(value),
        Err(err) => {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Warning,
                corrupt_code,
                format!("{} contains invalid JSON: {err}", path.display()),
                Some(path_for_report(workspace_root, path)),
            );
            None
        }
    }
}

fn load_skill_manifest(
    workspace_root: &Path,
    package: &ResolvedPackageInfo,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) -> Result<Option<SkillManifest>> {
    let path = package.root.join("agent.json");
    match load_manifest_value(&path).and_then(|(value, _)| parse_skill_manifest(&value)) {
        Ok(manifest) => Ok(Some(manifest)),
        Err(err) => {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Suppressed,
                "invalid_skill_manifest",
                format!(
                    "Skill manifest {} could not be loaded: {err}",
                    path.display()
                ),
                Some(path_for_report(workspace_root, &path)),
            );
            Ok(None)
        }
    }
}

fn load_memory_manifest(
    workspace_root: &Path,
    package: &ResolvedPackageInfo,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) -> Option<MemoryManifest> {
    let path = package.root.join("agent.json");
    match load_manifest_value(&path).and_then(|(value, _)| parse_memory_manifest(&value)) {
        Ok(manifest) => Some(manifest),
        Err(err) => {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Suppressed,
                "invalid_memory_manifest",
                format!(
                    "Memory manifest {} could not be loaded: {err}",
                    path.display()
                ),
                Some(path_for_report(workspace_root, &path)),
            );
            None
        }
    }
}

fn load_knowledge_manifest(
    workspace_root: &Path,
    package: &ResolvedPackageInfo,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) -> Option<KnowledgeManifest> {
    let path = package.root.join("agent.json");
    match load_manifest_value(&path).and_then(|(value, _)| parse_knowledge_manifest(&value)) {
        Ok(manifest) => Some(manifest),
        Err(err) => {
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Suppressed,
                "invalid_knowledge_manifest",
                format!(
                    "Knowledge manifest {} could not be loaded: {err}",
                    path.display()
                ),
                Some(path_for_report(workspace_root, &path)),
            );
            None
        }
    }
}

fn package_names(
    package_graph: &BTreeMap<String, ResolvedPackageInfo>,
    keys: &[String],
) -> BTreeSet<String> {
    keys.iter()
        .filter_map(|key| package_graph.get(key))
        .map(|package| package.name.clone())
        .collect()
}

fn package_by_name<'a>(
    package_graph: &'a BTreeMap<String, ResolvedPackageInfo>,
    kind: PackageKind,
    name: &str,
) -> Option<&'a ResolvedPackageInfo> {
    package_graph
        .values()
        .find(|package| package.kind == kind && package.name == name)
}

fn package_ref_parts(reference: &PackageReference) -> (String, String) {
    match reference {
        PackageReference::String(raw) => package_ref_string_parts(raw),
        PackageReference::Object { name, version } => (
            name.clone(),
            version.clone().unwrap_or_else(|| "*".to_string()),
        ),
    }
}

fn package_ref_string_parts(raw: &str) -> (String, String) {
    if let Some(last_at) = raw.rfind('@')
        && last_at > 0
    {
        return (raw[..last_at].to_string(), raw[last_at + 1..].to_string());
    }
    (raw.to_string(), "*".to_string())
}

fn package_identity(raw: &str) -> String {
    package_ref_string_parts(raw).0
}

fn mcp_import_scope(import: &HarnessMcpImport) -> &HarnessMcpScope {
    match import {
        HarnessMcpImport::Stdio { scope, .. } | HarnessMcpImport::Http { scope, .. } => scope,
    }
}

fn mcp_scope_label(scope: &HarnessMcpScope) -> String {
    match scope {
        HarnessMcpScope::Global => "global".to_string(),
        HarnessMcpScope::Phases { phases } => format!("phases:{}", phases.join(",")),
    }
}

fn installed_package_root(
    workspace_root: &Path,
    kind: PackageKind,
    package: &str,
    version: &str,
) -> Result<PathBuf> {
    let (owner, name) = split_package_ref(package)?;
    Ok(workspace_root
        .join(".agentpm")
        .join(package_dir(kind))
        .join(owner)
        .join(name)
        .join(version))
}

fn installed_manifest_path(
    workspace_root: &Path,
    kind: PackageKind,
    package: &str,
    version: &str,
) -> Result<PathBuf> {
    Ok(installed_package_root(workspace_root, kind, package, version)?.join("agent.json"))
}

fn package_dir(kind: PackageKind) -> &'static str {
    match kind {
        PackageKind::Tool => "tools",
        PackageKind::Agent => "agents",
        PackageKind::Skill => "skills",
        PackageKind::Knowledge => "knowledge",
        PackageKind::Memory => "memory",
        PackageKind::Profile => "profiles",
        PackageKind::Loop => "loops",
    }
}

fn resolve_workspace_file(root: &Path, relative: &str) -> Result<PathBuf> {
    let path = PathBuf::from(relative);
    if path.is_absolute() {
        bail!("path must be workspace-relative");
    }
    for component in path.components() {
        match component {
            Component::ParentDir => bail!("path must not contain `..`"),
            Component::RootDir | Component::Prefix(_) => bail!("path must be workspace-relative"),
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    let candidate = root.join(path);
    if candidate.exists() {
        let root = root.canonicalize()?;
        let resolved = candidate.canonicalize()?;
        if !resolved.starts_with(&root) {
            bail!("path escapes the package/workspace root");
        }
        return Ok(resolved);
    }
    Ok(candidate)
}

fn path_for_report(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

fn push_diag(
    diagnostics: &mut Vec<PreflightDiagnostic>,
    severity: PreflightDiagnosticSeverity,
    code: impl Into<String>,
    message: impl Into<String>,
    path: Option<impl Into<String>>,
) {
    diagnostics.push(PreflightDiagnostic {
        severity,
        code: code.into(),
        message: message.into(),
        path: path.map(Into::into),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::write_lock;
    use crate::semver::types::{LockV2, LockedPackage, LockedRoot, package_key};
    use chrono::Utc;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("agentpm-harness-plan-{label}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_json(path: &Path, value: Value) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    }

    fn lock_with_root(root: &Path, agent: LockedRoot, packages: BTreeMap<String, LockedPackage>) {
        write_lock(
            root,
            &Lock::V2(LockV2 {
                lockfile_version: 3,
                generated: Utc::now(),
                packages,
                roots: BTreeMap::from([("local:agent".to_string(), agent)]),
            }),
        )
        .unwrap();
    }

    fn base_packages() -> BTreeMap<String, LockedPackage> {
        let mut packages = BTreeMap::new();
        insert_package(
            &mut packages,
            PackageKind::Loop,
            "@zack/review-loop",
            "0.1.0",
        );
        insert_package(&mut packages, PackageKind::Tool, "@zack/search", "0.1.0");
        insert_package(&mut packages, PackageKind::Tool, "@zack/comment", "0.1.0");
        insert_package(
            &mut packages,
            PackageKind::Skill,
            "@zack/review-skill",
            "0.1.0",
        );
        insert_package(
            &mut packages,
            PackageKind::Memory,
            "@zack/session-memory",
            "0.1.0",
        );
        insert_package(
            &mut packages,
            PackageKind::Knowledge,
            "@zack/guide",
            "0.1.0",
        );
        insert_package(&mut packages, PackageKind::Profile, "@zack/style", "0.1.0");
        packages
    }

    fn insert_package(
        packages: &mut BTreeMap<String, LockedPackage>,
        kind: PackageKind,
        name: &str,
        version: &str,
    ) {
        packages.insert(
            package_key(kind, name, version),
            LockedPackage {
                kind,
                name: name.to_string(),
                version: version.to_string(),
                integrity: "sha256-test".to_string(),
            },
        );
    }

    fn base_root() -> LockedRoot {
        LockedRoot {
            name: Some("@zack/local-agent".to_string()),
            version: Some("0.1.0".to_string()),
            tools: vec![
                "tool:@zack/search@0.1.0".to_string(),
                "tool:@zack/comment@0.1.0".to_string(),
            ],
            skills: vec!["skill:@zack/review-skill@0.1.0".to_string()],
            knowledge: vec!["knowledge:@zack/guide@0.1.0".to_string()],
            memory: vec!["memory:@zack/session-memory@0.1.0".to_string()],
            profiles: vec!["profile:@zack/style@0.1.0".to_string()],
            r#loop: Some("loop:@zack/review-loop@0.1.0".to_string()),
            reserved: Default::default(),
        }
    }

    fn write_base_workspace(root: &Path) {
        write_json(
            &root.join("agent.json"),
            json!({
                "kind": "agent",
                "name": "@zack/local-agent",
                "version": "0.1.0",
                "description": "Local harness agent.",
                "tools": ["@zack/search@0.1.0", "@zack/comment@0.1.0"],
                "skills": ["@zack/review-skill@0.1.0"],
                "knowledge": ["@zack/guide@0.1.0"],
                "memory": ["@zack/session-memory@0.1.0"],
                "profiles": ["@zack/style@0.1.0"],
                "loop": "@zack/review-loop@0.1.0",
                "bindings": {
                    "consumer_context": { "file": "context.md" },
                    "global": {
                        "knowledge": ["@zack/guide"],
                        "profiles": ["@zack/style"],
                        "skills": ["@zack/review-skill"],
                        "memory": [
                            { "package": "@zack/session-memory", "spaces": ["session"] }
                        ]
                    },
                    "phases": {
                        "inspect": { "tools": ["@zack/search"] },
                        "respond": { "tools": ["@zack/comment"] }
                    },
                    "mcp": [
                        { "id": "agent-tools", "tools": ["@zack/search"] }
                    ]
                }
            }),
        );
        fs::write(
            root.join("context.md"),
            "Runtime-specific workspace context.\n",
        )
        .unwrap();
        write_loop(root);
        write_tool(root, "@zack/search", "0.1.0", "python");
        write_tool(root, "@zack/comment", "0.1.0", "python");
        write_skill(root);
        write_memory(root);
        write_knowledge(root);
    }

    fn package_root(root: &Path, kind: PackageKind, name: &str, version: &str) -> PathBuf {
        installed_package_root(root, kind, name, version).unwrap()
    }

    fn write_loop(root: &Path) {
        write_json(
            &package_root(root, PackageKind::Loop, "@zack/review-loop", "0.1.0").join("agent.json"),
            json!({
                "kind": "loop",
                "name": "@zack/review-loop",
                "version": "0.1.0",
                "description": "Review loop.",
                "loop": {
                    "entry_phase": "inspect",
                    "phases": [
                        {
                            "id": "inspect",
                            "objective": "Inspect the request.",
                            "outcomes": [
                                { "id": "respond", "description": "Draft a response." },
                                { "id": "handoff", "description": "Hand off." }
                            ]
                        },
                        {
                            "id": "respond",
                            "objective": "Respond.",
                            "outcomes": []
                        }
                    ],
                    "transitions": [
                        { "from": "inspect", "on": "respond", "to": "respond" },
                        { "from": "inspect", "on": "handoff", "to": "$handoff" },
                        { "from": "respond", "on": "complete", "to": "$end" }
                    ],
                    "checkpoints": [
                        {
                            "id": "approve-response",
                            "type": "approval",
                            "before_phase": "respond",
                            "on_reject": "$handoff"
                        }
                    ]
                }
            }),
        );
    }

    fn write_skill(root: &Path) {
        write_json(
            &package_root(root, PackageKind::Skill, "@zack/review-skill", "0.1.0")
                .join("agent.json"),
            json!({
                "kind": "skill",
                "name": "@zack/review-skill",
                "version": "0.1.0",
                "description": "Review skill.",
                "tools": ["@zack/search@0.1.0"],
                "skill": {
                    "entrypoint": "SKILL.md",
                    "references": []
                }
            }),
        );
    }

    fn write_tool(root: &Path, name: &str, version: &str, command: &str) {
        write_json(
            &package_root(root, PackageKind::Tool, name, version).join("agent.json"),
            json!({
                "kind": "tool",
                "name": name,
                "version": version,
                "description": "Harness test tool.",
                "entrypoint": {
                    "command": command,
                    "args": ["tool.py"],
                    "cwd": ".",
                    "timeout_ms": 1000,
                    "env": {}
                },
                "inputs": { "type": "object" },
                "outputs": { "type": "object" }
            }),
        );
    }

    fn write_memory(root: &Path) {
        let memory_root = package_root(root, PackageKind::Memory, "@zack/session-memory", "0.1.0");
        write_json(
            &memory_root.join("agent.json"),
            json!({
                "kind": "memory",
                "name": "@zack/session-memory",
                "version": "0.1.0",
                "description": "Session memory.",
                "memory": {
                    "scopes": { "user": { "description": "User." } },
                    "record_types": {
                        "note": {
                            "schema": "schemas/note.schema.json",
                            "version": "1.0.0",
                            "description": "Note."
                        }
                    },
                    "spaces": {
                        "session": {
                            "model": "document",
                            "scope": ["user"],
                            "retrieval": { "modes": ["key"] },
                            "description": "Session state.",
                            "record_types": ["note"]
                        }
                    }
                }
            }),
        );
        write_json(
            &memory_root.join("memory/build.json"),
            json!({ "type": "test" }),
        );
        write_json(
            &memory_root.join("memory/contracts/index.json"),
            json!({
                "contracts": [
                    { "path": "memory/contracts/session.note.schema.json" }
                ]
            }),
        );
        write_json(
            &memory_root.join("memory/contracts/session.note.schema.json"),
            json!({ "type": "object" }),
        );
    }

    fn write_knowledge(root: &Path) {
        let knowledge_root = package_root(root, PackageKind::Knowledge, "@zack/guide", "0.1.0");
        write_json(
            &knowledge_root.join("agent.json"),
            json!({
                "kind": "knowledge",
                "name": "@zack/guide",
                "version": "0.1.0",
                "description": "Guide.",
                "knowledge": {
                    "mode": "context",
                    "content_type": "text/markdown",
                    "documents": [
                        { "path": "knowledge/docs/guide.md", "content_type": "text/markdown" }
                    ]
                }
            }),
        );
        fs::create_dir_all(knowledge_root.join("knowledge/docs")).unwrap();
        fs::write(knowledge_root.join("knowledge/docs/guide.md"), "# Guide\n").unwrap();
    }

    fn options() -> HarnessBootstrapOptions {
        HarnessBootstrapOptions::default()
    }

    fn codes(plan: &ResolvedHarnessPlan) -> BTreeSet<String> {
        plan.report
            .diagnostics
            .iter()
            .map(|diag| diag.code.clone())
            .collect()
    }

    #[test]
    fn preflight_requires_agent_lock() {
        let root = temp_dir("missing-lock");
        let plan = resolve_harness_plan(&root, &options()).unwrap();
        assert_eq!(plan.report.status, PreflightStatus::Failed);
        assert!(codes(&plan).contains("missing_lock"));
    }

    #[test]
    fn preflight_selects_single_local_agent_and_applies_state_dir_override() {
        let root = temp_dir("single-agent");
        write_base_workspace(&root);
        lock_with_root(&root, base_root(), base_packages());
        let plan = resolve_harness_plan(
            &root,
            &HarnessBootstrapOptions {
                state_dir_override: Some(PathBuf::from("tmp/harness-state")),
                runtime_scopes: BTreeMap::from([("user".to_string(), "user-1".to_string())]),
                ..options()
            },
        )
        .unwrap();
        assert_eq!(plan.report.status, PreflightStatus::Ready);
        assert_eq!(
            plan.selected_agent.as_ref().unwrap().name,
            "@zack/local-agent"
        );
        assert!(plan.state_dir.ends_with("tmp/harness-state"));
    }

    #[test]
    fn preflight_selects_installed_agent_only_root() {
        let root = temp_dir("installed-agent");
        write_loop(&root);
        let mut packages = base_packages();
        insert_package(
            &mut packages,
            PackageKind::Agent,
            "@zack/installed-agent",
            "0.1.0",
        );
        let agent_root = package_root(&root, PackageKind::Agent, "@zack/installed-agent", "0.1.0");
        write_json(
            &agent_root.join("agent.json"),
            json!({
                "kind": "agent",
                "name": "@zack/installed-agent",
                "version": "0.1.0",
                "loop": "@zack/review-loop@0.1.0",
                "bindings": {}
            }),
        );
        write_lock(
            &root,
            &Lock::V2(LockV2 {
                lockfile_version: 3,
                generated: Utc::now(),
                packages,
                roots: BTreeMap::from([(
                    "agent:@zack/installed-agent@0.1.0".to_string(),
                    LockedRoot {
                        r#loop: Some("loop:@zack/review-loop@0.1.0".to_string()),
                        ..Default::default()
                    },
                )]),
            }),
        )
        .unwrap();

        let plan = resolve_harness_plan(&root, &options()).unwrap();
        assert_eq!(plan.report.status, PreflightStatus::Ready);
        assert_eq!(
            plan.selected_agent.as_ref().unwrap().name,
            "@zack/installed-agent"
        );
    }

    #[test]
    fn preflight_rejects_agent_without_loop_as_non_runnable() {
        let root = temp_dir("missing-loop");
        write_base_workspace(&root);
        let mut root_lock = base_root();
        root_lock.r#loop = None;
        lock_with_root(&root, root_lock, base_packages());
        let plan = resolve_harness_plan(&root, &options()).unwrap();
        assert_eq!(plan.report.status, PreflightStatus::Failed);
        assert!(codes(&plan).contains("agent_missing_loop"));
    }

    #[test]
    fn preflight_warns_unknown_phase_and_bad_memory_selector() {
        let root = temp_dir("bad-bindings");
        write_base_workspace(&root);
        write_json(
            &root.join("agent.json"),
            json!({
                "kind": "agent",
                "name": "@zack/local-agent",
                "version": "0.1.0",
                "tools": ["@zack/search@0.1.0"],
                "memory": ["@zack/session-memory@0.1.0"],
                "loop": "@zack/review-loop@0.1.0",
                "bindings": {
                    "phases": {
                        "missing-phase": {
                            "tools": ["@zack/search"],
                            "memory": [
                                { "package": "@zack/session-memory", "spaces": ["session"] }
                            ]
                        },
                        "inspect": {
                            "memory": [
                                { "package": "@zack/session-memory", "spaces": ["missing"] }
                            ]
                        }
                    }
                }
            }),
        );
        let mut root_lock = base_root();
        root_lock.tools = vec!["tool:@zack/search@0.1.0".to_string()];
        root_lock.skills = Vec::new();
        root_lock.knowledge = Vec::new();
        root_lock.profiles = Vec::new();
        lock_with_root(&root, root_lock, base_packages());
        let plan = resolve_harness_plan(&root, &options()).unwrap();
        let codes = codes(&plan);
        assert!(codes.contains("unknown_phase_binding"));
        assert!(codes.contains("bad_memory_selector"));
        assert!(!codes.contains("unresolved_runtime_scope"));
    }

    #[test]
    fn preflight_suppresses_missing_bound_package_and_invalid_mcp_export_tool() {
        let root = temp_dir("bad-package-binding");
        write_base_workspace(&root);
        write_json(
            &root.join("agent.json"),
            json!({
                "kind": "agent",
                "name": "@zack/local-agent",
                "version": "0.1.0",
                "tools": ["@zack/search@0.1.0"],
                "loop": "@zack/review-loop@0.1.0",
                "bindings": {
                    "global": {
                        "tools": ["@zack/not-declared"]
                    },
                    "mcp": [
                        { "id": "exports", "tools": ["@zack/search", "@zack/comment"] }
                    ]
                }
            }),
        );
        let mut root_lock = base_root();
        root_lock.tools = vec!["tool:@zack/search@0.1.0".to_string()];
        root_lock.skills = Vec::new();
        root_lock.knowledge = Vec::new();
        root_lock.memory = Vec::new();
        root_lock.profiles = Vec::new();
        lock_with_root(&root, root_lock, base_packages());
        let plan = resolve_harness_plan(&root, &options()).unwrap();
        let codes = codes(&plan);
        assert!(codes.contains("missing_bound_package"));
        assert!(codes.contains("invalid_mcp_export_tool"));
    }

    #[test]
    fn preflight_detects_skill_inherited_tool_duplicate() {
        let root = temp_dir("skill-dedupe");
        write_base_workspace(&root);
        write_json(
            &root.join("agent.json"),
            json!({
                "kind": "agent",
                "name": "@zack/local-agent",
                "version": "0.1.0",
                "tools": ["@zack/search@0.1.0"],
                "skills": ["@zack/review-skill@0.1.0"],
                "loop": "@zack/review-loop@0.1.0",
                "bindings": {
                    "global": {
                        "tools": ["@zack/search"],
                        "skills": ["@zack/review-skill"]
                    }
                }
            }),
        );
        let mut root_lock = base_root();
        root_lock.tools = vec!["tool:@zack/search@0.1.0".to_string()];
        root_lock.knowledge = Vec::new();
        root_lock.memory = Vec::new();
        root_lock.profiles = Vec::new();
        lock_with_root(&root, root_lock, base_packages());
        let plan = resolve_harness_plan(
            &root,
            &HarnessBootstrapOptions {
                runtime_scopes: BTreeMap::from([("user".to_string(), "user-1".to_string())]),
                ..options()
            },
        )
        .unwrap();
        assert!(codes(&plan).contains("duplicate_tool_candidate"));
    }

    #[test]
    fn preflight_marks_runtime_incompatible_tool_unavailable() {
        let root = temp_dir("tool-runtime");
        write_base_workspace(&root);
        write_json(
            &package_root(&root, PackageKind::Tool, "@zack/search", "0.1.0").join("agent.json"),
            json!({
                "kind": "tool",
                "name": "@zack/search",
                "version": "0.1.0",
                "description": "Runtime mismatch tool.",
                "entrypoint": {
                    "command": "python",
                    "args": ["tool.py"],
                    "cwd": ".",
                    "timeout_ms": 1000,
                    "env": {}
                },
                "runtime": {
                    "type": "node"
                },
                "inputs": { "type": "object" },
                "outputs": { "type": "object" }
            }),
        );
        lock_with_root(&root, base_root(), base_packages());

        let plan = resolve_harness_plan(
            &root,
            &HarnessBootstrapOptions {
                runtime_scopes: BTreeMap::from([("user".to_string(), "user-1".to_string())]),
                ..options()
            },
        )
        .unwrap();

        assert!(codes(&plan).contains("tool_runtime_mismatch"));
        assert_eq!(
            plan.report
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == "tool_runtime_mismatch")
                .count(),
            1
        );
        assert!(
            plan.capabilities.iter().any(|capability| {
                capability.kind == "tool"
                    && capability.identity == "@zack/search"
                    && capability.state == CapabilityState::Unavailable
            }),
            "expected @zack/search to be unavailable, got: {:#?}",
            plan.capabilities
        );
    }

    #[test]
    fn preflight_warns_for_missing_tool_required_environment() {
        let root = temp_dir("tool-env");
        write_base_workspace(&root);
        write_json(
            &package_root(&root, PackageKind::Tool, "@zack/search", "0.1.0").join("agent.json"),
            json!({
                "kind": "tool",
                "name": "@zack/search",
                "version": "0.1.0",
                "description": "Env-dependent tool.",
                "entrypoint": {
                    "command": "python",
                    "args": ["tool.py"],
                    "cwd": ".",
                    "timeout_ms": 1000,
                    "env": {}
                },
                "environment": {
                    "vars": {
                        "AGENTPM_HARNESS_REQUIRED_TEST_TOKEN_123": {
                            "required": true,
                            "description": "Required test token."
                        }
                    }
                },
                "inputs": { "type": "object" },
                "outputs": { "type": "object" }
            }),
        );
        lock_with_root(&root, base_root(), base_packages());

        let plan = resolve_harness_plan(
            &root,
            &HarnessBootstrapOptions {
                runtime_scopes: BTreeMap::from([("user".to_string(), "user-1".to_string())]),
                ..options()
            },
        )
        .unwrap();

        assert!(codes(&plan).contains("tool_required_environment_missing"));
        assert!(
            plan.capabilities.iter().any(|capability| {
                capability.kind == "tool"
                    && capability.identity == "@zack/search"
                    && capability.state == CapabilityState::Available
            }),
            "expected @zack/search to remain available with an env warning, got: {:#?}",
            plan.capabilities
        );
    }

    #[test]
    fn preflight_rejects_multiple_agents_without_selector() {
        let root = temp_dir("multiple-agents");
        write_base_workspace(&root);
        write_json(
            &root.join("agents/other.agent.json"),
            json!({
                "kind": "agent",
                "name": "@zack/other-agent",
                "version": "0.1.0",
                "loop": "@zack/review-loop@0.1.0"
            }),
        );
        let packages = base_packages();
        write_lock(
            &root,
            &Lock::V2(LockV2 {
                lockfile_version: 3,
                generated: Utc::now(),
                packages,
                roots: BTreeMap::from([
                    ("local:agent".to_string(), base_root()),
                    (
                        "local:agent:agents/other.agent.json".to_string(),
                        LockedRoot {
                            name: Some("@zack/other-agent".to_string()),
                            version: Some("0.1.0".to_string()),
                            r#loop: Some("loop:@zack/review-loop@0.1.0".to_string()),
                            ..Default::default()
                        },
                    ),
                ]),
            }),
        )
        .unwrap();
        let plan = resolve_harness_plan(&root, &options()).unwrap();
        assert_eq!(plan.report.status, PreflightStatus::SelectionRequired);
        assert!(codes(&plan).contains("agent_selection_required"));

        let selected = resolve_harness_plan(
            &root,
            &HarnessBootstrapOptions {
                agent_selector: Some("@zack/other-agent".to_string()),
                ..options()
            },
        )
        .unwrap();
        assert_eq!(
            selected.selected_agent.as_ref().unwrap().name,
            "@zack/other-agent"
        );
    }

    #[test]
    fn config_local_mcp_phase_scope_passes_then_preflight_warns_unknown_phase() {
        let root = temp_dir("mcp-phase");
        write_base_workspace(&root);
        lock_with_root(&root, base_root(), base_packages());
        write_json(
            &root.join("agentpm.harness.json"),
            json!({
                "version": 1,
                "mcp": {
                    "imports": {
                        "docs": {
                            "transport": "stdio",
                            "command": "node",
                            "scope": {
                                "mode": "phases",
                                "phases": ["not-a-loop-phase"]
                            }
                        }
                    }
                }
            }),
        );
        let plan = resolve_harness_plan(&root, &options()).unwrap();
        assert!(codes(&plan).contains("unknown_mcp_import_phase"));
    }

    #[test]
    fn preflight_warns_missing_consumer_context_file() {
        let root = temp_dir("missing-context");
        write_base_workspace(&root);
        write_json(
            &root.join("agent.json"),
            json!({
                "kind": "agent",
                "name": "@zack/local-agent",
                "version": "0.1.0",
                "loop": "@zack/review-loop@0.1.0",
                "bindings": {
                    "consumer_context": { "file": "missing-context.md" }
                }
            }),
        );
        let mut root_lock = base_root();
        root_lock.tools = Vec::new();
        root_lock.skills = Vec::new();
        root_lock.knowledge = Vec::new();
        root_lock.memory = Vec::new();
        root_lock.profiles = Vec::new();
        lock_with_root(&root, root_lock, base_packages());

        let plan = resolve_harness_plan(&root, &options()).unwrap();

        assert!(codes(&plan).contains("missing_consumer_context"));
        assert_eq!(plan.consumer_context.state, CapabilityState::Unavailable);
        assert_eq!(
            plan.consumer_context.file.as_deref(),
            Some("missing-context.md")
        );
        assert!(
            plan.consumer_context
                .path
                .as_ref()
                .is_some_and(|path| path.ends_with("missing-context.md"))
        );
    }

    #[test]
    fn preflight_warns_unsafe_consumer_context_and_unresolved_scope() {
        let root = temp_dir("context-scope");
        write_base_workspace(&root);
        write_json(
            &root.join("agent.json"),
            json!({
                "kind": "agent",
                "name": "@zack/local-agent",
                "version": "0.1.0",
                "memory": ["@zack/session-memory@0.1.0"],
                "loop": "@zack/review-loop@0.1.0",
                "bindings": {
                    "consumer_context": { "file": "../secret.md" },
                    "global": {
                        "memory": [
                            { "package": "@zack/session-memory", "spaces": ["session"] }
                        ]
                    }
                }
            }),
        );
        let mut root_lock = base_root();
        root_lock.tools = Vec::new();
        root_lock.skills = Vec::new();
        root_lock.knowledge = Vec::new();
        root_lock.profiles = Vec::new();
        lock_with_root(&root, root_lock, base_packages());
        let plan = resolve_harness_plan(&root, &options()).unwrap();
        let codes = codes(&plan);
        assert!(codes.contains("unsafe_consumer_context"));
        assert!(codes.contains("unresolved_runtime_scope"));
    }

    #[test]
    fn preflight_reports_irrelevant_runtime_mapping_and_host_readiness() {
        let root = temp_dir("config-readiness");
        write_base_workspace(&root);
        lock_with_root(&root, base_root(), base_packages());
        write_json(
            &root.join("agentpm.harness.json"),
            json!({
                "version": 1,
                "providers": {
                    "models": {
                        "custom-model": {
                            "implementation": {
                                "type": "host",
                                "request_timeout_ms": 1000
                            }
                        }
                    }
                },
                "knowledge": {
                    "runtimes": {
                        "remote-knowledge": {
                            "implementation": {
                                "type": "host",
                                "request_timeout_ms": 1000
                            }
                        }
                    },
                    "packages": {
                        "@zack/not-used": { "runtime": "remote-knowledge" }
                    }
                }
            }),
        );
        let plan = resolve_harness_plan(&root, &options()).unwrap();
        let codes = codes(&plan);
        assert!(codes.contains("irrelevant_knowledge_runtime_mapping"));
        assert!(codes.contains("host_implementation_unavailable"));
    }

    #[test]
    fn preflight_warns_missing_generated_memory_and_knowledge_metadata() {
        let root = temp_dir("generated-missing");
        write_base_workspace(&root);
        fs::remove_file(
            package_root(
                root.as_path(),
                PackageKind::Memory,
                "@zack/session-memory",
                "0.1.0",
            )
            .join("memory/build.json"),
        )
        .unwrap();
        fs::remove_file(
            package_root(
                root.as_path(),
                PackageKind::Knowledge,
                "@zack/guide",
                "0.1.0",
            )
            .join("knowledge/docs/guide.md"),
        )
        .unwrap();
        lock_with_root(&root, base_root(), base_packages());
        let plan = resolve_harness_plan(
            &root,
            &HarnessBootstrapOptions {
                runtime_scopes: BTreeMap::from([("user".to_string(), "user-1".to_string())]),
                ..options()
            },
        )
        .unwrap();
        let codes = codes(&plan);
        assert!(codes.contains("missing_memory_build_metadata"));
        assert!(codes.contains("missing_knowledge_document"));
    }
}
