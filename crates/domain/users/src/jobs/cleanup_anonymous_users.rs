use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult};
use tracing::info;

use crate::UserService;

#[derive(Debug, Clone, Copy)]
pub struct CleanupAnonymousUsersJob;

#[async_trait]
impl Job for CleanupAnonymousUsersJob {
    fn name(&self) -> &'static str {
        "cleanup_anonymous_users"
    }

    fn description(&self) -> &'static str {
        "Cleans up old anonymous users (30d)"
    }

    fn schedule(&self) -> &'static str {
        "0 0 * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> Result<JobResult> {
        let start_time = std::time::Instant::now();

        let db_pool = Arc::clone(
            ctx.db_pool::<DbPool>()
                .ok_or_else(|| anyhow::anyhow!("DbPool not available in job context"))?,
        );

        info!("Job started");

        let user_service = UserService::new(&db_pool)?;
        let deleted_users = user_service.cleanup_old_anonymous(30).await?;

        let duration_ms = start_time.elapsed().as_millis() as u64;

        info!(
            deleted_users = deleted_users,
            duration_ms = duration_ms,
            "Job completed"
        );

        Ok(JobResult::success()
            .with_stats(deleted_users, 0)
            .with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&CleanupAnonymousUsersJob);
