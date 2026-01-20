pub mod db;
pub mod jobs;
pub mod logs;
pub mod services;
pub mod system;

use anyhow::Result;
use clap::Subcommand;
use systemprompt_runtime::DatabaseContext;

use crate::CliConfig;

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

    #[command(subcommand, about = "System authentication and session management")]
    System(system::SystemCommands),
}

pub async fn execute(cmd: InfraCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        InfraCommands::Services(cmd) => services::execute(cmd, config).await,
        InfraCommands::Db(cmd) => db::execute(cmd, config).await,
        InfraCommands::Jobs(cmd) => jobs::execute(cmd, config).await,
        InfraCommands::Logs(cmd) => logs::execute(cmd, config).await,
        InfraCommands::System(cmd) => system::execute(cmd).await,
    }
}

pub async fn execute_with_db(
    cmd: InfraCommands,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    match cmd {
        InfraCommands::Db(cmd) => db::execute_with_db(cmd, db_ctx, config).await,
        InfraCommands::Logs(cmd) => logs::execute_with_db(cmd, db_ctx, config).await,
        _ => anyhow::bail!("This command requires full profile context"),
    }
}
