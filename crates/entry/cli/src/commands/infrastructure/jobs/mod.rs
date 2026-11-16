//! `jobs` CLI command group: list, inspect, run, and manage scheduled jobs.
//!
//! [`JobsCommands`] enumerates the subcommands; [`execute`] dispatches each to
//! its submodule and returns the result. Includes manual job runs, history and
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

use systemprompt_extension::AdmittedSchedule;
use systemprompt_extension::cli::{CommandHost, CommandOutput};

#[derive(Debug, Subcommand)]
pub enum JobsCommands {
    #[command(about = "List available jobs")]
    List,

    #[command(about = "Show detailed information about a job")]
    Show(show::ShowArgs),

    #[command(
        about = "Run a scheduled job manually",
        long_about = "Runs the named jobs now. A manual run is an explicit operator act and runs \
                      a job whatever its enabled flag; `infra jobs disable` governs scheduled \
                      runs only."
    )]
    Run(run::RunArgs),

    #[command(about = "View job execution history")]
    History(history::HistoryArgs),

    #[command(about = "Enable a job")]
    Enable(enable::EnableArgs),

    #[command(about = "Disable a job's scheduled runs (a manual run still runs it)")]
    Disable(disable::DisableArgs),

    #[command(
        about = "Delete scheduled_jobs rows no admitted job names",
        long_about = "Lists the scheduled_jobs rows that no job of the admitted composition \
                      names, and deletes them after confirmation (--dry-run only lists). The \
                      rows of a member this profile disables are included: it is not admitted. \
                      A node's boot never deletes a row, so this is how a retired job's row is \
                      removed. Run it with a binary and profile that admit every capability the \
                      cluster runs: a row only another composition admits would otherwise be \
                      deleted."
    )]
    Prune(prune::PruneArgs),
}

pub async fn execute(cmd: JobsCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    match cmd {
        JobsCommands::List => list::execute(host).await,
        JobsCommands::Show(args) => show::execute(args, host).await,
        JobsCommands::Run(args) => run::execute(args, host).await,
        JobsCommands::History(args) => history::execute(args, host).await,
        JobsCommands::Enable(args) => enable::execute(args, host).await,
        JobsCommands::Disable(args) => disable::execute(args, host).await,
        JobsCommands::Prune(args) => prune::execute(args, host).await,
    }
}

async fn admitted_schedule(host: &dyn CommandHost) -> Result<AdmittedSchedule> {
    Ok(host.resolved().await?.get::<AdmittedSchedule>()?)
}
