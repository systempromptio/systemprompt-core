//! Session management commands.

mod list;
pub mod login;
pub mod login_helpers;
mod logout;
mod show;
mod switch;
pub mod types;

use anyhow::Result;
use clap::Subcommand;

use crate::context::CommandContext;
use crate::descriptor::{CommandDescriptor, DescribeCommand};
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum SessionCommands {
    #[command(about = "Show current session and routing info", alias = "current")]
    Show,

    #[command(about = "Switch to a different profile")]
    Switch { profile_name: String },

    #[command(about = "List available profiles")]
    List,

    #[command(about = "Create an admin session for CLI access")]
    Login(login::LoginArgs),

    #[command(about = "Remove a session")]
    Logout(logout::LogoutArgs),
}

impl DescribeCommand for SessionCommands {
    fn descriptor(&self) -> CommandDescriptor {
        match self {
            Self::Login(_) => CommandDescriptor::PROFILE_SECRETS_AND_PATHS.with_skip_validation(),
            Self::Switch { .. } | Self::Show | Self::List | Self::Logout(_) => {
                CommandDescriptor::NONE
            },
        }
    }
}

pub async fn execute(cmd: SessionCommands, ctx: &CommandContext) -> Result<()> {
    match cmd {
        SessionCommands::Show => {
            let result = show::execute(&ctx.cli);
            render_result(&result, &ctx.cli);
            Ok(())
        },
        SessionCommands::Switch { profile_name } => {
            let result = switch::execute(&profile_name, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        SessionCommands::List => {
            let result = list::execute(&ctx.cli);
            render_result(&result, &ctx.cli);
            Ok(())
        },
        SessionCommands::Login(args) => {
            let result = login::execute(args, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        SessionCommands::Logout(ref args) => {
            let result = logout::execute(args, &ctx.cli)?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
    }
}
