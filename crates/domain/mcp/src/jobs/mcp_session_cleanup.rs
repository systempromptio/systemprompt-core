use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult};
use tracing::info;

use crate::repository::McpSessionRepository;

const STALE_SESSION_RETENTION_DAYS: i32 = 7;

#[derive(Debug, Clone, Copy)]
pub struct McpSessionCleanupJob;

#[async_trait]
impl Job for McpSessionCleanupJob {
    fn name(&self) -> &'static str {
        "mcp_session_cleanup"
    }

    fn description(&self) -> &'static str {
        "Expires stale MCP sessions and deletes old closed/expired records"
    }

    fn schedule(&self) -> &'static str {
        "0 */30 * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> Result<JobResult> {
        let start_time = std::time::Instant::now();

        let db_pool = Arc::clone(
            ctx.db_pool::<DbPool>()
                .ok_or_else(|| anyhow::anyhow!("DbPool not available in job context"))?,
        );

        let repo = McpSessionRepository::new(&db_pool)?;

        let expired = repo.cleanup_expired().await?;
        let deleted = repo.delete_stale(STALE_SESSION_RETENTION_DAYS).await?;

        let duration_ms = start_time.elapsed().as_millis() as u64;

        if expired > 0 || deleted > 0 {
            info!(
                expired = expired,
                deleted = deleted,
                duration_ms = duration_ms,
                "MCP session cleanup completed"
            );
        }

        Ok(JobResult::success()
            .with_stats(expired + deleted, 0)
            .with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&McpSessionCleanupJob);
