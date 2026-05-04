//! Scheduler traits for triggering jobs and inspecting their status.

use async_trait::async_trait;
use std::sync::Arc;

/// Errors returned by scheduler implementations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SchedulerError {
    /// No registered job matches the supplied name.
    #[error("Job not found: {0}")]
    JobNotFound(String),

    /// The scheduler subsystem is not currently running.
    #[error("Scheduler unavailable: {0}")]
    Unavailable(String),

    /// A job ran but reported failure.
    #[error("Job execution failed: {0}")]
    ExecutionFailed(String),

    /// Catch-all for unexpected scheduler failures.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Lifecycle phase of a scheduled job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum JobStatus {
    /// Registered but never run.
    Pending,
    /// Currently executing.
    Running,
    /// Last run completed successfully.
    Success,
    /// Last run reported failure.
    Failed,
    /// Disabled by configuration.
    Disabled,
}

/// Snapshot of a single job's state.
#[derive(Debug, Clone)]
pub struct JobInfo {
    /// Job name as registered with the scheduler.
    pub name: String,
    /// Current lifecycle status.
    pub status: JobStatus,
    /// Timestamp of the most recent run.
    pub last_run: Option<chrono::DateTime<chrono::Utc>>,
    /// Timestamp of the next scheduled run, if known.
    pub next_run: Option<chrono::DateTime<chrono::Utc>>,
    /// Total number of runs recorded.
    pub run_count: i64,
    /// Error message produced by the most recent failure.
    pub last_error: Option<String>,
}

/// Trigger jobs and inspect their state.
///
/// `#[async_trait]` is required because the trait is consumed as
/// `Arc<dyn JobTrigger>` via [`DynJobTrigger`].
#[async_trait]
pub trait JobTrigger: Send + Sync {
    /// Force `job_name` to run immediately.
    async fn trigger_job(&self, job_name: &str) -> Result<(), SchedulerError>;

    /// Return the current [`JobInfo`] for `job_name`.
    async fn get_job_status(&self, job_name: &str) -> Result<JobInfo, SchedulerError>;

    /// Return [`JobInfo`] for every registered job.
    async fn list_jobs(&self) -> Result<Vec<JobInfo>, SchedulerError>;

    /// Report whether the scheduler subsystem is currently active.
    async fn is_running(&self) -> bool;
}

/// Start, stop, and probe the scheduler subsystem itself.
///
/// `#[async_trait]` is required because the trait is consumed as
/// `Arc<dyn SchedulerLifecycle>` via [`DynSchedulerLifecycle`].
#[async_trait]
pub trait SchedulerLifecycle: Send + Sync {
    /// Start the scheduler.
    async fn start(&self) -> Result<(), SchedulerError>;

    /// Stop the scheduler and drain in-flight jobs.
    async fn stop(&self) -> Result<(), SchedulerError>;

    /// Run a liveness probe.
    async fn health_check(&self) -> Result<bool, SchedulerError>;
}

/// Shared `Arc` alias for [`JobTrigger`].
pub type DynJobTrigger = Arc<dyn JobTrigger>;

/// Shared `Arc` alias for [`SchedulerLifecycle`].
pub type DynSchedulerLifecycle = Arc<dyn SchedulerLifecycle>;
