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
use crate::descriptor::{CommandDescriptor, DataImpact, DescribeCommand};

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
        return Err(crate::shared::database_scoped_command_error(ctx));
    }

    match cmd {
        InfraCommands::Services(cmd) => services::execute(cmd, ctx).await,
        InfraCommands::Db(cmd) => db::execute(cmd, ctx).await,
        InfraCommands::Jobs(cmd) => jobs::execute(cmd, ctx).await,
        InfraCommands::Logs(cmd) => logs::execute(cmd, ctx).await,
    }
}

// Why: the registry lives in a OnceLock, so whoever installs it first decides
// whether discovered models are served. The server must be that installer,
// with discovery; a plain `try_init` here made `try_init_with_discovery` a
// silent no-op on every boot.
impl DescribeCommand for InfraCommands {
    fn descriptor(&self) -> CommandDescriptor {
        match self {
            Self::Services(cmd) => {
                let desc = match cmd {
                    services::ServicesCommands::Serve { .. }
                    | services::ServicesCommands::Start { .. } => {
                        CommandDescriptor::PROFILE_SECRETS_AND_PATHS.with_model_discovery()
                    },
                    _ => CommandDescriptor::PROFILE_SECRETS_AND_PATHS,
                };
                if cmd.serves_api() {
                    desc.with_serves_api()
                } else {
                    desc
                }
            },
            Self::Jobs(jobs::JobsCommands::Run(_) | jobs::JobsCommands::List) => {
                CommandDescriptor::FULL.with_skip_validation()
            },
            Self::Logs(logs::LogsCommands::Delete(_) | logs::LogsCommands::Cleanup(_)) => {
                CommandDescriptor::FULL
            },
            Self::Logs(_) => CommandDescriptor::FULL.with_read_only(),
            _ => CommandDescriptor::FULL,
        }
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
                DbCommands::MigrateDown { .. }
                | DbCommands::MigrateMarkApplied { .. }
                | DbCommands::AssignAdmin { .. },
            ) => DataImpact::Irreversible,
            Self::Db(
                DbCommands::Execute { .. }
                | DbCommands::Migrate { .. }
                | DbCommands::MigrateRepair { .. },
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
