use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use systemprompt_analytics::SessionCleanupService;
use systemprompt_runtime::AppContext;

use super::types::SessionCleanupOutput;
use crate::shared::CommandResult;

#[derive(Debug, Clone, Copy, Args)]
pub struct CleanupSessionsArgs {
    #[arg(
        long,
        default_value = "1",
        help = "Sessions inactive for more than N hours"
    )]
    pub hours: i32,

    #[arg(long, help = "Preview what would be cleaned without executing")]
    pub dry_run: bool,
}

pub async fn execute(args: CleanupSessionsArgs) -> Result<CommandResult<SessionCleanupOutput>> {
    let ctx = Arc::new(AppContext::new().await?);

    if args.dry_run {
        let pool = ctx.db_pool().pool_arc()?;
        let cutoff_hours = args.hours;

        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM user_sessions
            WHERE ended_at IS NULL
              AND last_activity_at < NOW() - ($1 || ' hours')::INTERVAL
            "#,
            cutoff_hours.to_string()
        )
        .fetch_one(&*pool)
        .await?;

        let output = SessionCleanupOutput {
            job_name: "session_cleanup".to_string(),
            sessions_cleaned: 0,
            hours_threshold: args.hours,
            message: format!(
                "DRY RUN: Would clean up {} inactive session(s) older than {} hour(s)",
                count, args.hours
            ),
        };

        return Ok(CommandResult::text(output).with_title("Session Cleanup (Dry Run)"));
    }

    let cleanup_service = SessionCleanupService::new(ctx.db_pool())?;
    let closed_count = cleanup_service
        .cleanup_inactive_sessions(args.hours)
        .await?;

    let output = SessionCleanupOutput {
        job_name: "session_cleanup".to_string(),
        sessions_cleaned: closed_count as i64,
        hours_threshold: args.hours,
        message: format!("Cleaned up {} inactive session(s)", closed_count),
    };

    Ok(CommandResult::text(output).with_title("Session Cleanup"))
}
