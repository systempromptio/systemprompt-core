pub mod types;

mod list;
mod show;
mod sync;
mod validate;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::cli_settings::get_global_config;
use crate::shared::render_result;
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum AgentsCommands {
    #[command(about = "List configured agents")]
    List(list::ListArgs),

    #[command(about = "Show agent details")]
    Show(show::ShowArgs),

    #[command(about = "Sync agents between disk and database")]
    Sync(sync::SyncArgs),

    #[command(about = "Validate agent configurations")]
    Validate(validate::ValidateArgs),
}

pub async fn execute(command: AgentsCommands) -> Result<()> {
    let config = get_global_config();
    execute_with_config(command, &config).await
}

pub async fn execute_with_config(command: AgentsCommands, config: &CliConfig) -> Result<()> {
    match command {
        AgentsCommands::List(args) => {
            let result = list::execute(args, config).context("Failed to list agents")?;
            render_result(&result);
            Ok(())
        },
        AgentsCommands::Show(args) => {
            let result = show::execute(&args, config).context("Failed to show agent")?;
            render_result(&result);
            Ok(())
        },
        AgentsCommands::Sync(args) => {
            let result = sync::execute(args, config)
                .await
                .context("Failed to sync agents")?;
            render_result(&result);
            Ok(())
        },
        AgentsCommands::Validate(args) => {
            let result = validate::execute(&args, config).context("Failed to validate agents")?;
            render_result(&result);
            Ok(())
        },
    }
}
