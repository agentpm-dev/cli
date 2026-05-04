use crate::prelude::*;
pub mod init;
pub mod install;
pub mod keys;
pub mod lint;
pub mod login;
pub mod namespace;
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

    /// Scaffold agent.json (tool or agent)
    Init(init::InitArgs),

    /// Lint agent.json (tool or agent)
    Lint(lint::LintArgs),

    /// Publish a tool to the registry
    Publish(publish::PublishArgs),

    /// Install tool(s) from the registry
    Install(install::InstallArgs),

    /// Execute an installed tool with JSON input
    Run(run::RunArgs),

    /// Expose installed tools through a local MCP server
    Serve(serve::ServeArgs),

    /// Manage local signing keys
    Keys(keys::KeysArgs),

    /// Manage namespace signers (server-side)
    Namespace(namespace::NamespaceArgs),
}
