//! `infra` command group: services, database, jobs, and logs administration.
//!
//! Routes [`InfraCommands`] to the per-domain subcommand modules. On a
//! `--database-url` invocation only the db and logs subtrees are served; the
//! rest require a full profile context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod db;
pub mod jobs;
pub mod logs;
pub mod services;

use anyhow::Result;
use clap::Subcommand;

use crate::context::CommandContext;
use crate::descriptor::DataImpact;

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
    if ctx.is_database_scoped()
        && !matches!(
            cmd,
            InfraCommands::Db(_)
                | InfraCommands::Logs(_)
                | InfraCommands::Jobs(
                    jobs::JobsCommands::List
                        | jobs::JobsCommands::Show(_)
                        | jobs::JobsCommands::History(_)
                )
        )
    {
        return Err(crate::shared::database_scoped_command_error());
    }

    match cmd {
        InfraCommands::Services(cmd) => services::execute(cmd, ctx).await,
        InfraCommands::Db(cmd) => db::execute(cmd, ctx).await,
        InfraCommands::Jobs(cmd) => jobs::execute(cmd, ctx).await,
        InfraCommands::Logs(cmd) => logs::execute(cmd, ctx).await,
    }
}

impl InfraCommands {
    pub const fn data_impact(&self) -> DataImpact {
        use db::DbCommands;
        use jobs::JobsCommands;
        use logs::LogsCommands;
        use services::ServicesCommands;

        match self {
            Self::Db(
                DbCommands::Execute { .. }
                | DbCommands::Migrate { .. }
                | DbCommands::MigrateDown { .. }
                | DbCommands::MigrateRepair { .. }
                | DbCommands::MigrateMarkApplied { .. }
                | DbCommands::AssignAdmin { .. },
            )
            | Self::Jobs(JobsCommands::Run(_))
            | Self::Logs(LogsCommands::Cleanup(_) | LogsCommands::Delete(_)) => {
                DataImpact::Destructive
            },
            Self::Db(
                DbCommands::Query { .. }
                | DbCommands::Tables { .. }
                | DbCommands::Describe { .. }
                | DbCommands::Info
                | DbCommands::Migrations { .. }
                | DbCommands::MigratePlan { .. }
                | DbCommands::MigrateStatus { .. }
                | DbCommands::Status
                | DbCommands::Count { .. }
                | DbCommands::Indexes { .. }
                | DbCommands::Size
                | DbCommands::Doctor,
            )
            | Self::Jobs(
                JobsCommands::List
                | JobsCommands::Show(_)
                | JobsCommands::History(_)
                | JobsCommands::Enable(_)
                | JobsCommands::Disable(_),
            )
            | Self::Logs(
                LogsCommands::View(_)
                | LogsCommands::Search(_)
                | LogsCommands::Stream(_)
                | LogsCommands::Export(_)
                | LogsCommands::Summary(_)
                | LogsCommands::Show(_)
                | LogsCommands::Trace(
                    logs::trace::TraceCommands::List(_) | logs::trace::TraceCommands::Show(_),
                )
                | LogsCommands::Governance(logs::governance::GovernanceCommands::Report(_))
                | LogsCommands::Request(
                    logs::request::RequestCommands::List(_)
                    | logs::request::RequestCommands::Show(_)
                    | logs::request::RequestCommands::Stats(_),
                )
                | LogsCommands::Tools(logs::tools::ToolsCommands::List(_))
                | LogsCommands::Audit(_),
            )
            | Self::Services(
                ServicesCommands::Start { .. }
                | ServicesCommands::Stop { .. }
                | ServicesCommands::Restart { .. }
                | ServicesCommands::Status { .. }
                | ServicesCommands::Cleanup { .. }
                | ServicesCommands::Serve { .. },
            ) => DataImpact::Preserving,
        }
    }
}
