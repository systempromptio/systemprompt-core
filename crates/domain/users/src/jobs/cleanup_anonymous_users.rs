//! Scheduled job pruning anonymous users past the retention window.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_provider_contracts::ProviderError;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use tracing::info;

use crate::UserService;

const DEFAULT_RETENTION_DAYS: i32 = 30;

#[derive(Debug, Clone, Copy)]
pub struct CleanupAnonymousUsersJob;

#[async_trait]
impl Job for CleanupAnonymousUsersJob {
    fn name(&self) -> &'static str {
        "cleanup_anonymous_users"
    }

    fn description(&self) -> &'static str {
        "Deletes old anonymous users (parameter retention_days, default 30); requires enforce"
    }

    fn schedule(&self) -> &'static str {
        "0 0 * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start_time = std::time::Instant::now();

        let db_pool = Arc::clone(ctx.db_pool::<DbPool>().ok_or_else(|| {
            ProviderError::Configuration("DbPool not available in job context".into())
        })?);

        info!("Job started");

        let retention_days = ctx
            .get_parameter_parsed::<i32>("retention_days")?
            .unwrap_or(DEFAULT_RETENTION_DAYS);

        let user_service =
            UserService::new(&db_pool).map_err(|e| ProviderError::Configuration(e.to_string()))?;
        let deleted_users = if ctx.enforce() {
            user_service
                .cleanup_old_anonymous(retention_days)
                .await
                .map_err(|e| ProviderError::Configuration(e.to_string()))?
        } else {
            let would_delete = user_service
                .count_old_anonymous(retention_days)
                .await
                .map_err(|e| ProviderError::Configuration(e.to_string()))?;
            info!(
                would_delete_users = would_delete,
                retention_days = retention_days,
                "enforce disabled: anonymous users qualify for deletion but were not deleted"
            );
            0
        };

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
