//! Periodic job that flags ghost sessions (0 requests, no landing page) as
//! behavioural bots.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::info;

use crate::error::SchedulerError;

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

        let db_pool = std::sync::Arc::clone(
            ctx.db_pool::<DbPool>()
                .ok_or_else(|| SchedulerError::missing_context("DbPool"))?,
        );

        let pool = db_pool.write_pool_arc().map_err(SchedulerError::from)?;

        // Why: Writers reclassify the very flags v_clean_traffic filters on, so the
        // human predicate stays inline here and must mirror that view.
        let result = sqlx::query_scalar!(
            r#"
            WITH cleaned AS (
                UPDATE user_sessions
                SET is_behavioral_bot = true,
                    behavioral_bot_reason = 'ghost_session',
                    behavioral_bot_score = 35
                WHERE is_bot = false
                  AND is_ai_crawler = false
                  AND is_scanner = false
                  AND is_behavioral_bot = false
                  AND request_count = 0
                  AND landing_page IS NULL
                  AND entry_url IS NULL
                  AND started_at < NOW() - INTERVAL '5 minutes'
                RETURNING 1
            )
            SELECT COUNT(*)::BIGINT as "count!" FROM cleaned
            "#
        )
        .fetch_one(pool.as_ref())
        .await
        .map_err(SchedulerError::from)?;

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
