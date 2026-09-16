//! `infra jobs cleanup-logs` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use systemprompt_database::DbPool;
use systemprompt_logging::LoggingRepository;

use super::types::LogCleanupOutput;
use crate::context::CommandContext;
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Copy, Args)]
pub struct LogCleanupArgs {
    #[arg(long, default_value = "30", help = "Delete logs older than N days")]
    pub days: i32,

    #[arg(long, help = "Preview what would be cleaned without executing")]
    pub dry_run: bool,
}

pub(super) async fn execute(args: LogCleanupArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let app = ctx.app_context().await?;
    execute_with_pool(args, app.db_pool()).await
}

pub async fn execute_with_pool(args: LogCleanupArgs, pool: &DbPool) -> Result<CommandOutput> {
    let repo = LoggingRepository::new(pool)?;
    let cutoff = chrono::Utc::now() - chrono::Duration::days(i64::from(args.days));

    if args.dry_run {
        let count = repo.count_logs_before(cutoff).await?;
        let output = LogCleanupOutput {
            job_name: "log_cleanup".to_owned(),
            entries_deleted: 0,
            days_threshold: args.days,
            message: format!(
                "DRY RUN: Would delete {} log entries older than {} day(s)",
                count, args.days
            ),
        };
        return Ok(CommandOutput::card_value("Log Cleanup (Dry Run)", &output));
    }

    let deleted_count = i64::try_from(repo.cleanup_old_logs(cutoff).await?).unwrap_or(i64::MAX);
    let output = LogCleanupOutput {
        job_name: "log_cleanup".to_owned(),
        entries_deleted: deleted_count,
        days_threshold: args.days,
        message: format!(
            "Deleted {} log entries older than {} day(s)",
            deleted_count, args.days
        ),
    };
    Ok(CommandOutput::card_value("Log Cleanup", &output))
}
