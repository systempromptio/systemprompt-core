//! `jobs` CLI command group: list, inspect, run, and manage scheduled jobs.
//!
//! [`JobsCommands`] enumerates the subcommands; [`execute`] dispatches each to
//! its submodule and renders the result. Includes manual job runs, history and
//! enable/disable toggles; one-off cleanups run as jobs (`infra jobs run`) or
//! through `infra logs cleanup` / `admin users session cleanup`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod types;

mod disable;
mod enable;
mod helpers;
pub mod history;
mod list;
mod run;
mod show;

use anyhow::Result;
use clap::Subcommand;

use crate::context::CommandContext;
use crate::shared::render_result;

use systemprompt_generator as _;

#[derive(Debug, Subcommand)]
pub enum JobsCommands {
    #[command(about = "List available jobs")]
    List,

    #[command(about = "Show detailed information about a job")]
    Show(show::ShowArgs),

    #[command(about = "Run a scheduled job manually")]
    Run(run::RunArgs),

    #[command(about = "View job execution history")]
    History(history::HistoryArgs),

    #[command(about = "Enable a job")]
    Enable(enable::EnableArgs),

    #[command(about = "Disable a job")]
    Disable(disable::DisableArgs),
}

pub async fn execute(cmd: JobsCommands, ctx: &CommandContext) -> Result<()> {
    match cmd {
        JobsCommands::List => {
            render_result(&list::execute(), &ctx.cli);
            Ok(())
        },
        JobsCommands::Show(args) => {
            render_result(&show::execute(args, ctx).await?, &ctx.cli);
            Ok(())
        },
        JobsCommands::Run(args) => {
            render_result(&run::execute(args, ctx).await?, &ctx.cli);
            Ok(())
        },
        JobsCommands::History(args) => {
            render_result(&history::execute(args, ctx).await?, &ctx.cli);
            Ok(())
        },
        JobsCommands::Enable(args) => {
            render_result(&enable::execute(args, ctx).await?, &ctx.cli);
            Ok(())
        },
        JobsCommands::Disable(args) => {
            render_result(&disable::execute(args, ctx).await?, &ctx.cli);
            Ok(())
        },
    }
}
