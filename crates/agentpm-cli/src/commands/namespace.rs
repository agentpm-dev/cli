use crate::prelude::*;
use anyhow::anyhow;
use std::fs;
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
        match self.command {
            NamespaceCmd::AddSigner { namespace, label, pubkey, token } => {
                let cfg = Config::load(base_url.clone())?;
                let token = resolve_token(&cfg, token)?;
                let mut client = AgentPmClient::new(base_url)?;
                if let Some(t) = token {
                    client = client.with_token(t);
                } else {
                    println!(
                        "Not authenticated. Set AGENTPM_TOKEN or pass --token, or run `agentpm login --paste`."
                    );
                    return Ok(());
                }

                let input = match pubkey {
                    Some(p) => fs::read_to_string(&p)
                        .with_context(|| format!("reading {}", p.display()))?,
                    None => {
                        use std::io::{self, Read};
                        let mut buf = String::new();
                        io::stdin().read_to_string(&mut buf)?;
                        buf
                    }
                };

                match client.create_namespace_signer(namespace, label.clone(), input.trim()).await {
                    Ok(..) => {
                        println!("✅  Added signer \"{}\"", label);
                    }
                    Err(SdkError::Unauthorized) => eprintln!("Unauthorized (401)."),
                    Err(e) => return Err(anyhow!(e)),
                }
            }

            NamespaceCmd::RevokeSigner { namespace, signer_id, token } => {
                let cfg = Config::load(base_url.clone())?;
                let token = resolve_token(&cfg, token)?;
                let mut client = AgentPmClient::new(base_url)?;
                if let Some(t) = token {
                    client = client.with_token(t);
                } else {
                    println!(
                        "Not authenticated. Set AGENTPM_TOKEN or pass --token, or run `agentpm login --paste`."
                    );
                    return Ok(());
                }

                match client.revoke_namespace_signer(namespace, signer_id.clone()).await {
                    Ok(_) => println!("✅  Revoked signer {}", signer_id),
                    Err(SdkError::NotFound) => eprintln!("Signer not found."),
                    Err(SdkError::Unauthorized) => eprintln!("Unauthorized (401)."),
                    Err(e) => return Err(anyhow!(e)),
                }
            }
        }
        Ok(())
    }
}
