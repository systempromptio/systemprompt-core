//! Periodic log-cleanup job: orphaned and aged-out `logs` rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_logging::LoggingRepository;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderError, ProviderResult};
use systemprompt_users::UserRepository;
use tracing::{debug, info};

use crate::error::SchedulerError;

const DEFAULT_LOG_RETENTION_DAYS: i32 = 30;

#[derive(Debug, Clone, Copy)]
pub struct DatabaseCleanupJob;

#[async_trait]
impl Job for DatabaseCleanupJob {
    fn name(&self) -> &'static str {
        "database_cleanup"
    }

    fn description(&self) -> &'static str {
        "Cleans up orphaned logs and old logs (parameter log_retention_days, default 30); deletion requires enforce"
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

        let log_retention_days = ctx
            .get_parameter_parsed::<i32>("log_retention_days")?
            .unwrap_or(DEFAULT_LOG_RETENTION_DAYS);
        let cutoff = chrono::Utc::now() - chrono::Duration::days(i64::from(log_retention_days));

        let logs = LoggingRepository::new(&db_pool)
            .map_err(|e| ProviderError::Configuration(e.to_string()))?;
        let users = UserRepository::new(&db_pool)
            .map_err(|e| ProviderError::Configuration(e.to_string()))?;
        let internal =
            |e: systemprompt_logging::models::LoggingError| ProviderError::Internal(e.to_string());

        // Why: `logs` and `users` have different owners, so the orphan set is
        // computed by asking each: the log owners seen, minus the users that
        // still exist.
        let seen = logs.distinct_log_user_ids().await.map_err(internal)?;
        let orphans = users
            .missing_ids(&seen)
            .await
            .map_err(|e| ProviderError::Internal(e.to_string()))?;

        let (orphaned_logs, old_logs) = if ctx.enforce() {
            let orphaned = logs
                .delete_logs_for_users(&orphans)
                .await
                .map_err(internal)?;
            let old = logs.cleanup_old_logs(cutoff).await.map_err(internal)?;
            (orphaned, old)
        } else {
            let orphaned = logs
                .count_logs_for_users(&orphans)
                .await
                .map_err(internal)?;
            let old = logs.count_logs_before(cutoff).await.map_err(internal)?;
            info!(
                would_delete_orphaned_logs = orphaned,
                would_delete_old_logs = old,
                log_retention_days = log_retention_days,
                "enforce disabled: log rows qualify for deletion but were not deleted"
            );
            (0, 0)
        };
        let total_deleted = orphaned_logs + old_logs;

        let duration_ms = u64::try_from(start_time.elapsed().as_millis()).unwrap_or(u64::MAX);

        debug!(
            total_deleted = total_deleted,
            orphaned_logs = orphaned_logs,
            old_logs = old_logs,
            duration_ms = duration_ms,
            "Job completed"
        );

        Ok(JobResult::success()
            .with_stats(total_deleted, 0)
            .with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&DatabaseCleanupJob);
