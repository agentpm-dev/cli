use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod commands;
mod config;
mod auth;
mod prelude;
mod assets;
mod io;
/*
TODO:
Try: AGENTPM_BASE_URL=http://127.0.0.1:8080 (or whatever you’ll run locally), or

Add a hosts entry if you want a pretty domain:

127.0.0.1 api.agentpackagemanager.local
(on macOS: edit /etc/hosts, then sudo dscacheutil -flushcache; sudo killall -HUP mDNSResponder)
 */

#[derive(Parser)]
#[command(name = "agentpm", version, about = "AgentPM CLI")]
struct Cli {
    /// API base URL (env: AGENTPM_BASE_URL)
    #[arg(long, global = true, env = "AGENTPM_BASE_URL", default_value = "https://api.agentpackagemanager.local")]
    base_url: String,

    #[command(subcommand)]
    command: commands::Commands,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt::layer())
        .init();

    let cli = Cli::parse();
    match cli.command {
        commands::Commands::Whoami(args) => args.run(cli.base_url.clone()).await,
        commands::Commands::Login(args) => args.run(cli.base_url.clone()).await,
        commands::Commands::Init(args) => args.run(cli.base_url.clone()).await,
    }
}
