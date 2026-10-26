//! `admin config catalog` — read-only views of the model catalog beyond the
//! per-object editors (`admin config credential|provider|endpoint|model|
//! deployment`): what Vertex model discovery found.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Subcommand;

use crate::CliConfig;

pub use super::catalog_discovery::discovery_rows;

#[derive(Debug, Clone, Copy, Subcommand)]
pub enum CatalogCommands {
    #[command(about = "Show what Vertex model discovery found")]
    Discovery,
}

pub fn execute(
    command: &CatalogCommands,
    config: &CliConfig,
    services: Option<&systemprompt_loader::ServicesState>,
) -> Result<()> {
    match command {
        CatalogCommands::Discovery => super::catalog_discovery::show_discovery(services, config),
    }
}
