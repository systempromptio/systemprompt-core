//! `core artifacts` command group: inspect A2A task artifacts.
//!
//! Dispatches the [`ArtifactsCommands`] subcommands (list, show) against the
//! invocation's [`CommandContext`], rendering each command's `CommandOutput`
//! to the configured output sink.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod list;
pub mod show;
pub mod types;

use crate::context::CommandContext;
use crate::shared::render_result;
use anyhow::Result;
use clap::Subcommand;

pub use types::*;

#[derive(Debug, Subcommand)]
pub enum ArtifactsCommands {
    #[command(about = "List artifacts")]
    List(list::ListArgs),

    #[command(about = "Show artifact details and content")]
    Show(show::ShowArgs),
}

pub async fn execute(cmd: ArtifactsCommands, ctx: &CommandContext) -> Result<()> {
    match cmd {
        ArtifactsCommands::List(args) => {
            let result = list::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
        },
        ArtifactsCommands::Show(args) => {
            let result = show::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
        },
    }
    Ok(())
}
