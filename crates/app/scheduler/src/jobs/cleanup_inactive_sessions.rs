//! Periodic job that closes user sessions inactive for >= 1 hour.

use async_trait::async_trait;
use systemprompt_analytics::SessionRepository;
use systemprompt_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::debug;

use crate::error::SchedulerError;

/// Scheduled job that closes inactive sessions older than 1 hour.
#[derive(Debug, Clone, Copy)]
pub struct CleanupInactiveSessionsJob;

#[async_trait]
impl Job for CleanupInactiveSessionsJob {
    fn name(&self) -> &'static str {
        "cleanup_inactive_sessions"
    }

    fn description(&self) -> &'static str {
        "Cleans up inactive sessions (1 hour threshold)"
    }

    fn schedule(&self) -> &'static str {
        "0 */10 * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start_time = std::time::Instant::now();

        let db_pool = std::sync::Arc::clone(
            ctx.db_pool::<DbPool>()
                .ok_or_else(|| SchedulerError::missing_context("DbPool"))?,
        );

        debug!("Job started");

        let session_repo = SessionRepository::new(&db_pool).map_err(SchedulerError::from)?;
        let closed_sessions = session_repo
            .cleanup_inactive(1)
            .await
            .map_err(SchedulerError::from)?;

        let duration_ms = start_time.elapsed().as_millis() as u64;

        debug!(
            closed_sessions = closed_sessions,
            duration_ms = duration_ms,
            inactive_minutes = 60,
            "Job completed"
        );

        Ok(JobResult::success()
            .with_stats(closed_sessions, 0)
            .with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&CleanupInactiveSessionsJob);
