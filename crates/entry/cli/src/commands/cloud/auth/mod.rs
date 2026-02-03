mod login;
mod logout;
mod whoami;

use crate::cli_settings::CliConfig;
use anyhow::Result;
use clap::{Args, Subcommand};

use super::Environment;

#[derive(Debug, Clone, Copy, Subcommand)]
pub enum AuthCommands {
    #[command(about = "Authenticate with systemprompt.io Cloud via OAuth")]
    Login {
        #[arg(value_enum, default_value_t = Environment::default(), hide = true)]
        environment: Environment,
    },

    #[command(about = "Clear saved cloud credentials")]
    Logout(LogoutArgs),

    #[command(about = "Show current authenticated user and token status")]
    Whoami,
}

#[derive(Debug, Clone, Copy, Args)]
pub struct LogoutArgs {
    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
}

pub async fn execute(cmd: AuthCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        AuthCommands::Login { environment } => login::execute(environment, config).await,
        AuthCommands::Logout(args) => logout::execute(args, config).await,
        AuthCommands::Whoami => whoami::execute().await,
    }
}
