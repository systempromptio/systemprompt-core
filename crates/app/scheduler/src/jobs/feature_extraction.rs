use anyhow::Result;
use async_trait::async_trait;
use systemprompt_core_analytics::{FeatureExtractionConfig, FeatureExtractionService};
use systemprompt_core_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult};
use tracing::info;

#[derive(Debug, Clone, Copy)]
pub struct FeatureExtractionJob;

#[async_trait]
impl Job for FeatureExtractionJob {
    fn name(&self) -> &'static str {
        "feature_extraction"
    }

    fn description(&self) -> &'static str {
        "Extracts ML behavioral features from completed sessions"
    }

    fn schedule(&self) -> &'static str {
        "0 */15 * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> Result<JobResult> {
        let start_time = std::time::Instant::now();

        let db_pool = std::sync::Arc::clone(
            ctx.db_pool::<DbPool>()
                .ok_or_else(|| anyhow::anyhow!("DbPool not available in job context"))?,
        );

        info!("Job started");

        let service = FeatureExtractionService::new(&db_pool, FeatureExtractionConfig::default())?;
        let sessions = self.get_unprocessed_sessions(&db_pool).await?;

        let total_sessions = sessions.len();
        let mut processed = 0u64;
        let mut failed = 0u64;

        for session_id in sessions {
            match service.extract_session_features(&session_id).await {
                Ok(_) => processed += 1,
                Err(e) => {
                    tracing::warn!(session_id = %session_id, error = %e, "Feature extraction failed");
                    failed += 1;
                },
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        info!(
            total_sessions = total_sessions,
            processed = processed,
            failed = failed,
            duration_ms = duration_ms,
            "Job completed"
        );

        Ok(JobResult::success()
            .with_stats(processed, failed)
            .with_duration(duration_ms))
    }
}

impl FeatureExtractionJob {
    async fn get_unprocessed_sessions(&self, db_pool: &DbPool) -> Result<Vec<String>> {
        let pool = db_pool.pool_arc()?;

        let sessions: Vec<String> = sqlx::query_scalar!(
            r#"
            SELECT s.session_id as "session_id!"
            FROM user_sessions s
            LEFT JOIN ml_behavioral_features m ON s.session_id = m.session_id
            WHERE s.ended_at IS NOT NULL
              AND s.ended_at > CURRENT_TIMESTAMP - INTERVAL '1 day'
              AND m.id IS NULL
              AND s.is_bot = false
            ORDER BY s.ended_at DESC
            LIMIT 500
            "#
        )
        .fetch_all(pool.as_ref())
        .await?;

        Ok(sessions)
    }
}

systemprompt_traits::submit_job!(&FeatureExtractionJob);
