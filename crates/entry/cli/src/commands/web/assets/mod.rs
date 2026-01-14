mod list;
mod show;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::shared::render_result;
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum AssetsCommands {
    #[command(about = "List all assets")]
    List(list::ListArgs),

    #[command(about = "Show asset details")]
    Show(show::ShowArgs),
}

pub fn execute(command: AssetsCommands, config: &CliConfig) -> Result<()> {
    match command {
        AssetsCommands::List(args) => {
            let result = list::execute(args, config).context("Failed to list assets")?;
            render_result(&result);
            Ok(())
        },
        AssetsCommands::Show(args) => {
            let result = show::execute(&args, config).context("Failed to show asset")?;
            render_result(&result);
            Ok(())
        },
    }
}
