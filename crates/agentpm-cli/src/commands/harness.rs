use crate::harness_plan::{
    CapabilityState, HarnessBootstrapOptions, HarnessExecutionSurface, PreflightDiagnosticSeverity,
    PreflightStatus, ResolvedHarnessPlan, resolve_harness_plan,
};
use crate::prelude::*;
use anyhow::{anyhow, bail};
use std::collections::BTreeMap;
use std::path::PathBuf;

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
}

impl HarnessArgs {
    pub async fn run(self, _base_url: String) -> Result<()> {
        let workspace_root = std::env::current_dir().context("reading current directory")?;
        let surface = self.surface();
        let plan = resolve_harness_plan(
            &workspace_root,
            &HarnessBootstrapOptions {
                agent_selector: self.agent.clone(),
                config_path: self.config.clone(),
                state_dir_override: self.state_dir.clone(),
                runtime_scopes: self.scopes.into_iter().collect(),
                surface,
            },
        )?;

        if self.json {
            println!("{}", serde_json::to_string_pretty(&plan.report)?);
        } else {
            print_harness_preflight(&plan);
        }

        match plan.report.status {
            PreflightStatus::Ready | PreflightStatus::ReadyWithWarnings => Ok(()),
            PreflightStatus::SelectionRequired => {
                bail!("Harness preflight requires an explicit Agent selector")
            }
            PreflightStatus::Failed => bail!("Harness preflight failed"),
        }
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

fn parse_scope(raw: &str) -> Result<(String, String)> {
    let Some((key, value)) = raw.split_once('=') else {
        return Err(anyhow!("scope must be KEY=VALUE"));
    };
    if key.is_empty() || value.is_empty() {
        return Err(anyhow!("scope key and value must be non-empty"));
    }
    Ok((key.to_string(), value.to_string()))
}

fn print_harness_preflight(plan: &ResolvedHarnessPlan) {
    println!("AgentPM Harness preflight");
    println!("Workspace: {}", plan.workspace_root.display());
    println!("Lockfile: {}", plan.lock_path.display());
    println!("State dir: {}", plan.state_dir.display());
    if let Some(config_path) = &plan.config.config_path {
        println!("Config: {}", config_path.display());
    } else {
        println!("Config: defaults");
    }
    if let Some(agent) = &plan.selected_agent {
        println!("Agent: {}@{}", agent.name, agent.version);
    }
    if let Some(loop_package) = &plan.loop_package {
        println!("Loop: {}@{}", loop_package.name, loop_package.version);
    }
    println!("Resolved packages: {}", plan.package_graph.len());
    println!("Runtime scopes: {}", plan.runtime_scopes.len());
    match &plan.consumer_context.file {
        Some(file) => println!(
            "Consumer context: {file} ({:?})",
            plan.consumer_context.state
        ),
        None => println!("Consumer context: not configured"),
    }

    let capability_counts = capability_counts(plan);
    if !capability_counts.is_empty() {
        println!();
        println!("Static capabilities:");
        for (state, count) in capability_counts {
            println!("- {state}: {count}");
        }
    }

    if !plan.report.diagnostics.is_empty() {
        println!();
        println!("Diagnostics:");
        for diagnostic in &plan.report.diagnostics {
            let severity = match diagnostic.severity {
                PreflightDiagnosticSeverity::Fatal => "fatal",
                PreflightDiagnosticSeverity::Warning => "warning",
                PreflightDiagnosticSeverity::Suppressed => "suppressed",
                PreflightDiagnosticSeverity::Pending => "pending",
                PreflightDiagnosticSeverity::Info => "info",
            };
            if let Some(path) = &diagnostic.path {
                println!(
                    "- [{severity}] {} ({path}) — {}",
                    diagnostic.code, diagnostic.message
                );
            } else {
                println!(
                    "- [{severity}] {} — {}",
                    diagnostic.code, diagnostic.message
                );
            }
        }
    }

    println!();
    println!("Status: {:?}", plan.report.status);
    println!(
        "Execution is not started in this milestone; this command currently performs bootstrap preflight only."
    );
}

fn capability_counts(plan: &ResolvedHarnessPlan) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for capability in &plan.capabilities {
        let key = match capability.state {
            CapabilityState::Available => "available",
            CapabilityState::Pending => "pending",
            CapabilityState::Unavailable => "unavailable",
            CapabilityState::Suppressed => "suppressed",
            CapabilityState::NotConfigured => "not_configured",
        };
        *counts.entry(key).or_insert(0) += 1;
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
