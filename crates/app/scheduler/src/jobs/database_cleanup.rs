//! Periodic database-cleanup job: orphan logs, MCP executions, expired
//! OAuth artifacts.

use async_trait::async_trait;
use systemprompt_database::{CleanupRepository, DbPool};
use systemprompt_traits::{Job, JobContext, JobResult, ProviderError, ProviderResult};
use tracing::debug;

use crate::error::SchedulerError;

#[derive(Debug, Clone, Copy)]
pub struct DatabaseCleanupJob;

#[async_trait]
impl Job for DatabaseCleanupJob {
    fn name(&self) -> &'static str {
        "database_cleanup"
    }

    fn description(&self) -> &'static str {
        "Cleans up orphaned logs, MCP executions, and expired OAuth tokens"
    }

    fn schedule(&self) -> &'static str {
        "0 0 3 * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start_time = std::time::Instant::now();

        let db_pool = std::sync::Arc::clone(
            ctx.db_pool::<DbPool>()
                .ok_or_else(|| SchedulerError::missing_context("DbPool"))?,
        );

        debug!("Job started");

        let write_pool = db_pool.write_pool_arc().map_err(SchedulerError::from)?;
        let cleanup_repo = CleanupRepository::new_with_write_pool((*write_pool).clone());
        let mut total_deleted = 0u64;

        let orphaned_logs = cleanup_repo
            .delete_orphaned_logs()
            .await
            .map_err(|e| ProviderError::from(SchedulerError::from(e)))?;
        total_deleted += orphaned_logs;

        let orphaned_mcp = cleanup_repo
            .delete_orphaned_mcp_executions()
            .await
            .map_err(|e| ProviderError::from(SchedulerError::from(e)))?;
        total_deleted += orphaned_mcp;

        let old_logs = cleanup_repo
            .delete_old_logs(30)
            .await
            .map_err(|e| ProviderError::from(SchedulerError::from(e)))?;
        total_deleted += old_logs;

        let oauth = Self::delete_expired_oauth(&cleanup_repo).await?;
        total_deleted += oauth.total();

        let duration_ms = start_time.elapsed().as_millis() as u64;

        debug!(
            total_deleted = total_deleted,
            orphaned_logs = orphaned_logs,
            orphaned_mcp = orphaned_mcp,
            old_logs = old_logs,
            oauth_codes = oauth.codes,
            oauth_tokens = oauth.tokens,
            oauth_state_bindings = oauth.state_bindings,
            oauth_jti_revocations = oauth.jti_revocations,
            id_jag_replays = oauth.id_jag_replays,
            duration_ms = duration_ms,
            "Job completed"
        );

        Ok(JobResult::success()
            .with_stats(total_deleted, 0)
            .with_duration(duration_ms))
    }
}

struct OauthCleanupCounts {
    codes: u64,
    tokens: u64,
    state_bindings: u64,
    jti_revocations: u64,
    id_jag_replays: u64,
}

impl OauthCleanupCounts {
    fn total(&self) -> u64 {
        self.codes + self.tokens + self.state_bindings + self.jti_revocations + self.id_jag_replays
    }
}

impl DatabaseCleanupJob {
    async fn delete_expired_oauth(
        cleanup_repo: &CleanupRepository,
    ) -> ProviderResult<OauthCleanupCounts> {
        let codes = cleanup_repo
            .delete_expired_oauth_codes()
            .await
            .map_err(|e| ProviderError::from(SchedulerError::from(e)))?;
        let tokens = cleanup_repo
            .delete_expired_oauth_tokens()
            .await
            .map_err(|e| ProviderError::from(SchedulerError::from(e)))?;
        let state_bindings = cleanup_repo
            .delete_expired_oauth_state_bindings()
            .await
            .map_err(|e| ProviderError::from(SchedulerError::from(e)))?;
        let jti_revocations = cleanup_repo
            .delete_expired_oauth_jti_revocations()
            .await
            .map_err(|e| ProviderError::from(SchedulerError::from(e)))?;
        let id_jag_replays = cleanup_repo
            .delete_expired_id_jag_replays()
            .await
            .map_err(|e| ProviderError::from(SchedulerError::from(e)))?;

        Ok(OauthCleanupCounts {
            codes,
            tokens,
            state_bindings,
            jti_revocations,
            id_jag_replays,
        })
    }
}

systemprompt_provider_contracts::submit_job!(&DatabaseCleanupJob);
