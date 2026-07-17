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

use crate::context::CommandContext;
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

    #[command(about = "Delete a link")]
    Delete(delete::DeleteArgs),
}

pub async fn execute(command: LinkCommands, ctx: &CommandContext) -> Result<()> {
    match command {
        LinkCommands::Generate(args) => {
            let result = generate::execute(args, ctx)
                .await
                .context("Failed to generate link")?;
            render_result(&result, &ctx.cli);
        },
        LinkCommands::Show(args) => {
            let result = show::execute(args, ctx)
                .await
                .context("Failed to show link")?;
            render_result(&result, &ctx.cli);
        },
        LinkCommands::List(args) => {
            let result = list::execute(args, ctx)
                .await
                .context("Failed to list links")?;
            render_result(&result, &ctx.cli);
        },
        LinkCommands::Performance(args) => {
            let result = performance::execute(args, ctx)
                .await
                .context("Failed to get link performance")?;
            render_result(&result, &ctx.cli);
        },
        LinkCommands::Delete(args) => {
            let result = delete::execute(args, ctx)
                .await
                .context("Failed to delete link")?;
            render_result(&result, &ctx.cli);
        },
    }
    Ok(())
}
