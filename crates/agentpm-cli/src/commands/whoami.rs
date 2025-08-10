use crate::prelude::*;

#[derive(Args, Debug, Default)]
pub struct WhoAmIArgs {}

impl WhoAmIArgs {
    pub async fn run(self, base_url: String) -> Result<()> {
        // TODO: Next step when you’re ready: wire whoami to actually use the token from auth::read_token and call a real GET /whoami in the SDK.

        // Load merged config (defaults/file/flag)
        let cfg = Config::load(base_url)?;

        // (Optional) read cached token to show how commands can access it
        if let Some(_tok) = read_token(&cfg)? {
            debug!("using cached token");
        }

        let client = AgentPmClient::new(cfg.base_url.clone())?;

        match client.whoami().await {
            Ok(me) => {
                println!("{me}");
            }
            Err(SdkError::Unauthorized) => {
                eprintln!("Not authorized. Try: `agentpm login`.");
            }
            Err(SdkError::Http(e)) if e.is_connect() => {
                eprintln!(
                    "Can’t connect to {}. Check DNS/hosts or server is running.\n{e}",
                    cfg.base_url
                );
            }
            Err(SdkError::Http(e)) if e.is_timeout() => {
                eprintln!(
                    "Request to {} timed out. Is the server reachable?\n{e}",
                    cfg.base_url
                );
            }
            Err(e) => {
                eprintln!("{e}");
            }
        }
        Ok(())
    }
}
