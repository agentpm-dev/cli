use crate::prelude::*;
pub mod init;
pub mod login;
pub mod whoami;

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Show current identity
    Whoami(whoami::WhoAmIArgs),

    /// Log in and cache credentials
    Login(login::LoginArgs),

    /// Scaffold agent.json (tool or agent)
    Init(init::InitArgs),
}
