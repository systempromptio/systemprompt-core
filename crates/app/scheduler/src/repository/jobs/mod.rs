use crate::models::{JobStatus, ScheduledJob};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::ScheduledJobId;

#[derive(Debug, Clone)]
pub struct JobRepository {
    pool: Arc<PgPool>,
}

impl JobRepository {
    pub fn new(db: &DbPool) -> anyhow::Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    pub async fn upsert_job(
        &self,
        job_name: &str,
        schedule: &str,
        enabled: bool,
    ) -> anyhow::Result<()> {
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
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn find_job(&self, job_name: &str) -> anyhow::Result<Option<ScheduledJob>> {
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

    pub async fn list_enabled_jobs(&self) -> anyhow::Result<Vec<ScheduledJob>> {
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

    pub async fn update_job_execution(
        &self,
        job_name: &str,
        status: JobStatus,
        error: Option<&str>,
        next_run: Option<DateTime<Utc>>,
    ) -> anyhow::Result<()> {
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
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn increment_run_count(&self, job_name: &str) -> anyhow::Result<()> {
        sqlx::query!(
            "UPDATE scheduled_jobs SET run_count = run_count + 1 WHERE job_name = $1",
            job_name
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }
}
