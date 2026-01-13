pub mod types;

mod create;
mod delete;
mod edit;
mod list;
mod show;
mod status;
mod validate;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::cli_settings::get_global_config;
use crate::shared::render_result;
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum AgentsCommands {
    #[command(about = "List all agents from config")]
    List(list::ListArgs),

    #[command(about = "Show agent config details")]
    Show(show::ShowArgs),

    #[command(about = "Validate agent config(s)")]
    Validate(validate::ValidateArgs),

    #[command(about = "Create new agent config")]
    Create(create::CreateArgs),

    #[command(about = "Edit agent config")]
    Edit(edit::EditArgs),

    #[command(about = "Delete agent config")]
    Delete(delete::DeleteArgs),

    #[command(about = "Check if agent service is running")]
    Status(status::StatusArgs),
}

pub async fn execute(command: AgentsCommands) -> Result<()> {
    let config = get_global_config();
    execute_with_config(command, &config).await
}

pub async fn execute_with_config(command: AgentsCommands, config: &CliConfig) -> Result<()> {
    match command {
        AgentsCommands::List(args) => {
            let result = list::execute(args, config)
                .await
                .context("Failed to list agents")?;
            render_result(&result);
            Ok(())
        }
        AgentsCommands::Show(args) => {
            let result = show::execute(args, config)
                .await
                .context("Failed to show agent")?;
            render_result(&result);
            Ok(())
        }
        AgentsCommands::Validate(args) => {
            let result = validate::execute(args, config)
                .await
                .context("Failed to validate agents")?;
            render_result(&result);
            Ok(())
        }
        AgentsCommands::Create(args) => {
            let result = create::execute(args, config)
                .await
                .context("Failed to create agent")?;
            render_result(&result);
            Ok(())
        }
        AgentsCommands::Edit(args) => {
            let result = edit::execute(args, config)
                .await
                .context("Failed to edit agent")?;
            render_result(&result);
            Ok(())
        }
        AgentsCommands::Delete(args) => {
            let result = delete::execute(args, config)
                .await
                .context("Failed to delete agent")?;
            render_result(&result);
            Ok(())
        }
        AgentsCommands::Status(args) => {
            let result = status::execute(args, config)
                .await
                .context("Failed to get agent status")?;
            render_result(&result);
            Ok(())
        }
    }
}
