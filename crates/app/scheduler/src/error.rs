//! Typed error boundary for the `systemprompt-scheduler` crate.
//!
//! [`SchedulerError`] is the canonical error returned from public, non-trait
//! signatures (services, repositories, lifecycle helpers). It composes
//! [`sqlx::Error`], [`tokio_cron_scheduler::JobSchedulerError`],
//! [`systemprompt_database::RepositoryError`],
//! [`systemprompt_analytics::AnalyticsError`], and
//! [`systemprompt_users::UserError`] via `#[from]` so `?` propagation works
//! transparently for every internal call site.
//!
//! Provider trait implementations (e.g. [`systemprompt_traits::Job`]) keep
//! returning [`systemprompt_provider_contracts::ProviderResult`] — the
//! `From<SchedulerError> for ProviderError` bridge below lets job bodies
//! propagate `SchedulerError` through `?` without bespoke `map_err` chains.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("Job not found: {job_name}")]
    JobNotFound { job_name: String },

    #[error("Invalid cron schedule: {schedule}")]
    InvalidSchedule { schedule: String },

    #[error("Job execution failed: {job_name} - {error}")]
    JobExecutionFailed { job_name: String, error: String },

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Repository error: {0}")]
    Repository(#[from] systemprompt_database::RepositoryError),

    #[error("Analytics error: {0}")]
    Analytics(#[from] systemprompt_analytics::AnalyticsError),

    #[error("Users error: {0}")]
    Users(#[from] systemprompt_users::UserError),

    #[error("Cron scheduler error: {0}")]
    CronScheduler(#[from] tokio_cron_scheduler::JobSchedulerError),

    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    #[error(
        "Job '{job_name}' declares owner '{owner}', but no active user with that name exists. \
         Owners must resolve to a real `users` row (status='active')."
    )]
    UnresolvedJobOwner { job_name: String, owner: String },

    #[error("Scheduler already running")]
    AlreadyRunning,

    #[error("Scheduler not initialized")]
    NotInitialized,

    #[error("Job context missing dependency: {0}")]
    MissingContext(String),

    #[error("Job panicked: {0}")]
    Panic(String),

    #[error("Distributed lock error: {0}")]
    DistributedLock(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("internal: {0}")]
    Internal(String),
}

impl SchedulerError {
    pub fn job_not_found(job_name: impl Into<String>) -> Self {
        Self::JobNotFound {
            job_name: job_name.into(),
        }
    }

    pub fn invalid_schedule(schedule: impl Into<String>) -> Self {
        Self::InvalidSchedule {
            schedule: schedule.into(),
        }
    }

    pub fn job_execution_failed(job_name: impl Into<String>, error: impl Into<String>) -> Self {
        Self::JobExecutionFailed {
            job_name: job_name.into(),
            error: error.into(),
        }
    }

    pub fn config_error(message: impl Into<String>) -> Self {
        Self::ConfigError {
            message: message.into(),
        }
    }

    pub fn missing_context(name: impl Into<String>) -> Self {
        Self::MissingContext(name.into())
    }

    pub fn panic(message: impl Into<String>) -> Self {
        Self::Panic(message.into())
    }
}

impl From<SchedulerError> for systemprompt_provider_contracts::ProviderError {
    fn from(err: SchedulerError) -> Self {
        Self::Internal(err.to_string())
    }
}

pub type SchedulerResult<T> = Result<T, SchedulerError>;
