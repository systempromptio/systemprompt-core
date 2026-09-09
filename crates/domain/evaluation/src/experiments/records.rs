//! Persisted execution and accounting views retain incomplete outcomes
//! explicitly.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    EvalBudgetId, EvalExecutionId, EvalExperimentId, EvalRevisionId, UserId,
};

use super::ExperimentSpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Queued,
    Running,
    AwaitingApproval,
    Completed,
    Error,
    Cancelled,
    Blocked,
    BudgetExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentStatus {
    Queued,
    Running,
    Completed,
    Cancelled,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetRecord {
    pub id: EvalBudgetId,
    pub cap: i64,
    pub reserved: i64,
    pub settled: i64,
    pub frozen: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentRecord {
    pub id: EvalExperimentId,
    pub owner_id: UserId,
    pub spec: ExperimentSpec,
    pub spec_digest: String,
    pub budget_id: EvalBudgetId,
    pub status: ExperimentStatus,
    pub created_at: DateTime<Utc>,
    pub accounting: BudgetRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub id: EvalExecutionId,
    pub experiment_id: EvalExperimentId,
    pub variant_index: i32,
    pub case_revision_id: EvalRevisionId,
    pub repetition: i32,
    pub status: ExecutionStatus,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub fencing_token: i64,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub result: Option<crate::repository::experiments::ExecutionCompletion>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExperimentDetail {
    pub experiment: ExperimentRecord,
    pub executions: Vec<ExecutionRecord>,
}
