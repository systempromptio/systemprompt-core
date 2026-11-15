//! `core content link` command group: trackable campaign links.
//!
//! Dispatches the [`LinkCommands`] subcommands (generate, show, list,
//! performance, delete) for creating short codes and reading their click
//! metrics.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod delete;
pub mod generate;
pub mod list;
pub mod performance;
pub mod show;

use anyhow::{Context, Result};
use clap::Subcommand;
use systemprompt_extension::cli::{CommandHost, CommandOutput};

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

    #[command(about = "Delete a link")]
    Delete(delete::DeleteArgs),
}

pub async fn execute(command: LinkCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    match command {
        LinkCommands::Generate(args) => {
            generate::execute(args, host)
                .await
                .context("Failed to generate link")
        },
        LinkCommands::Show(args) => {
            show::execute(args, host)
                .await
                .context("Failed to show link")
        },
        LinkCommands::List(args) => {
            list::execute(args, host)
                .await
                .context("Failed to list links")
        },
        LinkCommands::Performance(args) => {
            performance::execute(args, host)
                .await
                .context("Failed to get link performance")
        },
        LinkCommands::Delete(args) => {
            delete::execute(args, host)
                .await
                .context("Failed to delete link")
        },
    }
}
