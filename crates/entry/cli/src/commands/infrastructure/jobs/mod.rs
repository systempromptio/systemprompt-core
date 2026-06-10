//! `jobs` CLI command group: list, inspect, run, and manage scheduled jobs.
//!
//! [`JobsCommands`] enumerates the subcommands; [`execute`] dispatches each to
//! its submodule and renders the result. Includes manual job runs, history,
//! enable/disable toggles, and the session/log cleanup helpers.

pub mod types;

mod cleanup_logs;
mod cleanup_sessions;
mod disable;
mod enable;
mod helpers;
mod history;
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

    #[command(about = "Clean up inactive sessions")]
    CleanupSessions(cleanup_sessions::CleanupSessionsArgs),

    #[command(about = "Clean up old log entries")]
    LogCleanup(cleanup_logs::LogCleanupArgs),

    #[command(about = "Clean up inactive sessions (alias)", hide = true)]
    SessionCleanup(cleanup_sessions::CleanupSessionsArgs),
}

pub async fn execute(cmd: JobsCommands, ctx: &CommandContext) -> Result<()> {
    match cmd {
        JobsCommands::List => {
            render_result(&list::execute(), &ctx.cli);
            Ok(())
        },
        JobsCommands::Show(args) => {
            render_result(&show::execute(args).await?, &ctx.cli);
            Ok(())
        },
        JobsCommands::Run(args) => {
            render_result(&run::execute(args).await?, &ctx.cli);
            Ok(())
        },
        JobsCommands::History(args) => {
            render_result(&history::execute(args).await?, &ctx.cli);
            Ok(())
        },
        JobsCommands::Enable(args) => {
            render_result(&enable::execute(args).await?, &ctx.cli);
            Ok(())
        },
        JobsCommands::Disable(args) => {
            render_result(&disable::execute(args).await?, &ctx.cli);
            Ok(())
        },
        JobsCommands::CleanupSessions(args) | JobsCommands::SessionCleanup(args) => {
            render_result(&cleanup_sessions::execute(args).await?, &ctx.cli);
            Ok(())
        },
        JobsCommands::LogCleanup(args) => {
            render_result(&cleanup_logs::execute(args).await?, &ctx.cli);
            Ok(())
        },
    }
}
