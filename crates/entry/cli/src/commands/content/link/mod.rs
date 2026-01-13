pub mod generate;
pub mod list;
pub mod performance;
pub mod show;

use crate::cli_settings::CliConfig;
use crate::shared::render_result;
use anyhow::{Context, Result};
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum LinkCommands {
    #[command(about = "Generate a trackable campaign link")]
    Generate(generate::GenerateArgs),

    #[command(about = "Show link details by short code")]
    Show(show::ShowArgs),

    #[command(about = "List links by campaign or content")]
    List(list::ListArgs),

    #[command(about = "Show link performance metrics")]
    Performance(performance::PerformanceArgs),
}

pub async fn execute(command: LinkCommands, config: &CliConfig) -> Result<()> {
    match command {
        LinkCommands::Generate(args) => {
            let result = generate::execute(args, config)
                .await
                .context("Failed to generate link")?;
            render_result(&result);
        },
        LinkCommands::Show(args) => {
            let result = show::execute(args, config)
                .await
                .context("Failed to show link")?;
            render_result(&result);
        },
        LinkCommands::List(args) => {
            let result = list::execute(args, config)
                .await
                .context("Failed to list links")?;
            render_result(&result);
        },
        LinkCommands::Performance(args) => {
            let result = performance::execute(args, config)
                .await
                .context("Failed to get link performance")?;
            render_result(&result);
        },
    }
    Ok(())
}
