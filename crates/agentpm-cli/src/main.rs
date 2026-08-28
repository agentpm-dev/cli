use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

mod adapter;
mod assets;
mod auth;
mod commands;
mod config;
mod harness_config;
mod harness_engine;
mod harness_observability;
mod harness_plan;
mod harness_runtime;
mod io;
mod keys;
mod manifest;
mod prelude;
mod runner;
mod runtime_version;
mod semver;
mod ui;
mod workspace;
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
    #[arg(
        long,
        global = true,
        env = "AGENTPM_BASE_URL",
        default_value_t = default_base_url()
    )]
    base_url: String,

    #[command(subcommand)]
    command: commands::Commands,
}

fn default_base_url() -> String {
    if cfg!(debug_assertions) {
        "http://api.agentpackagemanager.local".into()
    } else {
        "https://api.agentpackagemanager.com".into()
    }
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
        commands::Commands::Lint(args) => args.run().await,
        commands::Commands::Knowledge(args) => args.run().await,
        commands::Commands::Memory(args) => args.run().await,
        commands::Commands::Publish(arg) => arg.run(cli.base_url.clone()).await,
        commands::Commands::Install(arg) => arg.run(cli.base_url.clone()).await,
        commands::Commands::New(arg) => arg.run(cli.base_url.clone()).await,
        commands::Commands::Run(arg) => arg.run(cli.base_url.clone()).await,
        commands::Commands::Serve(arg) => arg.run(cli.base_url.clone()).await,
        commands::Commands::Harness(arg) => arg.run(cli.base_url.clone()).await,
        commands::Commands::Export(arg) => arg.run(cli.base_url.clone()).await,
        commands::Commands::Keys(args) => args.run().await,
        commands::Commands::Namespace(args) => args.run(cli.base_url.clone()).await,
    }
}
