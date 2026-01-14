use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobInfo {
    pub name: String,
    pub description: String,
    pub schedule: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobListOutput {
    pub jobs: Vec<JobInfo>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobRunOutput {
    pub job_name: String,
    pub status: String,
    pub duration_ms: u64,
    pub result: JobRunResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobRunResult {
    pub success: bool,
    pub message: Option<String>,
    pub items_processed: Option<u64>,
    pub items_failed: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionCleanupOutput {
    pub job_name: String,
    pub sessions_cleaned: i64,
    pub hours_threshold: i32,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LogCleanupOutput {
    pub job_name: String,
    pub entries_deleted: i64,
    pub days_threshold: i32,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobShowOutput {
    pub name: String,
    pub description: String,
    pub schedule: String,
    pub schedule_human: String,
    pub enabled: bool,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: Option<DateTime<Utc>>,
    pub last_status: Option<String>,
    pub last_error: Option<String>,
    pub run_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobHistoryEntry {
    pub job_name: String,
    pub status: String,
    pub run_at: DateTime<Utc>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobHistoryOutput {
    pub entries: Vec<JobHistoryEntry>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobEnableOutput {
    pub job_name: String,
    pub enabled: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BatchJobRunOutput {
    pub jobs_run: Vec<JobRunOutput>,
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DryRunOutput {
    pub job_name: String,
    pub would_affect: String,
    pub estimated_count: Option<i64>,
    pub message: String,
}
