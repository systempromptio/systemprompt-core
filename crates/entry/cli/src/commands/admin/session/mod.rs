//! Session management commands.

mod list;
mod login;
mod logout;
mod show;
mod switch;
pub mod types;

use anyhow::Result;
use clap::Subcommand;

use crate::cli_settings::CliConfig;
use crate::descriptor::{CommandDescriptor, DescribeCommand};
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum SessionCommands {
    #[command(about = "Show current session and routing info")]
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
            Self::Login(_) | Self::Switch { .. } => CommandDescriptor::PROFILE_AND_SECRETS,
            Self::Show | Self::List | Self::Logout(_) => CommandDescriptor::NONE,
        }
    }
}

pub async fn execute(cmd: SessionCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        SessionCommands::Show => {
            let result = show::execute(config)?;
            render_result(&result);
            Ok(())
        },
        SessionCommands::Switch { profile_name } => {
            let result = switch::execute(&profile_name, config).await?;
            render_result(&result);
            Ok(())
        },
        SessionCommands::List => {
            let result = list::execute(config)?;
            render_result(&result);
            Ok(())
        },
        SessionCommands::Login(args) => {
            let result = login::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        SessionCommands::Logout(ref args) => {
            let result = logout::execute(args, config)?;
            render_result(&result);
            Ok(())
        },
    }
}
