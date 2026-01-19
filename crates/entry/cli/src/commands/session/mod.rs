//! Session management commands.

mod list;
mod show;
mod switch;

use anyhow::Result;
use clap::Subcommand;

use crate::cli_settings::CliConfig;

#[derive(Debug, Subcommand)]
pub enum SessionCommands {
    #[command(about = "Show current session and routing info")]
    Show,

    #[command(about = "Switch to a different profile")]
    Switch { profile_name: String },

    #[command(about = "List available profiles")]
    List,
}

pub fn execute(cmd: SessionCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        SessionCommands::Show => show::execute(config),
        SessionCommands::Switch { profile_name } => switch::execute(&profile_name, config),
        SessionCommands::List => {
            list::execute(config);
            Ok(())
        },
    }
}
