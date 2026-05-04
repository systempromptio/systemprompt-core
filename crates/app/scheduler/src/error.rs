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

/// Canonical error type for `systemprompt-scheduler` public APIs.
#[derive(Debug, Error)]
pub enum SchedulerError {
    /// A scheduled job by the supplied name was not registered or persisted.
    #[error("Job not found: {job_name}")]
    JobNotFound {
        /// The job name that could not be located.
        job_name: String,
    },

    /// The supplied cron expression failed validation.
    #[error("Invalid cron schedule: {schedule}")]
    InvalidSchedule {
        /// The offending cron expression.
        schedule: String,
    },

    /// A scheduled job body returned a failure or panicked during execution.
    #[error("Job execution failed: {job_name} - {error}")]
    JobExecutionFailed {
        /// Name of the job whose execution failed.
        job_name: String,
        /// The underlying error message captured from the failed run.
        error: String,
    },

    /// Underlying `sqlx` driver error (connection, decode, protocol).
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Repository-level error from `systemprompt-database` abstractions.
    #[error("Repository error: {0}")]
    Repository(#[from] systemprompt_database::RepositoryError),

    /// Failure originating in the analytics domain (fingerprint, session).
    #[error("Analytics error: {0}")]
    Analytics(#[from] systemprompt_analytics::AnalyticsError),

    /// Failure originating in the users domain (banned-IP, identity).
    #[error("Users error: {0}")]
    Users(#[from] systemprompt_users::UserError),

    /// Underlying `tokio-cron-scheduler` error (registration, start, dispatch).
    #[error("Cron scheduler error: {0}")]
    CronScheduler(#[from] tokio_cron_scheduler::JobSchedulerError),

    /// Configuration value missing or malformed.
    #[error("Configuration error: {message}")]
    ConfigError {
        /// Human-readable description of the configuration failure.
        message: String,
    },

    /// `start` was invoked while the scheduler was already running.
    #[error("Scheduler already running")]
    AlreadyRunning,

    /// A lifecycle method was invoked before [`crate::SchedulerService::new`].
    #[error("Scheduler not initialized")]
    NotInitialized,

    /// A required runtime context value (e.g.
    /// [`systemprompt_database::DbPool`]) was missing from the
    /// [`systemprompt_traits::JobContext`].
    #[error("Job context missing dependency: {0}")]
    MissingContext(String),

    /// A spawned job panicked; the payload is captured as a string.
    #[error("Job panicked: {0}")]
    Panic(String),

    /// I/O failure (logging, filesystem).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Catch-all for upstream errors that have not been narrowed.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl SchedulerError {
    /// Construct a [`SchedulerError::JobNotFound`] from any string-like name.
    pub fn job_not_found(job_name: impl Into<String>) -> Self {
        Self::JobNotFound {
            job_name: job_name.into(),
        }
    }

    /// Construct a [`SchedulerError::InvalidSchedule`] from a cron expression.
    pub fn invalid_schedule(schedule: impl Into<String>) -> Self {
        Self::InvalidSchedule {
            schedule: schedule.into(),
        }
    }

    /// Construct a [`SchedulerError::JobExecutionFailed`] from job + error
    /// strings.
    pub fn job_execution_failed(job_name: impl Into<String>, error: impl Into<String>) -> Self {
        Self::JobExecutionFailed {
            job_name: job_name.into(),
            error: error.into(),
        }
    }

    /// Construct a [`SchedulerError::ConfigError`] from any string-like
    /// message.
    pub fn config_error(message: impl Into<String>) -> Self {
        Self::ConfigError {
            message: message.into(),
        }
    }

    /// Construct a [`SchedulerError::MissingContext`] from a dependency name.
    pub fn missing_context(name: impl Into<String>) -> Self {
        Self::MissingContext(name.into())
    }

    /// Construct a [`SchedulerError::Panic`] from a panic payload string.
    pub fn panic(message: impl Into<String>) -> Self {
        Self::Panic(message.into())
    }
}

impl From<SchedulerError> for systemprompt_provider_contracts::ProviderError {
    fn from(err: SchedulerError) -> Self {
        Self::Internal(anyhow::Error::new(err))
    }
}

/// Convenience alias used by all public, non-trait scheduler APIs.
pub type SchedulerResult<T> = Result<T, SchedulerError>;
