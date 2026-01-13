pub mod types;

mod create;
mod delete;
mod edit;
mod list;
mod status;
mod sync;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::cli_settings::get_global_config;
use crate::shared::render_result;
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum SkillsCommands {
    #[command(about = "List configured skills")]
    List(list::ListArgs),

    #[command(about = "Create new skill")]
    Create(create::CreateArgs),

    #[command(about = "Edit skill configuration")]
    Edit(edit::EditArgs),

    #[command(about = "Delete a skill")]
    Delete(delete::DeleteArgs),

    #[command(about = "Show database sync status")]
    Status(status::StatusArgs),

    #[command(about = "Sync skills between disk and database")]
    Sync(sync::SyncArgs),
}

pub async fn execute(command: SkillsCommands) -> Result<()> {
    let config = get_global_config();
    execute_with_config(command, &config).await
}

pub async fn execute_with_config(command: SkillsCommands, config: &CliConfig) -> Result<()> {
    match command {
        SkillsCommands::List(args) => {
            let result = list::execute(args, config)
                .await
                .context("Failed to list skills")?;
            render_result(&result);
            Ok(())
        }
        SkillsCommands::Create(args) => {
            let result = create::execute(args, config)
                .await
                .context("Failed to create skill")?;
            render_result(&result);
            Ok(())
        }
        SkillsCommands::Edit(args) => {
            let result = edit::execute(args, config)
                .await
                .context("Failed to edit skill")?;
            render_result(&result);
            Ok(())
        }
        SkillsCommands::Delete(args) => {
            let result = delete::execute(args, config)
                .await
                .context("Failed to delete skill")?;
            render_result(&result);
            Ok(())
        }
        SkillsCommands::Status(args) => {
            let result = status::execute(args, config)
                .await
                .context("Failed to get skill status")?;
            render_result(&result);
            Ok(())
        }
        SkillsCommands::Sync(args) => {
            let result = sync::execute(args, config)
                .await
                .context("Failed to sync skills")?;
            render_result(&result);
            Ok(())
        }
    }
}
