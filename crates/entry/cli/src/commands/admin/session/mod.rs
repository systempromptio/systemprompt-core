//! Session management commands.

mod list;
mod login;
mod show;
mod switch;

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
}

impl DescribeCommand for SessionCommands {
    fn descriptor(&self) -> CommandDescriptor {
        match self {
            Self::Login(_) => CommandDescriptor::PROFILE_AND_SECRETS,
            Self::Show | Self::Switch { .. } | Self::List => CommandDescriptor::NONE,
        }
    }
}

pub async fn execute(cmd: SessionCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        SessionCommands::Show => show::execute(config),
        SessionCommands::Switch { profile_name } => switch::execute(&profile_name, config),
        SessionCommands::List => {
            list::execute(config);
            Ok(())
        },
        SessionCommands::Login(args) => {
            let result = login::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
    }
}
