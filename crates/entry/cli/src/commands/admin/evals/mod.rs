//! `admin evals` read and case-capture commands.
//!
//! Paid execution is available only through supervised experiments so every
//! dispatch is reservation-backed and generation-pinned.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod list;
pub mod promote;
mod shared;
pub mod show;

use anyhow::Result;
use clap::Subcommand;

use crate::context::CommandContext;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum EvalsCommands {
    #[command(about = "List evaluation runs")]
    List(list::ListArgs),

    #[command(about = "Show a run and its results")]
    Show(show::ShowArgs),

    #[command(about = "Promote an AI request into the golden case set")]
    Promote(promote::PromoteArgs),
}

pub async fn execute(cmd: EvalsCommands, ctx: &CommandContext) -> Result<()> {
    let result = match cmd {
        EvalsCommands::List(args) => list::execute(args, ctx).await?,
        EvalsCommands::Show(args) => show::execute(args, ctx).await?,
        EvalsCommands::Promote(args) => promote::execute(args, ctx).await?,
    };
    render_result(&result, &ctx.cli);
    Ok(())
}
