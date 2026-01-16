use anyhow::Result;
use async_trait::async_trait;
use systemprompt_core_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult};
use tracing::info;

use crate::repository::SchedulerRepository;

#[derive(Debug, Clone, Copy)]
pub struct CleanupEmptyContextsJob;

#[async_trait]
impl Job for CleanupEmptyContextsJob {
    fn name(&self) -> &'static str {
        "cleanup_empty_contexts"
    }

    fn description(&self) -> &'static str {
        "Deletes empty conversation contexts older than 1 hour"
    }

    fn schedule(&self) -> &'static str {
        "0 0 */2 * * *" // Every 2 hours
    }

    async fn execute(&self, ctx: &JobContext) -> Result<JobResult> {
        let start_time = std::time::Instant::now();

        let db_pool = std::sync::Arc::clone(
            ctx.db_pool::<DbPool>()
                .ok_or_else(|| anyhow::anyhow!("DbPool not available in job context"))?,
        );

        info!("Job started");

        let repository = SchedulerRepository::new(&db_pool)?;
        let deleted_count = repository.cleanup_empty_contexts(1).await?;

        let duration_ms = start_time.elapsed().as_millis() as u64;

        info!(
            deleted_contexts = deleted_count,
            duration_ms = duration_ms,
            "Job completed"
        );

        Ok(JobResult::success()
            .with_stats(deleted_count, 0)
            .with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&CleanupEmptyContextsJob);
