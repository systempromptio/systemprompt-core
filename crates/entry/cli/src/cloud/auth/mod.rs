//! Authentication commands for `SystemPrompt` Cloud
//!
//! Handles login, logout, and user identity.

mod login;
mod logout;
mod whoami;

use anyhow::Result;
use clap::Subcommand;

use super::Environment;

#[derive(Subcommand)]
pub enum AuthCommands {
    #[command(about = "Authenticate with SystemPrompt Cloud via OAuth")]
    Login {
        #[arg(value_enum, default_value_t = Environment::default())]
        environment: Environment,
    },

    #[command(about = "Clear saved cloud credentials")]
    Logout,

    #[command(about = "Show current authenticated user and token status")]
    Whoami,
}

pub async fn execute(cmd: AuthCommands) -> Result<()> {
    match cmd {
        AuthCommands::Login { environment } => login::execute(environment).await,
        AuthCommands::Logout => logout::execute(),
        AuthCommands::Whoami => whoami::execute().await,
    }
}
