//! `admin evals` command group: judge runs over production AI traffic.
//!
//! Exposes [`EvalsCommands`] — launch an auto-improve judge run, list and
//! inspect runs, replay a run's failures, and promote a sampled request into
//! the golden case set. All inference goes through the process-local
//! [`AiService`](systemprompt_ai::AiService) built from the active profile.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod list;
pub mod promote;
pub mod replay;
pub mod run;
mod shared;
pub mod show;

use anyhow::Result;
use clap::Subcommand;

use crate::context::CommandContext;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum EvalsCommands {
    #[command(about = "Judge a sample of recent AI requests (auto-improve pass)")]
    Run(run::RunArgs),

    #[command(about = "List evaluation runs")]
    List(list::ListArgs),

    #[command(about = "Show a run and its results")]
    Show(show::ShowArgs),

    #[command(about = "Replay a run's failures as a new replay run")]
    Replay(replay::ReplayArgs),

    #[command(about = "Promote an AI request into the golden case set")]
    Promote(promote::PromoteArgs),
}

pub async fn execute(cmd: EvalsCommands, ctx: &CommandContext) -> Result<()> {
    let result = match cmd {
        EvalsCommands::Run(args) => run::execute(args, ctx).await?,
        EvalsCommands::List(args) => list::execute(args, ctx).await?,
        EvalsCommands::Show(args) => show::execute(args, ctx).await?,
        EvalsCommands::Replay(args) => replay::execute(args, ctx).await?,
        EvalsCommands::Promote(args) => promote::execute(args, ctx).await?,
    };
    render_result(&result, &ctx.cli);
    Ok(())
}
