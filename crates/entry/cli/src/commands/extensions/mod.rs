pub mod types;

mod capabilities;
mod config;
mod list;
mod show;
mod validate;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::shared::render_result;
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum ExtensionsCommands {
    #[command(about = "List all discovered extensions")]
    List(list::ListArgs),

    #[command(about = "Show detailed extension information")]
    Show(show::ShowArgs),

    #[command(about = "Validate extension dependencies and configurations")]
    Validate(validate::ValidateArgs),

    #[command(about = "Show extension configuration")]
    Config(config::ConfigArgs),

    #[command(subcommand, about = "List capabilities across all extensions")]
    Capabilities(capabilities::CapabilitiesCommands),
}

pub fn execute(cmd: ExtensionsCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        ExtensionsCommands::List(args) => {
            let result = list::execute(args, config).context("Failed to list extensions")?;
            render_result(&result);
            Ok(())
        },
        ExtensionsCommands::Show(args) => {
            let result = show::execute(args, config).context("Failed to show extension")?;
            render_result(&result);
            Ok(())
        },
        ExtensionsCommands::Validate(args) => {
            let result = validate::execute(args, config).context("Failed to validate extensions")?;
            render_result(&result);
            Ok(())
        },
        ExtensionsCommands::Config(args) => {
            let result = config::execute(args, config).context("Failed to get extension config")?;
            render_result(&result);
            Ok(())
        },
        ExtensionsCommands::Capabilities(cmd) => capabilities::execute(cmd, config),
    }
}
