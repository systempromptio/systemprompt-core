//! Read-only inspection of web assets (CSS, fonts, images, favicons).
//!
//! Dispatches the `web assets` subcommands ([`AssetsCommands`]) to list the
//! asset inventory or show details for a single asset.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod asset_type;
pub mod list;
pub mod show;

use anyhow::{Context, Result};
use clap::Subcommand;

use systemprompt_extension::cli::{CliConfig, CommandOutput};
use std::path::Path;

#[derive(Debug, Subcommand)]
pub enum AssetsCommands {
    #[command(about = "List all assets")]
    List(list::ListArgs),

    #[command(about = "Show asset details")]
    Show(show::ShowArgs),
}

pub fn execute(command: AssetsCommands, config: &CliConfig, services_root: &Path) -> Result<CommandOutput> {
    match command {
        AssetsCommands::List(args) => {
            list::execute(args, config, services_root).context("Failed to list assets")
        },
        AssetsCommands::Show(args) => {
            show::execute(&args, config, services_root).context("Failed to show asset")
        },
    }
}
