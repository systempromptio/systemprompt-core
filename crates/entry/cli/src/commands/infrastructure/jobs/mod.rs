//! `jobs` CLI command group: list, inspect, run, and manage scheduled jobs.
//!
//! [`JobsCommands`] enumerates the subcommands; [`execute`] dispatches each to
//! its submodule and renders the result. Includes manual job runs, history and
//! enable/disable toggles, and `prune`, the only path that deletes
//! `scheduled_jobs` rows; one-off cleanups run as jobs (`infra jobs run`) or
//! through `infra logs cleanup` / `admin users session cleanup`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod types;

mod cron_text;
mod disable;
mod enable;
pub mod history;
mod list;
mod prune;
mod run;
mod show;

use anyhow::Result;
use clap::Subcommand;

use crate::context::CommandContext;
use crate::shared::render_result;

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

    #[command(
        about = "Delete scheduled_jobs rows no enabled extension in this binary registers",
        long_about = "Lists the scheduled_jobs rows that no extension enabled in this binary and \
                      profile registers, and deletes them after confirmation (--dry-run only \
                      lists). A node's boot never deletes a row, so this is how a retired job's \
                      row is removed. Run it from a binary and profile that compose every \
                      capability the cluster runs: a row registered only by another node's \
                      composition would otherwise be deleted."
    )]
    Prune(prune::PruneArgs),
}

pub async fn execute(cmd: JobsCommands, ctx: &CommandContext) -> Result<()> {
    match cmd {
        JobsCommands::List => render_result(&list::execute(ctx).await?, &ctx.cli),
        JobsCommands::Show(args) => render_result(&show::execute(args, ctx).await?, &ctx.cli),
        JobsCommands::Run(args) => render_result(&run::execute(args, ctx).await?, &ctx.cli),
        JobsCommands::History(args) => render_result(&history::execute(args, ctx).await?, &ctx.cli),
        JobsCommands::Enable(args) => render_result(&enable::execute(args, ctx).await?, &ctx.cli),
        JobsCommands::Disable(args) => render_result(&disable::execute(args, ctx).await?, &ctx.cli),
        JobsCommands::Prune(args) => render_result(&prune::execute(args, ctx).await?, &ctx.cli),
    }
    Ok(())
}
