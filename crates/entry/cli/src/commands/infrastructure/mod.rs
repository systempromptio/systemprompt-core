//! `infra` command group: services, database, jobs, and logs administration.
//!
//! Routes [`InfraCommands`] to the per-domain subcommand modules. On a
//! `--database-url` invocation only the db and logs subtrees are served; the
//! rest require a full profile context.

pub mod db;
pub mod jobs;
pub mod logs;
pub mod services;

use anyhow::Result;
use clap::Subcommand;

use crate::context::CommandContext;

#[derive(Debug, Subcommand)]
pub enum InfraCommands {
    #[command(
        subcommand,
        about = "Service lifecycle management (start, stop, status)"
    )]
    Services(services::ServicesCommands),

    #[command(subcommand, about = "Database operations and administration")]
    Db(db::DbCommands),

    #[command(subcommand, about = "Background jobs and scheduling")]
    Jobs(jobs::JobsCommands),

    #[command(subcommand, about = "Log streaming and tracing")]
    Logs(logs::LogsCommands),
}

pub async fn execute(cmd: InfraCommands, ctx: &CommandContext) -> Result<()> {
    if ctx.is_database_scoped() && !matches!(cmd, InfraCommands::Db(_) | InfraCommands::Logs(_)) {
        return Err(crate::shared::database_scoped_command_error());
    }

    match cmd {
        InfraCommands::Services(cmd) => services::execute(cmd, ctx).await,
        InfraCommands::Db(cmd) => db::execute(cmd, ctx).await,
        InfraCommands::Jobs(cmd) => jobs::execute(cmd, ctx).await,
        InfraCommands::Logs(cmd) => logs::execute(cmd, ctx).await,
    }
}
