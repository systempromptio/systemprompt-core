//! Domain models exposed by the scheduler crate.
//!
//! This module re-exports [`JobConfig`] / [`SchedulerConfig`] from
//! `systemprompt-models` and defines the persistence-layer types
//! ([`ScheduledJob`], [`JobStatus`]) used by [`crate::repository`].
//!
//! The crate-wide error type lives in [`crate::error`]; it is re-exported here
//! as [`SchedulerError`] for backwards-compatible access via `crate::models`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use systemprompt_identifiers::ScheduledJobId;

pub use systemprompt_models::services::{JobConfig, SchedulerConfig};

pub use crate::error::{SchedulerError, SchedulerResult};

/// Last-known execution status of a scheduled job, stored in the
/// `scheduled_jobs.last_status` column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    /// Last execution returned [`systemprompt_traits::JobResult::success`].
    Success,
    /// Last execution returned a failed [`systemprompt_traits::JobResult`] or
    /// the job body returned an error.
    Failed,
    /// Job is currently executing or has been claimed for execution.
    Running,
}

impl JobStatus {
    /// Stable string representation used for database persistence and
    /// structured logging.
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

/// Persistent scheduled-job record matching the `scheduled_jobs` table.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ScheduledJob {
    /// Typed primary key for this scheduled job.
    pub id: ScheduledJobId,
    /// Stable, human-readable identifier (matches the `Job::name` constant).
    pub job_name: String,
    /// Cron expression governing recurring execution.
    pub schedule: String,
    /// Whether the scheduler is currently allowed to dispatch this job.
    pub enabled: bool,
    /// Wall-clock timestamp of the most recent execution attempt.
    pub last_run: Option<DateTime<Utc>>,
    /// Wall-clock timestamp of the next planned execution (when known).
    pub next_run: Option<DateTime<Utc>>,
    /// Stringified last [`JobStatus`] for cheap display without a JOIN.
    pub last_status: Option<String>,
    /// Captured error message from the most recent failing execution.
    pub last_error: Option<String>,
    /// Total successful + failed runs since job registration.
    pub run_count: i32,
    /// Row creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp (any column change).
    pub updated_at: DateTime<Utc>,
}
