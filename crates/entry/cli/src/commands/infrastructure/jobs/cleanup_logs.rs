use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use systemprompt_database::CleanupRepository;
use systemprompt_runtime::AppContext;

use super::types::LogCleanupOutput;
use crate::shared::CommandResult;

#[derive(Debug, Clone, Copy, Args)]
pub struct LogCleanupArgs {
    #[arg(long, default_value = "30", help = "Delete logs older than N days")]
    pub days: i32,

    #[arg(long, help = "Preview what would be cleaned without executing")]
    pub dry_run: bool,
}

pub(crate) async fn execute(args: LogCleanupArgs) -> Result<CommandResult<LogCleanupOutput>> {
    let ctx = Arc::new(AppContext::new().await?);
    let write_pool = ctx.db_pool().write_pool_arc()?;
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
        return Ok(CommandResult::text(output).with_title("Log Cleanup (Dry Run)"));
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
    Ok(CommandResult::text(output).with_title("Log Cleanup"))
}
