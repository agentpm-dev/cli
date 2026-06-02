use crate::prelude::*;
pub mod export;
pub mod init;
pub mod install;
pub mod keys;
pub mod lint;
pub mod login;
pub mod namespace;
pub mod new;
pub mod publish;
pub mod run;
pub mod serve;
pub mod whoami;

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Show current identity
    Whoami(whoami::WhoAmIArgs),

    /// Log in and cache credentials
    Login(login::LoginArgs),

    /// Scaffold agent.json (tool, agent, or template)
    Init(init::InitArgs),

    /// Lint agent.json (tool, agent, or template)
    Lint(lint::LintArgs),

    /// Publish a tool to the registry
    Publish(publish::PublishArgs),

    /// Install tool(s) from the registry
    Install(install::InstallArgs),

    /// Generate a project from a published or local workflow template
    New(new::NewArgs),

    /// Execute an installed tool with JSON input
    Run(run::RunArgs),

    /// Expose installed tools through a local MCP server
    Serve(serve::ServeArgs),

    /// Generate a starter skill scaffold from an installed tool
    Export(export::ExportArgs),

    /// Manage local signing keys
    Keys(keys::KeysArgs),

    /// Manage namespace signers (server-side)
    Namespace(namespace::NamespaceArgs),
}
