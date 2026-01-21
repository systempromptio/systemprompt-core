use std::sync::Arc;

use super::policies::RetentionConfig;
use crate::repository::LoggingRepository;
use chrono::Utc;
use systemprompt_database::DbPool;
use tokio_cron_scheduler::{Job, JobScheduler};

#[derive(Debug)]
pub struct RetentionScheduler {
    config: RetentionConfig,
    db_pool: DbPool,
}

impl RetentionScheduler {
    #[must_use]
    pub const fn new(config: RetentionConfig, db_pool: DbPool) -> Self {
        Self { config, db_pool }
    }

    pub async fn start(self) -> anyhow::Result<()> {
        if !self.config.enabled {
            log_scheduler_disabled();
            return Ok(());
        }

        log_scheduler_starting(&self.config.schedule);
        let scheduler = JobScheduler::new().await?;
        let job = create_retention_job(self.config, self.db_pool)?;
        scheduler.add(job).await?;
        scheduler.start().await?;
        log_scheduler_started();
        Ok(())
    }
}

fn log_scheduler_disabled() {
    tracing::info!("Log retention scheduler is disabled");
}

fn log_scheduler_starting(schedule: &str) {
    tracing::info!(schedule = %schedule, "Starting log retention scheduler");
}

fn log_scheduler_started() {
    tracing::info!("Log retention scheduler started successfully");
}

fn create_retention_job(config: RetentionConfig, db_pool: DbPool) -> anyhow::Result<Job> {
    let schedule = config.schedule.clone();

    Job::new_async(schedule.as_str(), move |_uuid, _lock| {
        let config = config.clone();
        let db_pool = Arc::clone(&db_pool);

        Box::pin(async move {
            if let Err(e) = execute_retention_cleanup(config, db_pool).await {
                tracing::error!(error = %e, "Retention cleanup failed");
            }
        })
    })
    .map_err(Into::into)
}

async fn execute_retention_cleanup(config: RetentionConfig, db_pool: DbPool) -> anyhow::Result<()> {
    log_cleanup_starting();
    let repo = create_logging_repository(&db_pool);
    let total_deleted = apply_all_policies(&repo, &config.policies).await;
    log_cleanup_completed(total_deleted);
    Ok(())
}

fn create_logging_repository(db_pool: &DbPool) -> LoggingRepository {
    LoggingRepository::new(Arc::clone(db_pool))
        .with_database(true)
        .with_terminal(false)
}

async fn apply_all_policies(
    repo: &LoggingRepository,
    policies: &[super::policies::RetentionPolicy],
) -> u64 {
    let mut total = 0u64;
    for policy in policies {
        total += apply_retention_policy(repo, policy).await;
    }
    total
}

fn log_cleanup_starting() {
    tracing::info!("Starting scheduled log retention cleanup");
}

fn log_cleanup_completed(total_deleted: u64) {
    tracing::info!(total_deleted = total_deleted, "Retention cleanup completed");
}

async fn apply_retention_policy(
    repo: &LoggingRepository,
    policy: &super::policies::RetentionPolicy,
) -> u64 {
    let cutoff = Utc::now() - chrono::Duration::days(i64::from(policy.retention_days));
    match cleanup_logs_before_cutoff(repo, cutoff).await {
        Ok(deleted) => {
            log_policy_applied(&policy.name, deleted, policy.retention_days);
            deleted
        },
        Err(e) => {
            log_policy_failed(&policy.name, &e);
            0
        },
    }
}

fn log_policy_applied(name: &str, deleted: u64, retention_days: u32) {
    tracing::info!(policy = %name, deleted = deleted, retention_days = retention_days, "Policy applied");
}

fn log_policy_failed(name: &str, error: &anyhow::Error) {
    tracing::error!(policy = %name, error = %error, "Failed to apply policy");
}

async fn cleanup_logs_before_cutoff(
    repo: &LoggingRepository,
    cutoff: chrono::DateTime<Utc>,
) -> anyhow::Result<u64> {
    repo.cleanup_old_logs(cutoff).await.map_err(Into::into)
}
