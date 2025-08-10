use crate::auth;
use crate::config::Config;
use crate::prelude::*;

#[derive(Args, Debug, Default)]
pub struct LoginArgs {
    // Later: flags like --api-key, --device, --org, etc.
}

impl LoginArgs {
    pub async fn run(self, base_url: String) -> Result<()> {
        let cfg = Config::load(base_url)?;
        auth::login_stub(&cfg).await?;
        println!(
            "Logged in (stub). Token written to: {}",
            cfg.token_file.display()
        );
        Ok(())
    }
}
