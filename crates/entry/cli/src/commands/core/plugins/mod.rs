pub mod types;

mod generate;
mod list;
mod show;
mod validate;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::cli_settings::get_global_config;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum PluginsCommands {
    #[command(about = "List configured plugins")]
    List(list::ListArgs),

    #[command(about = "Show plugin details")]
    Show(show::ShowArgs),

    #[command(about = "Validate plugin configuration")]
    Validate(validate::ValidateArgs),

    #[command(about = "Generate Claude Code plugin output")]
    Generate(generate::GenerateArgs),
}

pub fn execute(command: PluginsCommands) -> Result<()> {
    let config = get_global_config();

    match command {
        PluginsCommands::List(args) => {
            let result = list::execute(args, &config).context("Failed to list plugins")?;
            render_result(&result);
            Ok(())
        },
        PluginsCommands::Show(args) => {
            let result = show::execute(&args, &config).context("Failed to show plugin")?;
            render_result(&result);
            Ok(())
        },
        PluginsCommands::Validate(args) => {
            let result = validate::execute(args, &config).context("Failed to validate plugins")?;
            render_result(&result);
            Ok(())
        },
        PluginsCommands::Generate(args) => {
            let result = generate::execute(&args, &config).context("Failed to generate plugins")?;
            render_result(&result);
            Ok(())
        },
    }
}
