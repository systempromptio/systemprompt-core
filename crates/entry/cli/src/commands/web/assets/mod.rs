//! Read-only inspection of web assets (CSS, fonts, images, favicons).
//!
//! Dispatches the `web assets` subcommands ([`AssetsCommands`]) to list the
//! asset inventory or show details for a single asset.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod asset_type;
pub mod list;
mod show;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::CliConfig;
use crate::shared::render_result;

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
            render_result(&result, config);
            Ok(())
        },
        AssetsCommands::Show(args) => {
            let result = show::execute(&args, config).context("Failed to show asset")?;
            render_result(&result, config);
            Ok(())
        },
    }
}
