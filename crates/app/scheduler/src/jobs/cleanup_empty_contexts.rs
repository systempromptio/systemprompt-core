//! Periodic job that prunes empty, audit-free conversation contexts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::info;

use crate::error::SchedulerError;
use crate::repository::SchedulerRepository;

const DEFAULT_RETENTION_HOURS: i64 = 24;

#[derive(Debug, Clone, Copy)]
pub struct CleanupEmptyContextsJob;

#[async_trait]
impl Job for CleanupEmptyContextsJob {
    fn name(&self) -> &'static str {
        "cleanup_empty_contexts"
    }

    fn description(&self) -> &'static str {
        "Deletes empty, audit-free conversation contexts (parameter retention_hours, default 24); requires enforce"
    }

    fn schedule(&self) -> &'static str {
        "0 0 */2 * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start_time = std::time::Instant::now();

        let db_pool = std::sync::Arc::clone(
            ctx.db_pool::<DbPool>()
                .ok_or_else(|| SchedulerError::missing_context("DbPool"))?,
        );

        info!("Job started");

        let retention_hours = ctx
            .get_parameter_parsed::<i64>("retention_hours")?
            .unwrap_or(DEFAULT_RETENTION_HOURS);

        let repository = SchedulerRepository::new(&db_pool)?;
        let deleted_count = if ctx.enforce() {
            repository.cleanup_empty_contexts(retention_hours).await?
        } else {
            let would_delete = repository.count_empty_contexts(retention_hours).await?;
            info!(
                would_delete_contexts = would_delete,
                retention_hours = retention_hours,
                "enforce disabled: empty contexts qualify for deletion but were not deleted"
            );
            0
        };

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
