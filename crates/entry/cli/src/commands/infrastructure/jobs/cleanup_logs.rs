use anyhow::Result;
use clap::Args;
use std::sync::Arc;
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

pub async fn execute(args: LogCleanupArgs) -> Result<CommandResult<LogCleanupOutput>> {
    let ctx = Arc::new(AppContext::new().await?);
    let pool = ctx.db_pool().pool_arc()?;

    if args.dry_run {
        let count: i64 = sqlx::query_scalar::<_, i64>(
            r"
            SELECT COUNT(*)
            FROM application_logs
            WHERE created_at < NOW() - ($1 || ' days')::INTERVAL
            ",
        )
        .bind(args.days.to_string())
        .fetch_one(&*pool)
        .await
        .unwrap_or(0);

        let output = LogCleanupOutput {
            job_name: "log_cleanup".to_string(),
            entries_deleted: 0,
            days_threshold: args.days,
            message: format!(
                "DRY RUN: Would delete {} log entries older than {} day(s)",
                count, args.days
            ),
        };

        return Ok(CommandResult::text(output).with_title("Log Cleanup (Dry Run)"));
    }

    let deleted_count = sqlx::query(
        r"
        DELETE FROM application_logs
        WHERE created_at < NOW() - ($1 || ' days')::INTERVAL
        ",
    )
    .bind(args.days.to_string())
    .execute(&*pool)
    .await?
    .rows_affected() as i64;

    let output = LogCleanupOutput {
        job_name: "log_cleanup".to_string(),
        entries_deleted: deleted_count,
        days_threshold: args.days,
        message: format!(
            "Deleted {} log entries older than {} day(s)",
            deleted_count, args.days
        ),
    };

    Ok(CommandResult::text(output).with_title("Log Cleanup"))
}
