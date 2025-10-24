use crate::prelude::*;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct NamespaceArgs {
    #[command(subcommand)]
    pub command: NamespaceCmd,
}

#[derive(Subcommand, Debug)]
pub enum NamespaceCmd {
    /// Add a signer (public key) to a namespace
    AddSigner {
        #[arg(long)]
        namespace: String,
        #[arg(long)]
        label: String,
        /// Path to a file containing the public key (SSH, PEM, or raw base64).
        /// If omitted, read from stdin.
        #[arg(long)]
        pubkey: Option<PathBuf>,

        /// Personal Access Token (overrides env/file)
        #[arg(long, value_name = "PAT", env = "AGENTPM_TOKEN")]
        token: Option<String>,
    },

    /// Revoke/deactivate a signer by id
    RevokeSigner {
        #[arg(long)]
        namespace: String,
        #[arg(long, value_name = "UUID")]
        signer_id: String,

        /// Personal Access Token (overrides env/file)
        #[arg(long, value_name = "PAT", env = "AGENTPM_TOKEN")]
        token: Option<String>,
    },
}

impl NamespaceArgs {
    pub async fn run(self, base_url: String) -> Result<()> {
        println!("{}", base_url.clone());
        Ok(())
    }
}
