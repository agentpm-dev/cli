use crate::manifest::{
    load_manifest_value, read_lock_or_default, write_lock, write_manifest_pretty_atomic,
};
use crate::prelude::*;
use crate::semver::adapt::{plan_to_sdk_resolve, to_sdk_request};
use crate::semver::types::{DesiredSet, ResolvePlan, lock_from_plan};
use crate::semver::update::maybe_update_agent_json;
use crate::{io::download::download_and_extract_all, io::fs, ui::Step};
use anyhow::anyhow;
use serde_json::Value;
use std::io::IsTerminal;
use std::path::PathBuf;

#[derive(clap::Parser, Debug, Clone)]
pub struct InstallArgs {
    // Path to the manifest (agent.json)
    #[arg(long, default_value = "agent.json")]
    pub manifest: String,

    /// Optional spec like @owner/name@^1.2
    pub spec: Option<String>,

    /// Fail if lockfile would change (no re-resolution)
    #[clap(long)]
    pub frozen: bool,

    /// Re-resolve and re-download even if the lock looks satisfied
    #[clap(long)]
    pub refresh: bool,

    /// Update range in agent.json if spec conflicts
    #[clap(long)]
    pub update_range: bool,

    /// Reduce output
    #[clap(long)]
    pub quiet: bool,

    /// Personal Access Token for headless auth (overrides env/file)
    #[arg(long, value_name = "PAT", env = "AGENTPM_TOKEN")]
    pub token: Option<String>,
}

impl InstallArgs {
    pub async fn run(&self, base_url: String) -> Result<()> {
        let cfg = Config::load(base_url.clone())?;

        // Quiet if the user asked OR stderr is not a terminal (piped/redirected)
        let auto_quiet = !std::io::stderr().is_terminal();
        let quiet = self.quiet || auto_quiet;

        // 0) Load PAT (login is not required, but send in incase private repo; for MVP we use cached token)
        let mut s = Step::new("Reading credentials", quiet);
        let token = resolve_token(&cfg, self.token.clone())?;
        if let Some(t) = &token {
            s.ok(format!("using {}", mask_token(t)));
        } else {
            s.ok("none (public installs only)");
        }

        let mut client = AgentPmClient::new(base_url.clone())?;
        if let Some(ref tok) = token {
            client = client.with_token(tok.clone());
        }

        // 1) Load agent.json (must be kind=agent)
        let manifest_path = PathBuf::from(&self.manifest);
        let (mut manifest_value, _raw) = load_manifest_value(&manifest_path)
            .with_context(|| format!("loading {}", manifest_path.display()))?;

        if manifest_value.get("kind").and_then(Value::as_str) != Some("agent") {
            return Err(anyhow!(
                "`agentpm install` currently supports kind=agent only."
            ));
        }

        // 2) Determine desired set
        let desired = DesiredSet::from_cli_or_agent_json(
            &manifest_value,
            self.spec.as_deref(),
            self.update_range,
        )?;

        // 3) If --frozen and lock exists, use it directly; else resolve
        let lock = read_lock_or_default(".")?;
        let steps_label = if self.frozen {
            "Validating lockfile"
        } else {
            "Resolving versions"
        };
        let mut s = Step::new(steps_label, self.quiet);

        let plan = if self.frozen {
            ResolvePlan::from_lock(&desired, &lock)?
        } else {
            let sdk_req = to_sdk_request(&desired);
            let sdk_resp = client.resolve_install(&sdk_req).await?; // network call → concrete versions + integrity
            let plan: ResolvePlan = sdk_resp.into();
            plan
        };
        s.ok("");

        // 4) Init download session → presigned URLs
        let mut s = Step::new("Requesting download URLs", self.quiet);
        let init = client.install_init(&plan_to_sdk_resolve(&plan)).await?; // includes per-artifact presigned URL + expected hash
        s.ok("");

        // 5) Download + verify + extract (parallel)
        let mut s = Step::new("Downloading tools", self.quiet);
        let dl_dir = PathBuf::from(".agentpm/cache");
        let tools_dir = PathBuf::from(".agentpm/tools");
        fs::ensure_dirs(&[&dl_dir, &tools_dir])?;
        download_and_extract_all(&init, &dl_dir, &tools_dir, self.refresh).await?;
        s.ok("");

        // 6) Finalize (report success for metrics / server-side bookkeeping)
        let mut s = Step::new("Finalizing install", self.quiet);
        client.install_finalize(&init.session_id).await?;
        s.ok("");

        // 7) Update lockfile (unless --frozen)
        if !self.frozen {
            write_lock(".", &lock_from_plan(&plan))?;
        }

        // 8) If single-spec and not present in agent.json, append or update range if allowed
        if let Some(spec) = &self.spec
            && maybe_update_agent_json(&mut manifest_value, spec, self.update_range)?
        {
            write_manifest_pretty_atomic(&manifest_path, &manifest_value)?;
        }

        Step::final_msg("Installed ✓", self.quiet);

        Ok(())
    }
}
