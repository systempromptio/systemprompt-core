//! Domain models exposed by the scheduler crate.
//!
//! This module re-exports [`JobConfig`] / [`SchedulerConfig`] from
//! `systemprompt-models` and defines the persistence-layer types
//! ([`ScheduledJob`], [`JobStatus`]) used by [`crate::repository`].
//!
//! The crate-wide error type lives in [`crate::error`].

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
