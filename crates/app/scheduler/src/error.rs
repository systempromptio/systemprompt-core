//! Typed error boundary for the `systemprompt-scheduler` crate.
//!
//! [`SchedulerError`] is the canonical error returned from public, non-trait
//! signatures (services, repositories, lifecycle helpers). It composes
//! [`tokio_cron_scheduler::JobSchedulerError`],
//! [`systemprompt_traits::RepositoryError`],
//! [`systemprompt_analytics::AnalyticsError`], and
//! [`systemprompt_users::UserError`] via `#[from]` so `?` propagation works
//! transparently for every internal call site. A [`sqlx::Error`] is
//! classified through [`systemprompt_traits::RepositoryError`] so a
//! not-found or constraint failure keeps its class.
//!
//! Provider trait implementations (e.g. [`systemprompt_traits::Job`]) keep
//! returning [`systemprompt_provider_contracts::ProviderResult`] — the
//! `From<SchedulerError> for ProviderError` bridge below lets job bodies
//! propagate `SchedulerError` through `?` without bespoke `map_err` chains.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::JobName;
use systemprompt_models::profile::OtlpSignal;
use systemprompt_provider_contracts::{MissingDependency, ProviderError};
use systemprompt_traits::{BoxedSource, RepositoryError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("Job not found: {job_name}")]
    JobNotFound { job_name: JobName },

    #[error(
        "Scheduler config references job(s) not present in the inventory catalog: {names}. \
         Every name in `jobs`/`bootstrap_jobs` must be registered via `submit_job!`."
    )]
    UnknownJob { names: String },

    #[error("Invalid cron schedule: {schedule}")]
    InvalidSchedule { schedule: String },

    #[error("Job execution failed: {job_name} - {source}")]
    JobExecutionFailed {
        job_name: JobName,
        #[source]
        source: ProviderError,
    },

    #[error("Invalid parameter format '{parameter}'. Use KEY=VALUE format.")]
    InvalidJobParameter { parameter: String },

    #[error("No jobs found with tag '{tag}'")]
    NoJobsWithTag { tag: String },

    #[error("Specify job name(s), use --all, or use --tag <tag> to run jobs")]
    NoJobsSelected,

    #[error("Repository error: {0}")]
    Repository(#[from] RepositoryError),

    #[error("Analytics error: {0}")]
    Analytics(#[from] systemprompt_analytics::AnalyticsError),

    #[error("Users error: {0}")]
    Users(#[from] systemprompt_users::UserError),

    #[error("Cron scheduler error: {0}")]
    CronScheduler(#[from] tokio_cron_scheduler::JobSchedulerError),

    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    #[error("Scheduler already running")]
    AlreadyRunning,

    #[error("Scheduler not initialized")]
    NotInitialized,

    #[error("Job context: {0}")]
    MissingContext(#[from] MissingDependency),

    #[error("Job panicked: {0}")]
    Panic(String),

    #[error("Distributed lock error: {0}")]
    DistributedLock(#[source] RepositoryError),

    #[error("Inventory refresh failed: {0}")]
    Inventory(#[from] systemprompt_runtime::managed::OrchestrationError),

    #[error("Managed marketplace error: {0}")]
    Managed(#[from] systemprompt_marketplace::managed::ManagedError),

    #[error("Unknown OTLP export signal '{signal}'")]
    UnknownOtlpSignal { signal: String },

    #[error("OTLP {signal} export failed: {source}")]
    OtlpExport {
        signal: OtlpSignal,
        #[source]
        source: BoxedSource,
    },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Process supervision error: {0}")]
    Supervision(#[from] systemprompt_loader::subprocess::SupervisionError),

    #[error("Port {port} still held by PID(s) {holders:?}")]
    PortOccupied { port: u16, holders: Vec<u32> },
}

impl SchedulerError {
    pub const fn job_not_found(job_name: JobName) -> Self {
        Self::JobNotFound { job_name }
    }

    pub fn invalid_schedule(schedule: impl Into<String>) -> Self {
        Self::InvalidSchedule {
            schedule: schedule.into(),
        }
    }

    pub const fn job_execution_failed(job_name: JobName, source: ProviderError) -> Self {
        Self::JobExecutionFailed { job_name, source }
    }

    pub fn config_error(message: impl Into<String>) -> Self {
        Self::ConfigError {
            message: message.into(),
        }
    }

    pub fn panic(message: impl Into<String>) -> Self {
        Self::Panic(message.into())
    }
}

impl From<sqlx::Error> for SchedulerError {
    fn from(err: sqlx::Error) -> Self {
        Self::Repository(RepositoryError::from(err))
    }
}

impl From<SchedulerError> for ProviderError {
    fn from(err: SchedulerError) -> Self {
        Self::Internal(Box::new(err))
    }
}

pub type SchedulerResult<T> = Result<T, SchedulerError>;
