//! Scheduler traits for job triggering and status.

use async_trait::async_trait;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SchedulerError {
    #[error("Job not found: {0}")]
    JobNotFound(String),

    #[error("Scheduler unavailable: {0}")]
    Unavailable(String),

    #[error("Job execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum JobStatus {
    Pending,
    Running,
    Success,
    Failed,
    Disabled,
}

#[derive(Debug, Clone)]
pub struct JobInfo {
    pub name: String,
    pub status: JobStatus,
    pub last_run: Option<chrono::DateTime<chrono::Utc>>,
    pub next_run: Option<chrono::DateTime<chrono::Utc>>,
    pub run_count: i64,
    pub last_error: Option<String>,
}

#[async_trait]
pub trait JobTrigger: Send + Sync {
    async fn trigger_job(&self, job_name: &str) -> Result<(), SchedulerError>;

    async fn get_job_status(&self, job_name: &str) -> Result<JobInfo, SchedulerError>;

    async fn list_jobs(&self) -> Result<Vec<JobInfo>, SchedulerError>;

    async fn is_running(&self) -> bool;
}

#[async_trait]
pub trait SchedulerLifecycle: Send + Sync {
    async fn start(&self) -> Result<(), SchedulerError>;

    async fn stop(&self) -> Result<(), SchedulerError>;

    async fn health_check(&self) -> Result<bool, SchedulerError>;
}

pub type DynJobTrigger = Arc<dyn JobTrigger>;

pub type DynSchedulerLifecycle = Arc<dyn SchedulerLifecycle>;
