//! Periodic job that flags ghost sessions (0 requests, no landing page) as
//! behavioural bots.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::info;

use crate::repository::AnalyticsRepository;

#[derive(Debug, Clone, Copy)]
pub struct GhostSessionCleanupJob;

#[async_trait]
impl Job for GhostSessionCleanupJob {
    fn name(&self) -> &'static str {
        "ghost_session_cleanup"
    }

    fn description(&self) -> &'static str {
        "Marks ghost sessions (0 requests, no landing page) as behavioral bots"
    }

    fn schedule(&self) -> &'static str {
        "0 */10 * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start_time = std::time::Instant::now();

        let analytics = AnalyticsRepository::new(ctx.get::<DbPool>()?);
        let result = analytics.mark_ghost_sessions_as_bots().await?;

        let marked = result as u64;
        let duration_ms = start_time.elapsed().as_millis() as u64;

        if marked > 0 {
            info!(
                marked = marked,
                duration_ms = duration_ms,
                "Ghost session cleanup completed"
            );
        }

        Ok(JobResult::success()
            .with_stats(marked, 0)
            .with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&GhostSessionCleanupJob);
