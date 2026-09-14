//! Persisted execution and accounting views retain incomplete outcomes
//! explicitly.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    EvalBudgetId, EvalExecutionId, EvalExperimentId, EvalRevisionId, UserId,
};

use super::ExperimentSpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentStatus {
    Queued,
    Running,
    Completed,
    Cancelled,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BudgetRecord {
    pub id: EvalBudgetId,
    pub cap: i64,
    pub reserved: i64,
    pub settled: i64,
    pub frozen: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExecutionRecord {
    pub id: EvalExecutionId,
    pub experiment_id: EvalExperimentId,
    pub variant_index: i32,
    pub case_revision_id: EvalRevisionId,
    pub repetition: i32,
    pub status: ExecutionStatus,
    pub lease_owner: Option<systemprompt_identifiers::EvalWorkerId>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub fencing_token: i64,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub result: Option<crate::repository::experiments::ExecutionCompletion>,
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct ExperimentDetail {
    pub experiment: ExperimentRecord,
    pub executions: Vec<ExecutionRecord>,
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct ExperimentPreflight {
    pub execution_count: u64,
    pub maximum_cost_microdollars: i64,
    pub available_microdollars: i64,
    pub affordable: bool,
    pub matrix_digest: String,
}
