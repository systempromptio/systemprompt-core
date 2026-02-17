pub mod types;

mod list;
mod validate;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::cli_settings::get_global_config;
use crate::shared::render_result;

#[derive(Debug, Clone, Copy, Subcommand)]
pub enum HooksCommands {
    #[command(about = "List hooks across all plugins")]
    List(list::ListArgs),

    #[command(about = "Validate all hook definitions")]
    Validate(validate::ValidateArgs),
}

pub fn execute(command: HooksCommands) -> Result<()> {
    let config = get_global_config();

    match command {
        HooksCommands::List(args) => {
            let result = list::execute(args, &config).context("Failed to list hooks")?;
            render_result(&result);
            Ok(())
        },
        HooksCommands::Validate(args) => {
            let result = validate::execute(args, &config).context("Failed to validate hooks")?;
            render_result(&result);
            Ok(())
        },
    }
}
