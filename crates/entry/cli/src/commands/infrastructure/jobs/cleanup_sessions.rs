//! `infra jobs cleanup-sessions` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use systemprompt_analytics::{SessionCleanupService, SessionRepository};
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;

use super::types::SessionCleanupOutput;
use crate::shared::CommandOutput;

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

pub(super) async fn execute(args: CleanupSessionsArgs) -> Result<CommandOutput> {
    let ctx = Arc::new(AppContext::new().await?);
    execute_with_pool(args, ctx.db_pool()).await
}

pub async fn execute_with_pool(args: CleanupSessionsArgs, pool: &DbPool) -> Result<CommandOutput> {
    if args.dry_run {
        let repo = SessionRepository::new(pool)?;
        let count = repo.count_inactive(args.hours).await?;

        let output = SessionCleanupOutput {
            job_name: "session_cleanup".to_owned(),
            sessions_cleaned: 0,
            hours_threshold: args.hours,
            message: format!(
                "DRY RUN: Would clean up {} inactive session(s) older than {} hour(s)",
                count, args.hours
            ),
        };

        return Ok(CommandOutput::card_value(
            "Session Cleanup (Dry Run)",
            &output,
        ));
    }

    let cleanup_service = SessionCleanupService::new(pool)?;
    let closed_count = cleanup_service
        .cleanup_inactive_sessions(args.hours)
        .await?;

    let output = SessionCleanupOutput {
        job_name: "session_cleanup".to_owned(),
        sessions_cleaned: closed_count as i64,
        hours_threshold: args.hours,
        message: format!("Cleaned up {} inactive session(s)", closed_count),
    };

    Ok(CommandOutput::card_value("Session Cleanup", &output))
}
