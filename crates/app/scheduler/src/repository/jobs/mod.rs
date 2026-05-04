//! `scheduled_jobs` table access — read replica + write pool wrapper.

use crate::error::SchedulerResult;
use crate::models::{JobStatus, ScheduledJob};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::ScheduledJobId;

/// Repository for the `scheduled_jobs` table.
#[derive(Debug, Clone)]
pub struct JobRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl JobRepository {
    /// Construct a new repository, capturing both the read and write pool
    /// handles from the shared [`DbPool`].
    pub fn new(db: &DbPool) -> SchedulerResult<Self> {
        let pool = db.pool_arc()?;
        let write_pool = db.write_pool_arc()?;
        Ok(Self { pool, write_pool })
    }

    /// Insert a new `scheduled_jobs` row, or update `schedule`/`enabled` on
    /// conflict with the existing row keyed by `job_name`.
    pub async fn upsert_job(
        &self,
        job_name: &str,
        schedule: &str,
        enabled: bool,
    ) -> SchedulerResult<()> {
        let id = ScheduledJobId::generate();
        let now = Utc::now();

        sqlx::query!(
            r#"
            INSERT INTO scheduled_jobs (id, job_name, schedule, enabled, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT(job_name) DO UPDATE SET
                schedule = EXCLUDED.schedule,
                enabled = EXCLUDED.enabled,
                updated_at = EXCLUDED.updated_at
            "#,
            id.as_str(),
            job_name,
            schedule,
            enabled,
            now,
            now
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(())
    }

    /// Fetch the row keyed by `job_name`, returning `None` if absent.
    pub async fn find_job(&self, job_name: &str) -> SchedulerResult<Option<ScheduledJob>> {
        sqlx::query_as!(
            ScheduledJob,
            r#"
            SELECT id, job_name, schedule, enabled, last_run, next_run, last_status, last_error,
                   run_count, created_at, updated_at
            FROM scheduled_jobs
            WHERE job_name = $1
            "#,
            job_name
        )
        .fetch_optional(&*self.pool)
        .await
        .map_err(Into::into)
    }

    /// List every job row whose `enabled` flag is currently `true`,
    /// alphabetised by `job_name`.
    pub async fn list_enabled_jobs(&self) -> SchedulerResult<Vec<ScheduledJob>> {
        sqlx::query_as!(
            ScheduledJob,
            r#"
            SELECT id, job_name, schedule, enabled, last_run, next_run, last_status, last_error,
                   run_count, created_at, updated_at
            FROM scheduled_jobs
            WHERE enabled = true
            ORDER BY job_name
            "#
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    /// Persist the post-execution status, optional error message, and next
    /// scheduled run for a job.
    pub async fn update_job_execution(
        &self,
        job_name: &str,
        status: JobStatus,
        error: Option<&str>,
        next_run: Option<DateTime<Utc>>,
    ) -> SchedulerResult<()> {
        let now = Utc::now();
        let status_str = status.as_str();

        sqlx::query!(
            r#"
            UPDATE scheduled_jobs
            SET last_run = $1,
                last_status = $2,
                last_error = $3,
                next_run = $4,
                updated_at = $5
            WHERE job_name = $6
            "#,
            now,
            status_str,
            error,
            next_run,
            now,
            job_name
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(())
    }

    /// Increment the `run_count` for a job by 1.
    pub async fn increment_run_count(&self, job_name: &str) -> SchedulerResult<()> {
        sqlx::query!(
            "UPDATE scheduled_jobs SET run_count = run_count + 1 WHERE job_name = $1",
            job_name
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }
}
