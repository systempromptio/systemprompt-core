mod assign;
mod demote;
mod promote;

use crate::cli_settings::CliConfig;
use anyhow::Result;
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum RoleCommands {
    #[command(about = "Assign roles to a user")]
    Assign(assign::AssignArgs),

    #[command(about = "Promote user to admin")]
    Promote(promote::PromoteArgs),

    #[command(about = "Demote user from admin")]
    Demote(demote::DemoteArgs),
}

pub async fn execute(cmd: RoleCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        RoleCommands::Assign(args) => assign::execute(args, config).await,
        RoleCommands::Promote(args) => promote::execute(args, config).await,
        RoleCommands::Demote(args) => demote::execute(args, config).await,
    }
}
