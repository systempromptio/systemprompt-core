//! Persistence layer for the scheduler crate.
//!
//! [`SchedulerRepository`] is the composite façade combining the per-domain
//! repositories ([`JobRepository`], [`AnalyticsRepository`]); it is the type
//! consumed by [`crate::services::SchedulerService`] and by the API
//! lifecycle bootstrap path.
//!
//! [`SecurityRepository`] / [`IpSessionRecord`] are exposed for direct use by
//! the [`crate::jobs::malicious_ip_blacklist`] job.

mod analytics;
mod jobs;
mod security;

pub use analytics::AnalyticsRepository;
pub use jobs::JobRepository;
pub use security::{IpSessionRecord, SecurityRepository};

use chrono::{DateTime, Utc};
use systemprompt_database::DbPool;

use crate::error::SchedulerResult;
use crate::models::{JobStatus, ScheduledJob};

/// Composite repository façade aggregating job-row and analytics access.
#[derive(Debug, Clone)]
pub struct SchedulerRepository {
    jobs: JobRepository,
    analytics: AnalyticsRepository,
}

impl SchedulerRepository {
    /// Construct a new façade from a shared [`DbPool`].
    pub fn new(db: &DbPool) -> SchedulerResult<Self> {
        Ok(Self {
            jobs: JobRepository::new(db)?,
            analytics: AnalyticsRepository::new(db)?,
        })
    }

    /// Insert or update the canonical row for a scheduled job.
    pub async fn upsert_job(
        &self,
        job_name: &str,
        schedule: &str,
        enabled: bool,
    ) -> SchedulerResult<()> {
        self.jobs.upsert_job(job_name, schedule, enabled).await
    }

    /// Look up the persisted record for a job by name.
    pub async fn find_job(&self, job_name: &str) -> SchedulerResult<Option<ScheduledJob>> {
        self.jobs.find_job(job_name).await
    }

    /// List every persisted job whose `enabled` flag is currently `true`.
    pub async fn list_enabled_jobs(&self) -> SchedulerResult<Vec<ScheduledJob>> {
        self.jobs.list_enabled_jobs().await
    }

    /// Persist the post-execution status for a job (last status, error, next
    /// run timestamp).
    pub async fn update_job_execution(
        &self,
        job_name: &str,
        status: JobStatus,
        error: Option<&str>,
        next_run: Option<DateTime<Utc>>,
    ) -> SchedulerResult<()> {
        self.jobs
            .update_job_execution(job_name, status, error, next_run)
            .await
    }

    /// Atomically increment the `run_count` for a job by 1.
    pub async fn increment_run_count(&self, job_name: &str) -> SchedulerResult<()> {
        self.jobs.increment_run_count(job_name).await
    }

    /// Delete `user_contexts` rows that have no associated `task_messages` and
    /// are older than `hours_old`.
    pub async fn cleanup_empty_contexts(&self, hours_old: i64) -> SchedulerResult<u64> {
        self.analytics.cleanup_empty_contexts(hours_old).await
    }
}
