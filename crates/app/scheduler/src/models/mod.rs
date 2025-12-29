use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use systemprompt_identifiers::ScheduledJobId;

pub use systemprompt_models::services::{JobConfig, SchedulerConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Success,
    Failed,
    Running,
}

impl JobStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Running => "running",
        }
    }
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SchedulerError {
    #[error("Job not found: {job_name}")]
    JobNotFound { job_name: String },

    #[error("Invalid cron schedule: {schedule}")]
    InvalidSchedule { schedule: String },

    #[error("Job execution failed: {job_name} - {error}")]
    JobExecutionFailed { job_name: String, error: String },

    #[error("Database operation failed")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    #[error("Scheduler already running")]
    AlreadyRunning,

    #[error("Scheduler not initialized")]
    NotInitialized,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ScheduledJob {
    pub id: ScheduledJobId,
    pub job_name: String,
    pub schedule: String,
    pub enabled: bool,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: Option<DateTime<Utc>>,
    pub last_status: Option<String>,
    pub last_error: Option<String>,
    pub run_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
