mod count;
mod list;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::shared::render_result;
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum AiCommands {
    #[command(about = "List AI-generated images")]
    List(list::ListArgs),

    #[command(about = "Count AI-generated images")]
    Count(count::CountArgs),
}

pub async fn execute(cmd: AiCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        AiCommands::List(args) => {
            let result = list::execute(args, config)
                .await
                .context("Failed to list AI images")?;
            render_result(&result);
            Ok(())
        },
        AiCommands::Count(args) => {
            let result = count::execute(args, config)
                .await
                .context("Failed to count AI images")?;
            render_result(&result);
            Ok(())
        },
    }
}
