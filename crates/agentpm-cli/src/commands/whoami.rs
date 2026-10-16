use crate::prelude::*;

#[derive(Args, Debug, Default)]
pub struct WhoAmIArgs {
    /// Personal Access Token for headless auth (overrides env/file)
    #[arg(long, value_name = "PAT", env = "AGENTPM_TOKEN")]
    pub token: Option<String>,
}

impl WhoAmIArgs {
    pub async fn run(self, base_url: String) -> Result<()> {
        // Load merged config (defaults/file/flag)
        let cfg = Config::load(base_url)?;

        let token = resolve_token(&cfg, self.token.clone())?;

        let mut client = AgentPmClient::new(cfg.base_url.clone())?;
        if let Some(t) = token.clone() {
            client = client.with_token(t);
        } else {
            // No token → clear message and exit 0 (like docker/podman do)
            println!(
                "Not authenticated. Set AGENTPM_TOKEN or pass --token, or run `agentpm login --paste`."
            );
            return Ok(());
        }

        let who = client.whoami().await?;
        println!(
            "✅  Logged in as {} ({:?})",
            who.email,
            who.scopes.unwrap_or_else(Vec::new),
        );
        Ok(())
    }
}
