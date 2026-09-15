//! Lifecycle records: measurements, accounting, approvals and suggestions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{EvalApprovalId, EvalBudgetId, EvalExecutionId, EvalExperimentId};

use crate::Result;
use crate::experiments::invalid;
use crate::models::{AccountingStatus, ApprovalStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeterministicMeasurement {
    pub hard_failures: Vec<String>,
    #[serde(alias = "deterministic_checks")]
    pub checks: BTreeMap<String, bool>,
    pub judgment: Option<crate::experiments::scoring::EvidenceJudgment>,
    pub quality_milli: Option<u32>,
    pub latency_ms: u64,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub tool_calls: u64,
    pub attempted_cost_microdollars: i64,
    pub accounting_status: AccountingStatus,
    pub verified_success: bool,
}

impl DeterministicMeasurement {
    pub(super) fn validate(&self) -> Result<()> {
        const CHECKS: [&str; 5] = [
            "arithmetic",
            "permissions",
            "evidence_references",
            "install_integrity",
            "write_readbacks",
        ];
        if self.attempted_cost_microdollars < 0
            || self.checks.len() != CHECKS.len()
            || CHECKS.iter().any(|name| !self.checks.contains_key(*name))
            || self.quality_milli.is_some_and(|score| score > 5000)
        {
            return Err(invalid(
                "Measurement requires all deterministic checks and bounded accounting",
            ));
        }
        if self.verified_success
            && (!self.hard_failures.is_empty()
                || self.checks.values().any(|value| !value)
                || self.quality_milli.is_none_or(|score| score < 4000))
        {
            return Err(invalid(
                "Verified success cannot bypass checks, hard failures, or the 4/5 threshold",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ExecutionAccounting {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub tool_calls: u64,
    pub attempted_cost_microdollars: i64,
    pub status: AccountingStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExecutionApproval {
    pub id: EvalApprovalId,
    pub execution_id: EvalExecutionId,
    pub operation: serde_json::Value,
    pub precondition_digest: String,
    pub status: ApprovalStatus,
}

#[derive(Debug, Clone, Copy)]
pub enum ApprovalDecision {
    Approve,
    Deny,
}

#[derive(Debug, Clone)]
pub enum ApprovalAuthorization {
    Authorized(EvalApprovalId),
    Pending(EvalApprovalId),
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SuggestionRequest {
    pub experiment_id: EvalExperimentId,
    pub budget_id: EvalBudgetId,
    pub operation_key: String,
    pub maximum_cost_microdollars: i64,
    pub supporting_execution_ids: Vec<EvalExecutionId>,
    pub proposed_changes: serde_json::Value,
    pub hypothesis: String,
    pub originating_evidence: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedSuggestion {
    pub proposed_changes: serde_json::Value,
    pub hypothesis: String,
    pub supporting_failures: Vec<String>,
    pub originating_evidence: Vec<String>,
}
