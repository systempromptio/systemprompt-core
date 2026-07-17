//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::info;

use crate::repository::McpSessionRepository;

const STALE_SESSION_RETENTION_DAYS: i32 = 7;

#[derive(Debug, Clone, Copy)]
struct McpSessionCleanupJob;

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

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start_time = std::time::Instant::now();

        let db_pool = Arc::clone(ctx.db_pool::<DbPool>().ok_or_else(|| {
            systemprompt_provider_contracts::ProviderError::Internal(
                "DbPool not available in job context".to_owned(),
            )
        })?);

        let repo = McpSessionRepository::new(&db_pool)
            .map_err(|e| systemprompt_provider_contracts::ProviderError::Internal(e.to_string()))?;

        let expired = repo
            .cleanup_expired()
            .await
            .map_err(|e| systemprompt_provider_contracts::ProviderError::Internal(e.to_string()))?;
        let deleted = repo
            .delete_stale(STALE_SESSION_RETENTION_DAYS)
            .await
            .map_err(|e| systemprompt_provider_contracts::ProviderError::Internal(e.to_string()))?;

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
