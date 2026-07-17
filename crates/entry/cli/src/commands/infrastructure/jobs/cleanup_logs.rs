//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use systemprompt_database::{CleanupRepository, DbPool};
use systemprompt_runtime::AppContext;

use super::types::LogCleanupOutput;
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Copy, Args)]
pub struct LogCleanupArgs {
    #[arg(long, default_value = "30", help = "Delete logs older than N days")]
    pub days: i32,

    #[arg(long, help = "Preview what would be cleaned without executing")]
    pub dry_run: bool,
}

pub(super) async fn execute(args: LogCleanupArgs) -> Result<CommandOutput> {
    let ctx = Arc::new(AppContext::new().await?);
    execute_with_pool(args, ctx.db_pool()).await
}

pub async fn execute_with_pool(args: LogCleanupArgs, pool: &DbPool) -> Result<CommandOutput> {
    let write_pool = pool.write_pool_arc()?;
    let repo = CleanupRepository::new_with_write_pool((*write_pool).clone());

    if args.dry_run {
        let count = repo.count_old_logs(args.days).await?;
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

    let deleted_count = repo.delete_old_logs(args.days).await? as i64;
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
