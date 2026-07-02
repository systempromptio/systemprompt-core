//! `cloud auth` subcommands: login, logout, and whoami.
//!
//! Dispatches the [`AuthCommands`] enum to the per-command modules that manage
//! the locally stored cloud credentials and authenticated-user state.

mod login;
mod logout;
mod whoami;

pub use login::complete_login;

use crate::context::CommandContext;
use crate::shared::render_result;
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

    #[command(
        about = "Show current authenticated user and token status",
        alias = "status"
    )]
    Whoami,
}

#[derive(Debug, Clone, Copy, Args)]
pub struct LogoutArgs {
    #[arg(short = 'y', long, help = "Skip confirmation prompts")]
    pub yes: bool,
}

pub async fn execute(cmd: AuthCommands, ctx: &CommandContext) -> Result<()> {
    match cmd {
        AuthCommands::Login { environment } => {
            let result = login::execute(environment, ctx.prompter(), &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        AuthCommands::Logout(args) => {
            let result = logout::execute(args, ctx.prompter(), &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        AuthCommands::Whoami => {
            let result = whoami::execute(&ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
    }
}
