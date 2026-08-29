use crate::harness_plan::{
    CapabilityState, HarnessBootstrapOptions, HarnessExecutionSurface, PreflightDiagnosticSeverity,
    PreflightStatus, ResolvedHarnessPlan, ResolvedPackageInfo, resolve_harness_plan,
};
use crate::harness_runtime::{
    ActionDispatcher, AgentPmActionDispatcher, ApprovalController, BuiltInModelRuntime,
    ConsumerContextSnapshot, ModelProviderSelection, ModelRuntime, PackageSnapshot,
    RuntimeCapabilitySnapshot, RuntimeSnapshot, ServiceReadinessSnapshot, SkillResourceSnapshot,
    SkillRuntimeSnapshot, ToolRuntimeSnapshot,
};
use crate::manifest::{
    load_manifest_value, parse_loop_manifest, parse_skill_manifest, parse_tool_manifest,
};
use crate::prelude::*;
use crate::{
    harness_engine::{
        HarnessEngine, HarnessEngineOptions, HarnessRunResult, HarnessSession,
        RuntimeTerminalResult,
    },
    harness_observability::{JsonlTraceSink, RunOutputPaths, allocate_harness_run_id},
};
use anyhow::{anyhow, bail};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::PathBuf;

use crate::semver::types::PackageKind;

#[derive(Args, Debug, Clone)]
pub struct HarnessArgs {
    /// Agent package identity to run, for example @owner/name or @owner/name@version
    #[arg(value_name = "AGENT")]
    pub agent: Option<String>,

    /// Path to agentpm.harness.json
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Override runtime.state_dir for this Harness session
    #[arg(long, value_name = "DIR")]
    pub state_dir: Option<PathBuf>,

    /// Resolve a runtime scope value, for example --scope user=user_123
    #[arg(long = "scope", value_name = "KEY=VALUE", value_parser = parse_scope)]
    pub scopes: Vec<(String, String)>,

    /// Use the machine/SDK protocol surface for readiness classification
    #[arg(long, conflicts_with = "headless")]
    pub machine: bool,

    /// Use the plain non-TUI script surface for readiness classification
    #[arg(long, conflicts_with = "machine")]
    pub headless: bool,

    /// Emit the preflight report as JSON
    #[arg(long)]
    pub json: bool,

    /// Run input text for one-shot --headless execution
    #[arg(long, value_name = "TEXT", conflicts_with = "input_file")]
    pub input: Option<String>,

    /// File containing Run input for one-shot --headless execution
    #[arg(long = "input-file", value_name = "FILE", conflicts_with = "input")]
    pub input_file: Option<PathBuf>,

    /// Override the generated run report path for one-shot --headless execution
    #[arg(long = "report", value_name = "FILE")]
    pub report: Option<PathBuf>,
}

impl HarnessArgs {
    pub async fn run(self, _base_url: String) -> Result<()> {
        let workspace_root = std::env::current_dir().context("reading current directory")?;
        let surface = self.surface();
        validate_surface_flags(surface, &self)?;
        let plan = resolve_harness_plan(
            &workspace_root,
            &HarnessBootstrapOptions {
                agent_selector: self.agent.clone(),
                config_path: self.config.clone(),
                state_dir_override: self.state_dir.clone(),
                runtime_scopes: self.scopes.iter().cloned().collect(),
                surface,
            },
        )?;

        if self.json {
            println!("{}", serde_json::to_string_pretty(&plan.report)?);
        } else {
            let stream = PreflightOutputStream::for_surface(surface);
            print_harness_preflight(&plan, surface, stream)?;
        }

        match plan.report.status {
            PreflightStatus::Ready | PreflightStatus::ReadyWithWarnings => {}
            PreflightStatus::SelectionRequired => {
                bail!("Harness preflight requires an explicit Agent selector")
            }
            PreflightStatus::Failed => bail!("Harness preflight failed"),
        }

        run_surface(surface, plan, self)
    }

    fn surface(&self) -> HarnessExecutionSurface {
        if self.machine {
            HarnessExecutionSurface::Machine
        } else if self.headless {
            HarnessExecutionSurface::Headless
        } else {
            HarnessExecutionSurface::Tui
        }
    }
}

fn validate_surface_flags(surface: HarnessExecutionSurface, args: &HarnessArgs) -> Result<()> {
    if surface == HarnessExecutionSurface::Headless && args.json {
        bail!(
            "--json cannot be combined with --headless; headless stdout is reserved for final output"
        );
    }
    Ok(())
}

impl HarnessExecutionSurface {
    fn run(self, plan: &ResolvedHarnessPlan, args: &HarnessArgs) -> Result<()> {
        match self {
            HarnessExecutionSurface::Headless => run_headless_surface(plan, args),
            HarnessExecutionSurface::Machine => run_machine_surface(plan),
            HarnessExecutionSurface::Tui => run_tui_surface(plan),
        }
    }
}

fn run_surface(
    surface: HarnessExecutionSurface,
    plan: ResolvedHarnessPlan,
    args: HarnessArgs,
) -> Result<()> {
    if surface == HarnessExecutionSurface::Headless {
        return run_headless_worker(move || surface.run(&plan, &args));
    }
    surface.run(&plan, &args)
}

fn run_headless_worker(run: impl FnOnce() -> Result<()> + Send + 'static) -> Result<()> {
    std::thread::spawn(run)
        .join()
        .map_err(|_| anyhow!("Harness headless worker panicked"))?
}

fn run_tui_surface(_plan: &ResolvedHarnessPlan) -> Result<()> {
    Ok(())
}

fn run_machine_surface(_plan: &ResolvedHarnessPlan) -> Result<()> {
    Ok(())
}

fn run_headless_surface(plan: &ResolvedHarnessPlan, args: &HarnessArgs) -> Result<()> {
    let input = read_run_input(args.input.as_deref(), args.input_file.as_ref())?;
    let selection = model_selection(plan)?;
    let mut model =
        BuiltInModelRuntime::from_selection(selection).map_err(|err| anyhow!(err.message))?;
    validate_model_capabilities(&model)?;
    let runtime = runtime_snapshot_from_plan(plan);
    let mut dispatcher = AgentPmActionDispatcher::from_runtime(&runtime)?;
    let terminal = execute_headless_plan(
        plan,
        input,
        args.report.as_ref(),
        &mut model,
        &mut dispatcher,
    )?;
    match terminal.status {
        crate::harness_observability::HarnessTerminalStatus::Ended
        | crate::harness_observability::HarnessTerminalStatus::HandedOff => {
            print_terminal_output(&terminal)?;
            Ok(())
        }
        status => bail!("{}", terminal_status_error_message(&terminal, status)?),
    }
}

fn validate_model_capabilities(model: &dyn ModelRuntime) -> Result<()> {
    let capabilities = model.capabilities();
    if !capabilities.semantic_actions {
        bail!("selected model runtime does not advertise Harness semantic action support");
    }
    if !capabilities.structured_output {
        bail!(
            "selected model runtime does not advertise structured output support required by Harness"
        );
    }
    Ok(())
}

fn print_terminal_output(terminal: &RuntimeTerminalResult) -> Result<()> {
    let Some(output) = &terminal.output else {
        return Ok(());
    };
    if let Some(text) = output.as_str() {
        println!("{text}");
    } else {
        println!("{}", serde_json::to_string_pretty(output)?);
    }
    Ok(())
}

fn terminal_status_error_message(
    terminal: &RuntimeTerminalResult,
    status: crate::harness_observability::HarnessTerminalStatus,
) -> Result<String> {
    let base = format!("Harness Run finished with terminal status {status:?}");
    let Some(output) = &terminal.output else {
        return Ok(base);
    };
    if let Some(error) = output.get("error").and_then(|value| value.as_str()) {
        return Ok(format!("{base}: {error}"));
    }
    if let Some(text) = output.as_str() {
        return Ok(format!("{base}: {text}"));
    }
    Ok(format!("{base}: {}", serde_json::to_string(output)?))
}

fn read_run_input(input: Option<&str>, input_file: Option<&PathBuf>) -> Result<String> {
    if let Some(input) = input {
        return Ok(input.to_string());
    }
    if let Some(path) = input_file {
        return std::fs::read_to_string(path)
            .with_context(|| format!("reading Harness input file {}", path.display()));
    }
    let mut buffer = String::new();
    std::io::stdin()
        .read_to_string(&mut buffer)
        .context("reading Harness input from stdin")?;
    if buffer.trim().is_empty() {
        bail!("Harness --headless requires --input, --input-file, or stdin input");
    }
    Ok(buffer)
}

fn model_selection(plan: &ResolvedHarnessPlan) -> Result<ModelProviderSelection> {
    let Some(model) = &plan.config.config.model else {
        bail!("Harness --headless requires model.provider and model.model in agentpm.harness.json");
    };
    Ok(ModelProviderSelection {
        provider: model.provider.clone(),
        model: model.model.clone(),
        options: model.options.clone(),
    })
}

fn execute_headless_plan(
    plan: &ResolvedHarnessPlan,
    input: String,
    report_override: Option<&PathBuf>,
    model: &mut dyn ModelRuntime,
    dispatcher: &mut dyn ActionDispatcher,
) -> Result<RuntimeTerminalResult> {
    let mut approvals = HeadlessApprovalController;
    execute_headless_plan_with_approvals(
        plan,
        input,
        report_override,
        model,
        dispatcher,
        &mut approvals,
    )
}

fn execute_headless_plan_with_approvals(
    plan: &ResolvedHarnessPlan,
    input: String,
    report_override: Option<&PathBuf>,
    model: &mut dyn ModelRuntime,
    dispatcher: &mut dyn ActionDispatcher,
    approvals: &mut dyn ApprovalController,
) -> Result<RuntimeTerminalResult> {
    let loop_manifest = load_plan_loop(plan)?;
    let mut session = HarnessSession::with_runtime_snapshot(runtime_snapshot_from_plan(plan));
    let run_id = allocate_harness_run_id();
    let output_paths = RunOutputPaths::resolve(
        &plan.state_dir,
        &run_id,
        report_override.map(PathBuf::as_path),
    )?;
    if plan.config.config.trace.enabled {
        session.emitter.add_sink(Box::new(JsonlTraceSink::create(
            &output_paths.events_path,
            plan.config.config.trace.clone(),
        )?));
    }
    let mut engine = HarnessEngine::new(
        loop_manifest,
        HarnessEngineOptions::new(plan.config.config.runtime.limits.clone()),
    );
    let result =
        engine.execute_run_with_id(&mut session, run_id, input, model, dispatcher, approvals)?;
    let HarnessRunResult::Terminal(result) = result else {
        bail!("Harness --headless cannot wait for interactive approval");
    };
    let mut terminal = *result;
    if plan.config.config.trace.enabled {
        terminal.report.trace_path = Some(output_paths.events_path.display().to_string());
    }
    session.emitter.flush()?;
    terminal
        .report
        .write_pretty(&output_paths.report_path, &plan.config.config.trace.content)?;
    Ok(terminal)
}

struct HeadlessApprovalController;

impl ApprovalController for HeadlessApprovalController {
    fn request_approval(
        &mut self,
        _checkpoint: &crate::manifest::LoopCheckpoint,
    ) -> crate::harness_runtime::ApprovalDecision {
        crate::harness_runtime::ApprovalDecision::Pending
    }
}

fn load_plan_loop(plan: &ResolvedHarnessPlan) -> Result<crate::manifest::LoopManifest> {
    let Some(loop_package) = &plan.loop_package else {
        bail!("Harness plan does not include a resolved Loop package");
    };
    let manifest_path = loop_package.root.join("agent.json");
    let (value, _) = load_manifest_value(&manifest_path)?;
    parse_loop_manifest(&value)
}

fn runtime_snapshot_from_plan(plan: &ResolvedHarnessPlan) -> RuntimeSnapshot {
    RuntimeSnapshot {
        session_id: String::new(),
        workspace_root: plan.workspace_root.clone(),
        state_dir: plan.state_dir.clone(),
        agent: plan.selected_agent.as_ref().map(|agent| PackageSnapshot {
            kind: "agent".into(),
            name: agent.name.clone(),
            version: agent.version.clone(),
            root: agent.manifest_path.parent().map(PathBuf::from),
        }),
        loop_package: plan.loop_package.as_ref().map(package_snapshot),
        package_graph: plan.package_graph.values().map(package_snapshot).collect(),
        runtime_config_sources: BTreeMap::from([(
            "state_dir".into(),
            format!("{:?}", plan.config.state_dir_source.kind),
        )]),
        runtime_scopes: plan.runtime_scopes.clone(),
        consumer_context: Some(ConsumerContextSnapshot {
            state: format!("{:?}", plan.consumer_context.state),
            file: plan.consumer_context.file.clone(),
            path: plan.consumer_context.path.clone(),
            content: None,
            byte_size: plan.consumer_context.byte_size,
            approximate_tokens: plan.consumer_context.approximate_tokens,
            sha256: plan.consumer_context.sha256.clone(),
        }),
        services: plan
            .capabilities
            .iter()
            .filter(|capability| capability.kind == "model_provider")
            .map(|capability| ServiceReadinessSnapshot {
                kind: capability.kind.clone(),
                identity: capability.identity.clone(),
                state: format!("{:?}", capability.state),
            })
            .collect(),
        hook_registrations: plan
            .config
            .config
            .hooks
            .bindings
            .iter()
            .map(|binding| format!("{:?}:{}", binding.hook, binding.implementation))
            .collect(),
        profiles: plan.profiles.values().cloned().collect(),
        profile_bindings: plan.profile_bindings.clone(),
        tools: tool_snapshots_from_plan(plan),
        skills: skill_snapshots_from_plan(plan),
        capability_candidates: plan
            .capabilities
            .iter()
            .map(|capability| RuntimeCapabilitySnapshot {
                kind: capability.kind.clone(),
                identity: capability.identity.clone(),
                scope: capability.scope.clone(),
                source: capability.source.clone(),
                state: capability_state_label(capability.state).into(),
            })
            .collect(),
        model: plan
            .config
            .config
            .model
            .as_ref()
            .map(|model| ModelProviderSelection {
                provider: model.provider.clone(),
                model: model.model.clone(),
                options: model.options.clone(),
            }),
    }
}

fn tool_snapshots_from_plan(plan: &ResolvedHarnessPlan) -> Vec<ToolRuntimeSnapshot> {
    let bound_tools = plan
        .capabilities
        .iter()
        .filter(|capability| capability.kind == "tool")
        .map(|capability| {
            (
                capability.identity.clone(),
                (
                    capability_state_label(capability.state).to_string(),
                    capability.source.clone(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    plan.package_graph
        .values()
        .filter(|package| package.kind == PackageKind::Tool)
        .filter_map(|package| {
            let (state, source) = bound_tools.get(&package.name)?;
            let path = package.root.join("agent.json");
            let manifest = load_manifest_value(&path)
                .and_then(|(value, _)| parse_tool_manifest(&value))
                .ok()?;
            Some(ToolRuntimeSnapshot {
                name: package.name.clone(),
                version: package.version.clone(),
                description: manifest
                    .description
                    .unwrap_or_else(|| "AgentPM Tool capability.".into()),
                root: Some(package.root.clone()),
                input_schema: manifest.inputs,
                state: state.clone(),
                source: source.clone(),
            })
        })
        .collect()
}

fn skill_snapshots_from_plan(plan: &ResolvedHarnessPlan) -> Vec<SkillRuntimeSnapshot> {
    let bound_skills = plan
        .capabilities
        .iter()
        .filter(|capability| capability.kind == "skill")
        .map(|capability| {
            (
                capability.identity.clone(),
                (
                    capability_state_label(capability.state).to_string(),
                    capability.source.clone(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    plan.package_graph
        .values()
        .filter(|package| package.kind == PackageKind::Skill)
        .filter_map(|package| {
            let (state, source) = bound_skills.get(&package.name)?;
            let path = package.root.join("agent.json");
            let manifest = load_manifest_value(&path)
                .and_then(|(value, _)| parse_skill_manifest(&value))
                .ok()?;
            let mut resources = vec![SkillResourceSnapshot {
                id: "entrypoint".into(),
                path: manifest.skill.entrypoint.clone(),
                kind: "entrypoint".into(),
            }];
            resources.extend(manifest.skill.references.iter().map(|reference| {
                SkillResourceSnapshot {
                    id: reference.clone(),
                    path: reference.clone(),
                    kind: "reference".into(),
                }
            }));
            Some(SkillRuntimeSnapshot {
                name: package.name.clone(),
                version: package.version.clone(),
                description: manifest
                    .description
                    .unwrap_or_else(|| "AgentPM Skill resource.".into()),
                root: Some(package.root.clone()),
                resources,
                state: state.clone(),
                source: source.clone(),
            })
        })
        .collect()
}

fn capability_state_label(state: CapabilityState) -> &'static str {
    match state {
        CapabilityState::Available => "available",
        CapabilityState::Pending => "pending",
        CapabilityState::Unavailable => "unavailable",
        CapabilityState::Suppressed => "suppressed",
        CapabilityState::NotConfigured => "not_configured",
    }
}

fn package_snapshot(package: &ResolvedPackageInfo) -> PackageSnapshot {
    PackageSnapshot {
        kind: format!("{:?}", package.kind),
        name: package.name.clone(),
        version: package.version.clone(),
        root: Some(package.root.clone()),
    }
}

fn parse_scope(raw: &str) -> Result<(String, String)> {
    let Some((key, value)) = raw.split_once('=') else {
        return Err(anyhow!("scope must be KEY=VALUE"));
    };
    if key.is_empty() || value.is_empty() {
        return Err(anyhow!("scope key and value must be non-empty"));
    }
    Ok((key.to_string(), value.to_string()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreflightOutputStream {
    Stdout,
    Stderr,
}

impl PreflightOutputStream {
    fn for_surface(surface: HarnessExecutionSurface) -> Self {
        match surface {
            HarnessExecutionSurface::Headless => Self::Stderr,
            HarnessExecutionSurface::Machine | HarnessExecutionSurface::Tui => Self::Stdout,
        }
    }

    fn line(self, line: impl std::fmt::Display) -> Result<()> {
        use std::io::Write;
        match self {
            Self::Stdout => {
                let mut stdout = std::io::stdout().lock();
                writeln!(stdout, "{line}").context("writing Harness preflight to stdout")
            }
            Self::Stderr => {
                let mut stderr = std::io::stderr().lock();
                writeln!(stderr, "{line}").context("writing Harness preflight to stderr")
            }
        }
    }
}

fn print_harness_preflight(
    plan: &ResolvedHarnessPlan,
    surface: HarnessExecutionSurface,
    stream: PreflightOutputStream,
) -> Result<()> {
    stream.line("AgentPM Harness preflight")?;
    stream.line(format!("Workspace: {}", plan.workspace_root.display()))?;
    stream.line(format!("Lockfile: {}", plan.lock_path.display()))?;
    stream.line(format!("State dir: {}", plan.state_dir.display()))?;
    if let Some(config_path) = &plan.config.config_path {
        stream.line(format!("Config: {}", config_path.display()))?;
    } else {
        stream.line("Config: defaults")?;
    }
    if let Some(agent) = &plan.selected_agent {
        stream.line(format!("Agent: {}@{}", agent.name, agent.version))?;
    }
    if let Some(loop_package) = &plan.loop_package {
        stream.line(format!(
            "Loop: {}@{}",
            loop_package.name, loop_package.version
        ))?;
    }
    stream.line(format!("Resolved packages: {}", plan.package_graph.len()))?;
    stream.line(format!("Runtime scopes: {}", plan.runtime_scopes.len()))?;
    match &plan.consumer_context.file {
        Some(file) => {
            let mut line = format!(
                "Consumer context: {file} ({:?})",
                plan.consumer_context.state
            );
            if let Some(byte_size) = plan.consumer_context.byte_size {
                line.push_str(&format!(", {byte_size} bytes"));
            }
            if let Some(approximate_tokens) = plan.consumer_context.approximate_tokens {
                line.push_str(&format!(", ~{approximate_tokens} tokens"));
            }
            if let Some(sha256) = &plan.consumer_context.sha256 {
                line.push_str(&format!(", {sha256}"));
            }
            stream.line(line)?
        }
        None => stream.line("Consumer context: not configured")?,
    }

    let capability_counts = capability_counts(plan);
    if !capability_counts.is_empty() {
        stream.line("")?;
        stream.line("Static capabilities:")?;
        for (state, count) in capability_counts {
            stream.line(format!("- {state}: {count}"))?;
        }
    }

    if !plan.report.diagnostics.is_empty() {
        stream.line("")?;
        stream.line("Diagnostics:")?;
        for diagnostic in &plan.report.diagnostics {
            let severity = match diagnostic.severity {
                PreflightDiagnosticSeverity::Fatal => "fatal",
                PreflightDiagnosticSeverity::Warning => "warning",
                PreflightDiagnosticSeverity::Suppressed => "suppressed",
                PreflightDiagnosticSeverity::Pending => "pending",
                PreflightDiagnosticSeverity::Info => "info",
            };
            if let Some(path) = &diagnostic.path {
                stream.line(format!(
                    "- [{severity}] {} ({path}) — {}",
                    diagnostic.code, diagnostic.message
                ))?;
            } else {
                stream.line(format!(
                    "- [{severity}] {} — {}",
                    diagnostic.code, diagnostic.message
                ))?;
            }
        }
    }

    stream.line("")?;
    stream.line(format!("Status: {:?}", plan.report.status))?;
    match surface {
        HarnessExecutionSurface::Headless => {
            stream.line("Execution: starting one-shot headless run after preflight.")?
        }
        HarnessExecutionSurface::Machine | HarnessExecutionSurface::Tui => {
            stream.line("Execution: preflight only for this surface in the current milestone.")?
        }
    }
    Ok(())
}

fn capability_counts(plan: &ResolvedHarnessPlan) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    let mut seen = std::collections::BTreeSet::new();
    for capability in &plan.capabilities {
        let key = match capability.state {
            CapabilityState::Available => "available",
            CapabilityState::Pending => "pending",
            CapabilityState::Unavailable => "unavailable",
            CapabilityState::Suppressed => "suppressed",
            CapabilityState::NotConfigured => "not_configured",
        };
        if !seen.insert((&capability.kind, &capability.identity, key)) {
            continue;
        }
        *counts.entry(key).or_insert(0) += 1;
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness_config::{
        HarnessConfig, HarnessConfigSource, HarnessTraceConfig, HarnessTraceContent,
        HarnessTraceLevel, ResolvedHarnessConfig,
    };
    use crate::harness_observability::{
        HarnessTerminalStatus, ReportPackageIdentity, RunReport, RunUsage,
    };
    use crate::harness_runtime::SemanticAction;
    use crate::harness_runtime::action::ScriptedActionDispatcher;
    use crate::harness_runtime::action::SemanticActionProposal;
    use crate::harness_runtime::model::{
        ModelCapabilityAdvertisement, ModelRuntimeFailure, ModelTurn, ScriptedModelRuntime,
    };
    use crate::semver::types::PackageKind;
    use serde_json::json;
    use std::fs;
    use std::path::Path;

    #[test]
    fn parse_scope_requires_key_value_pair() {
        assert_eq!(
            parse_scope("user=user-1").unwrap(),
            ("user".to_string(), "user-1".to_string())
        );
        assert!(parse_scope("user").is_err());
        assert!(parse_scope("=user-1").is_err());
        assert!(parse_scope("user=").is_err());
    }

    #[test]
    fn default_surface_is_tui_with_explicit_headless_and_machine_modes() {
        let default_args = HarnessArgs {
            agent: None,
            config: None,
            state_dir: None,
            scopes: Vec::new(),
            machine: false,
            headless: false,
            json: false,
            input: None,
            input_file: None,
            report: None,
        };
        assert_eq!(default_args.surface(), HarnessExecutionSurface::Tui);

        let headless_args = HarnessArgs {
            headless: true,
            ..default_args.clone()
        };
        assert_eq!(headless_args.surface(), HarnessExecutionSurface::Headless);

        let machine_args = HarnessArgs {
            machine: true,
            ..default_args
        };
        assert_eq!(machine_args.surface(), HarnessExecutionSurface::Machine);
    }

    #[test]
    fn headless_preflight_uses_stderr_and_rejects_json_flag() {
        assert_eq!(
            PreflightOutputStream::for_surface(HarnessExecutionSurface::Headless),
            PreflightOutputStream::Stderr
        );
        assert_eq!(
            PreflightOutputStream::for_surface(HarnessExecutionSurface::Tui),
            PreflightOutputStream::Stdout
        );
        assert_eq!(
            PreflightOutputStream::for_surface(HarnessExecutionSurface::Machine),
            PreflightOutputStream::Stdout
        );
        let args = HarnessArgs {
            agent: None,
            config: None,
            state_dir: None,
            scopes: Vec::new(),
            machine: false,
            headless: true,
            json: true,
            input: None,
            input_file: None,
            report: None,
        };
        let err = validate_surface_flags(HarnessExecutionSurface::Headless, &args).unwrap_err();
        assert!(err.to_string().contains("--json cannot be combined"));
    }

    #[test]
    fn model_selection_requires_configured_model_for_headless_execution() {
        let root = temp_dir("missing-model-selection");
        let plan = minimal_plan(&root);
        let err = model_selection(&plan).unwrap_err();
        assert!(err.to_string().contains("requires model.provider"));
    }

    #[test]
    fn failed_headless_terminal_status_includes_terminal_error_detail() {
        let terminal = RuntimeTerminalResult {
            status: HarnessTerminalStatus::Failed,
            output: Some(json!({ "error": "OPENAI_API_KEY is required for provider `openai`" })),
            report: minimal_run_report("run-1"),
        };
        let message =
            terminal_status_error_message(&terminal, HarnessTerminalStatus::Failed).unwrap();
        assert!(message.contains("terminal status Failed"));
        assert!(message.contains("OPENAI_API_KEY"));
    }

    #[test]
    fn model_capability_validation_rejects_missing_semantic_or_structured_support() {
        let runtime = UnsupportedModelRuntime {
            semantic_actions: false,
            structured_output: true,
        };
        let err = validate_model_capabilities(&runtime).unwrap_err();
        assert!(err.to_string().contains("semantic action support"));

        let runtime = UnsupportedModelRuntime {
            semantic_actions: true,
            structured_output: false,
        };
        let err = validate_model_capabilities(&runtime).unwrap_err();
        assert!(err.to_string().contains("structured output support"));
    }

    #[tokio::test]
    async fn headless_worker_constructs_blocking_provider_outside_tokio_runtime() {
        run_headless_worker(|| {
            let _runtime = BuiltInModelRuntime::from_selection(ModelProviderSelection {
                provider: "openai".into(),
                model: "gpt-4o-mini".into(),
                options: json!({}),
            })
            .map_err(|err| anyhow!(err.message))?;
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn headless_execution_runs_one_engine_run_and_writes_report() {
        let root = temp_dir("headless-exec");
        let loop_root = root.join(".agentpm/loops/zack/review-loop/0.1.0");
        write_json(
            &loop_root.join("agent.json"),
            json!({
                "kind": "loop",
                "name": "@zack/review-loop",
                "version": "0.1.0",
                "loop": {
                    "entry_phase": "respond",
                    "phases": [
                        { "id": "respond", "objective": "Respond to the request." }
                    ],
                    "transitions": [
                        { "from": "respond", "on": "complete", "to": "$end" }
                    ]
                }
            }),
        );
        let mut plan = minimal_plan(&root);
        plan.config.config.model = Some(crate::harness_config::HarnessModelConfig {
            provider: "ollama".into(),
            model: "test-model".into(),
            options: json!({}),
        });
        plan.config.config.trace = HarnessTraceConfig {
            enabled: true,
            level: HarnessTraceLevel::Verbose,
            content: HarnessTraceContent::Full,
        };
        plan.loop_package = Some(ResolvedPackageInfo {
            key: "loop:@zack/review-loop@0.1.0".into(),
            kind: PackageKind::Loop,
            name: "@zack/review-loop".into(),
            version: "0.1.0".into(),
            root: loop_root,
        });
        let report_path = root.join("custom-report.json");
        let mut model = ScriptedModelRuntime::new(vec![ModelTurn {
            assistant_content: Some("final response".into()),
            actions: Vec::new(),
            usage: RunUsage::default(),
            finish_reason: Some("stop".into()),
            provider_metadata: BTreeMap::new(),
        }]);
        let mut dispatcher = ScriptedActionDispatcher::default();
        let result = execute_headless_plan(
            &plan,
            "write a response".into(),
            Some(&report_path),
            &mut model,
            &mut dispatcher,
        )
        .unwrap();
        assert_eq!(result.status, HarnessTerminalStatus::Ended);
        assert_eq!(result.output, Some(json!("final response")));
        assert!(report_path.exists());
        let events_path = plan
            .state_dir
            .join("runs")
            .join(&result.report.run_id)
            .join("events.jsonl");
        assert_eq!(
            result.report.trace_path.as_deref(),
            Some(events_path.to_string_lossy().as_ref())
        );
        let report_json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&report_path).unwrap()).unwrap();
        assert_eq!(
            report_json["trace_path"],
            events_path.to_string_lossy().as_ref()
        );
        let events = fs::read_to_string(&events_path).unwrap();
        assert!(events.contains("\"event_type\":\"run_started\""));
        assert!(events.contains("\"event_type\":\"run_completed\""));
        let parsed_events = events
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect::<Vec<serde_json::Value>>();
        let prompt_event = parsed_events
            .iter()
            .find(|event: &&serde_json::Value| event["event_type"] == "prompt_prepared")
            .unwrap();
        let prompt = prompt_event["payload"]["fields"]["prompt"]
            .as_str()
            .unwrap();
        assert!(prompt.contains("Harness authority"));
        assert!(prompt.contains("write a response"));
        let model_completed = parsed_events
            .iter()
            .find(|event: &&serde_json::Value| event["event_type"] == "model_request_completed")
            .unwrap();
        assert_eq!(
            model_completed["payload"]["fields"]["assistant_content"],
            "final response"
        );
        assert_eq!(
            model_completed["payload"]["fields"]["finish_reason"],
            "stop"
        );
        let phase_result = parsed_events
            .iter()
            .find(|event: &&serde_json::Value| event["event_type"] == "phase_result_ready")
            .unwrap();
        assert_eq!(phase_result["payload"]["output"], "final response");
        assert_eq!(model.requests.len(), 1);
        assert_eq!(model.requests[0].prompt.sections.len(), 6);
    }

    #[test]
    fn headless_execution_runs_three_phase_loop_and_writes_report() {
        let root = temp_dir("headless-three-phase");
        let loop_root = root.join(".agentpm/loops/zack/review-loop/0.1.0");
        write_json(
            &loop_root.join("agent.json"),
            json!({
                "kind": "loop",
                "name": "@zack/review-loop",
                "version": "0.1.0",
                "loop": {
                    "entry_phase": "assess",
                    "phases": [
                        {
                            "id": "assess",
                            "objective": "Assess the request.",
                            "outcomes": [
                                { "id": "draft", "description": "Draft a response." }
                            ]
                        },
                        {
                            "id": "draft",
                            "objective": "Draft the response.",
                            "outcomes": [
                                { "id": "review", "description": "Review the response." }
                            ]
                        },
                        { "id": "review", "objective": "Review the response." }
                    ],
                    "transitions": [
                        { "from": "assess", "on": "draft", "to": "draft" },
                        { "from": "draft", "on": "review", "to": "review" },
                        { "from": "review", "on": "complete", "to": "$end" }
                    ]
                }
            }),
        );
        let mut plan = minimal_plan(&root);
        plan.config.config.model = Some(crate::harness_config::HarnessModelConfig {
            provider: "ollama".into(),
            model: "test-model".into(),
            options: json!({}),
        });
        plan.loop_package = Some(ResolvedPackageInfo {
            key: "loop:@zack/review-loop@0.1.0".into(),
            kind: PackageKind::Loop,
            name: "@zack/review-loop".into(),
            version: "0.1.0".into(),
            root: loop_root,
        });
        let mut model = ScriptedModelRuntime::new(vec![
            phase_completion_turn(Some("draft"), Some(json!({ "assessment": "ok" }))),
            phase_completion_turn(Some("review"), Some(json!({ "draft": "ready" }))),
            phase_completion_turn(None, Some(json!({ "final": "approved" }))),
        ]);
        let mut dispatcher = ScriptedActionDispatcher::default();
        let result = execute_headless_plan(
            &plan,
            "prepare a response".into(),
            None,
            &mut model,
            &mut dispatcher,
        )
        .unwrap();

        assert_eq!(result.status, HarnessTerminalStatus::Ended);
        assert_eq!(result.output, Some(json!({ "final": "approved" })));
        assert_eq!(result.report.phase_summaries.len(), 3);
        assert_eq!(model.requests.len(), 3);
    }

    #[test]
    fn headless_execution_reports_approval_required_terminal_status() {
        let root = temp_dir("headless-approval-required");
        let loop_root = root.join(".agentpm/loops/zack/review-loop/0.1.0");
        write_json(
            &loop_root.join("agent.json"),
            json!({
                "kind": "loop",
                "name": "@zack/review-loop",
                "version": "0.1.0",
                "loop": {
                    "entry_phase": "assess",
                    "checkpoints": [
                        {
                            "id": "approve-review",
                            "type": "approval",
                            "before_phase": "review",
                            "on_reject": "$handoff"
                        }
                    ],
                    "phases": [
                        {
                            "id": "assess",
                            "objective": "Assess the request.",
                            "outcomes": [
                                { "id": "review", "description": "Review the response." }
                            ]
                        },
                        { "id": "review", "objective": "Review the response." }
                    ],
                    "transitions": [
                        { "from": "assess", "on": "review", "to": "review" },
                        { "from": "review", "on": "complete", "to": "$end" }
                    ]
                }
            }),
        );
        let mut plan = minimal_plan(&root);
        plan.config.config.model = Some(crate::harness_config::HarnessModelConfig {
            provider: "ollama".into(),
            model: "test-model".into(),
            options: json!({}),
        });
        plan.loop_package = Some(ResolvedPackageInfo {
            key: "loop:@zack/review-loop@0.1.0".into(),
            kind: PackageKind::Loop,
            name: "@zack/review-loop".into(),
            version: "0.1.0".into(),
            root: loop_root,
        });
        let report_path = root.join("approval-report.json");
        let mut model = ScriptedModelRuntime::new(vec![phase_completion_turn(
            Some("review"),
            Some(json!({ "assessment": "needs review" })),
        )]);
        let mut dispatcher = ScriptedActionDispatcher::default();
        let result = execute_headless_plan(
            &plan,
            "prepare a response".into(),
            Some(&report_path),
            &mut model,
            &mut dispatcher,
        )
        .unwrap();

        assert_eq!(result.status, HarnessTerminalStatus::ApprovalRequired);
        assert!(report_path.exists());
        assert_eq!(model.requests.len(), 1);
    }

    fn phase_completion_turn(
        outcome: Option<&str>,
        output: Option<serde_json::Value>,
    ) -> ModelTurn {
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "complete",
                SemanticAction::PhaseCompletion {
                    outcome: outcome.map(str::to_string),
                    output,
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("stop".into()),
            provider_metadata: BTreeMap::new(),
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "agentpm-harness-command-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn minimal_run_report(run_id: &str) -> RunReport {
        RunReport {
            report_version: crate::harness_observability::HARNESS_REPORT_SCHEMA_VERSION,
            session_id: "session-1".into(),
            run_id: run_id.into(),
            agent: ReportPackageIdentity {
                name: "@zack/test-agent".into(),
                version: "0.1.0".into(),
            },
            loop_package: ReportPackageIdentity {
                name: "@zack/review-loop".into(),
                version: "0.1.0".into(),
            },
            started_at: chrono::Utc::now(),
            ended_at: None,
            duration_ms: None,
            terminal_status: HarnessTerminalStatus::Failed,
            terminal_output: None,
            preflight_status: PreflightStatus::Ready,
            diagnostics: Vec::new(),
            runtime: Default::default(),
            runtime_sources: BTreeMap::new(),
            consumer_context: None,
            scope_summaries: Vec::new(),
            phase_summaries: Vec::new(),
            checkpoint_summaries: Vec::new(),
            action_summaries: Vec::new(),
            tool_summaries: Vec::new(),
            mcp_summaries: Vec::new(),
            knowledge_summaries: Vec::new(),
            memory_summaries: Vec::new(),
            usage: RunUsage::default(),
            retry_count: 0,
            repair_count: 0,
            error_count: 0,
            approval_summary: BTreeMap::new(),
            cancellation_summary: BTreeMap::new(),
            trace_path: None,
        }
    }

    fn minimal_plan(root: &Path) -> ResolvedHarnessPlan {
        let config = HarnessConfig {
            version: 1,
            ..HarnessConfig::default()
        };
        ResolvedHarnessPlan {
            workspace_root: root.to_path_buf(),
            lock_path: root.join("agent.lock"),
            state_dir: root.join(".agentpm-state"),
            config: ResolvedHarnessConfig {
                workspace_root: root.to_path_buf(),
                config_path: None,
                config,
                state_dir: root.join(".agentpm-state"),
                state_dir_source: HarnessConfigSource::cli_override(),
            },
            selected_agent: Some(crate::harness_plan::ResolvedAgentRoot {
                root_key: "local:agent:agent.json".into(),
                name: "@zack/test-agent".into(),
                version: "0.1.0".into(),
                manifest_path: root.join("agent.json"),
                package_key: None,
                tools: Vec::new(),
                skills: Vec::new(),
                knowledge: Vec::new(),
                memory: Vec::new(),
                profiles: Vec::new(),
                loop_key: "loop:@zack/review-loop@0.1.0".into(),
            }),
            loop_package: None,
            package_graph: BTreeMap::new(),
            runtime_scopes: BTreeMap::new(),
            consumer_context: crate::harness_plan::ConsumerContextReadiness {
                state: CapabilityState::NotConfigured,
                file: None,
                path: None,
                byte_size: None,
                approximate_tokens: None,
                sha256: None,
            },
            profile_bindings: crate::harness_runtime::model::ProfileBindingSnapshot::default(),
            profiles: BTreeMap::new(),
            capabilities: Vec::new(),
            report: crate::harness_plan::PreflightReport {
                status: PreflightStatus::Ready,
                diagnostics: Vec::new(),
            },
        }
    }

    fn write_json(path: &Path, value: serde_json::Value) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    }

    struct UnsupportedModelRuntime {
        semantic_actions: bool,
        structured_output: bool,
    }

    impl ModelRuntime for UnsupportedModelRuntime {
        fn capabilities(&self) -> ModelCapabilityAdvertisement {
            ModelCapabilityAdvertisement {
                semantic_actions: self.semantic_actions,
                structured_output: self.structured_output,
                multimodal_input: false,
                context_window_tokens: None,
                usage_reporting: false,
            }
        }

        fn generate(
            &mut self,
            _request: crate::harness_runtime::ModelRequest,
        ) -> std::result::Result<ModelTurn, ModelRuntimeFailure> {
            unreachable!("capability validation should fail before generation")
        }
    }
}
