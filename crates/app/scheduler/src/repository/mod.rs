mod analytics;
mod jobs;

pub use analytics::AnalyticsRepository;
pub use jobs::JobRepository;

use chrono::{DateTime, Utc};
use systemprompt_core_database::DbPool;

use crate::models::{JobStatus, ScheduledJob};

#[derive(Debug, Clone)]
pub struct SchedulerRepository {
    jobs: JobRepository,
    analytics: AnalyticsRepository,
}

impl SchedulerRepository {
    pub fn new(db: &DbPool) -> anyhow::Result<Self> {
        Ok(Self {
            jobs: JobRepository::new(db)?,
            analytics: AnalyticsRepository::new(db)?,
        })
    }

    pub async fn upsert_job(
        &self,
        job_name: &str,
        schedule: &str,
        enabled: bool,
    ) -> anyhow::Result<()> {
        self.jobs.upsert_job(job_name, schedule, enabled).await
    }

    pub async fn find_job(&self, job_name: &str) -> anyhow::Result<Option<ScheduledJob>> {
        self.jobs.find_job(job_name).await
    }

    pub async fn list_enabled_jobs(&self) -> anyhow::Result<Vec<ScheduledJob>> {
        self.jobs.list_enabled_jobs().await
    }

    pub async fn update_job_execution(
        &self,
        job_name: &str,
        status: JobStatus,
        error: Option<&str>,
        next_run: Option<DateTime<Utc>>,
    ) -> anyhow::Result<()> {
        self.jobs
            .update_job_execution(job_name, status, error, next_run)
            .await
    }

    pub async fn increment_run_count(&self, job_name: &str) -> anyhow::Result<()> {
        self.jobs.increment_run_count(job_name).await
    }

    pub async fn cleanup_empty_contexts(&self, hours_old: i64) -> anyhow::Result<u64> {
        self.analytics.cleanup_empty_contexts(hours_old).await
    }
}
