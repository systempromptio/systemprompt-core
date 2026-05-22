mod assign;
mod demote;
mod promote;

use crate::cli_settings::CliConfig;
use crate::shared::render_result;
use anyhow::{Result, bail};
use clap::Subcommand;
use systemprompt_database::DbPool;

#[derive(Debug, Subcommand)]
pub enum RoleCommands {
    #[command(about = "Assign roles to a user")]
    Assign(assign::AssignArgs),

    #[command(
        about = "Promote a user to the admin role",
        long_about = "Promote a user to the admin role.\n\nThis command operates only on the \
                      built-in 'admin' role. To assign other roles use:\n  systemprompt admin \
                      users role assign <USER_ID> --roles <ROLE>..."
    )]
    Promote(promote::PromoteArgs),

    #[command(
        about = "Demote a user from the admin role",
        long_about = "Demote a user from the admin role.\n\nThis command operates only on the \
                      built-in 'admin' role. To revoke other roles use:\n  systemprompt admin \
                      users role assign <USER_ID> --roles <ROLE>..."
    )]
    Demote(demote::DemoteArgs),
}

pub async fn execute(cmd: RoleCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        RoleCommands::Assign(args) => {
            let result = assign::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        RoleCommands::Promote(args) => {
            let result = promote::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        RoleCommands::Demote(args) => {
            let result = demote::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
    }
}

pub fn execute_with_pool(_cmd: RoleCommands, _pool: &DbPool, _config: &CliConfig) -> Result<()> {
    bail!("Role management operations require full profile context")
}
