//! Approval, measurement, suggestion, cleanup and restart reconciliation state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use systemprompt_identifiers::{
    AiRequestId, EvalApprovalId, EvalBudgetId, EvalExecutionId, EvalExperimentId, EvalSuggestionId,
    UserId,
};

use super::{BudgetRepository, ExecutionLease, ReservationAdmission};

#[path = "lifecycle_approvals.rs"]
mod approvals;
#[path = "lifecycle_recovery.rs"]
mod recovery;
use crate::Result;
use crate::experiments::invalid;
pub use approvals::ApprovalVerdict;
pub use recovery::CleanupReport;

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
    pub accounting_status: String,
    pub verified_success: bool,
}

impl DeterministicMeasurement {
    fn validate(&self) -> Result<()> {
        const CHECKS: [&str; 5] = [
            "arithmetic",
            "permissions",
            "evidence_references",
            "install_integrity",
            "write_readbacks",
        ];
        if self.attempted_cost_microdollars < 0
            || !matches!(
                self.accounting_status.as_str(),
                "complete" | "partial" | "unknown"
            )
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

#[derive(Debug, Clone, Serialize)]
pub struct ComparisonReport {
    pub experiment_id: EvalExperimentId,
    pub attempted: i64,
    pub completed: i64,
    pub hard_failures: i64,
    pub unscored: i64,
    pub verified_successes: i64,
    pub attempted_cost_microdollars: i64,
    pub cost_per_verified_success_microdollars: Option<i64>,
    pub accounting_complete: i64,
    pub accounting_total: i64,
    pub variants: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecutionAccounting {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub tool_calls: u64,
    pub attempted_cost_microdollars: i64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionApproval {
    pub id: EvalApprovalId,
    pub execution_id: EvalExecutionId,
    pub operation: serde_json::Value,
    pub precondition_digest: String,
    pub status: String,
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

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone)]
pub struct EvaluationLifecycleRepository {
    pub(crate) pool: PgPool,
    budgets: BudgetRepository,
    admission: std::sync::Arc<dyn crate::capabilities::ExecutionAdmission>,
}

impl EvaluationLifecycleRepository {
    pub fn new(pool: PgPool) -> Self {
        Self::with_admission(
            pool,
            std::sync::Arc::new(crate::capabilities::VerifiedExecutionAdmission),
        )
    }

    pub fn with_admission(
        pool: PgPool,
        admission: std::sync::Arc<dyn crate::capabilities::ExecutionAdmission>,
    ) -> Self {
        Self {
            admission,
            budgets: BudgetRepository::new(pool.clone()),
            pool,
        }
    }

    pub async fn should_generate_suggestion(
        &self,
        owner: &UserId,
        execution: &EvalExecutionId,
        limit: u32,
    ) -> Result<bool> {
        if limit == 0 {
            return Ok(false);
        }
        Ok(sqlx::query_scalar!(r#"SELECT EXISTS(
            SELECT 1 FROM eval_executions target
            JOIN eval_experiments e ON e.id=target.experiment_id
            JOIN eval_resource_revisions c ON c.id=target.case_revision_id AND c.owner_id=e.owner_id
            WHERE e.owner_id=$1 AND target.id=$2 AND target.variant_index=1 AND target.repetition=0
              AND c.content->'content'->>'partition'='development'
              AND (SELECT count(*) FROM eval_executions ranked
                   JOIN eval_resource_revisions rc ON rc.id=ranked.case_revision_id AND rc.owner_id=e.owner_id
                   WHERE ranked.experiment_id=e.id AND ranked.variant_index=1 AND ranked.repetition=0
                     AND rc.content->'content'->>'partition'='development'
                     AND (ranked.case_revision_id,ranked.id)<=(target.case_revision_id,target.id)) <= $3
              AND NOT EXISTS(SELECT 1 FROM eval_suggestions s WHERE s.owner_id=e.owner_id AND target.id=ANY(s.supporting_execution_ids))
        )"#, owner.as_str(), execution.as_str(), i64::from(limit)).fetch_one(&self.pool).await?.unwrap_or(false))
    }

    pub async fn execution_accounting(
        &self,
        owner: &UserId,
        execution: &EvalExecutionId,
    ) -> Result<ExecutionAccounting> {
        let row = sqlx::query!(r#"SELECT count(m.request_id) AS "requests!",count(*) FILTER(WHERE q.status='completed' AND q.completed_at IS NOT NULL AND q.input_tokens IS NOT NULL AND q.output_tokens IS NOT NULL AND r.actual IS NOT NULL) AS "complete!",COALESCE(sum(q.input_tokens),0)::BIGINT AS "input_tokens!",COALESCE(sum(q.output_tokens),0)::BIGINT AS "output_tokens!",COALESCE(sum(q.cost_microdollars),0)::BIGINT AS "cost!",(SELECT count(*) FROM ai_request_tool_calls t WHERE t.request_id IN (SELECT request_id FROM eval_request_reservations WHERE execution_id=$2)) AS "tool_calls!" FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id LEFT JOIN eval_request_reservations m ON m.execution_id=x.id LEFT JOIN ai_requests q ON q.id=m.request_id AND q.user_id=e.owner_id LEFT JOIN eval_budget_reservations r ON r.id=m.reservation_id WHERE e.owner_id=$1 AND x.id=$2 GROUP BY x.id"#,
            owner.as_str(), execution.as_str()).fetch_optional(&self.pool).await?.ok_or_else(|| crate::experiments::missing("Execution accounting unavailable in this scope"))?;
        let requests = row.requests;
        let complete = row.complete;
        let status = if requests == 0 {
            "unknown"
        } else if complete == requests {
            "complete"
        } else {
            "partial"
        };
        let input = row.input_tokens;
        let output = row.output_tokens;
        Ok(ExecutionAccounting {
            input_tokens: (requests > 0).then(|| u64::try_from(input).unwrap_or_default()),
            output_tokens: (requests > 0).then(|| u64::try_from(output).unwrap_or_default()),
            tool_calls: u64::try_from(row.tool_calls).unwrap_or_default(),
            attempted_cost_microdollars: row.cost,
            status: status.to_owned(),
        })
    }

    pub async fn record_measurement(
        &self,
        owner: &UserId,
        lease: &ExecutionLease,
        measurement: &DeterministicMeasurement,
    ) -> Result<()> {
        measurement.validate()?;
        let scoring = sqlx::query!("SELECT r.content,m.manifest FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id JOIN eval_resource_revisions r ON r.id=e.spec->>'rubric' AND r.owner_id=e.owner_id JOIN eval_execution_evidence m ON m.execution_id=x.id WHERE e.owner_id=$1 AND x.id=$2",
            owner.as_str(), lease.execution_id.as_str()).fetch_optional(&self.pool).await?
            .ok_or_else(|| crate::experiments::conflict("Measurement requires retained execution evidence"))?;
        let resource: crate::experiments::resources::ResourceContent =
            serde_json::from_value(scoring.content)?;
        let crate::experiments::resources::ResourceContent::Rubric(rubric) = resource else {
            return Err(invalid("Experiment rubric changed type"));
        };
        let evidence_manifest: crate::experiments::execution::ExecutionEvidence =
            serde_json::from_value(scoring.manifest)?;
        let mut references = BTreeSet::new();
        for artifact in &evidence_manifest.artifacts {
            references.insert(artifact.relative_path.clone());
            references.insert(artifact.sha256.clone());
        }
        for request in &evidence_manifest.requests {
            references.insert(request.as_str().to_owned());
        }
        match (&measurement.judgment, measurement.quality_milli) {
            (Some(judgment), Some(quality)) => {
                let outcome = crate::experiments::scoring::score(&rubric, judgment, &references)?;
                if quality != outcome.score_milli
                    || measurement.verified_success
                        != (outcome.passed
                            && measurement.hard_failures.is_empty()
                            && measurement.checks.values().all(|value| *value))
                {
                    return Err(invalid(
                        "Semantic score or verified-success state differs from its evidence",
                    ));
                }
            },
            (None, None) => {},
            _ => {
                return Err(invalid(
                    "Missing or invalid semantic judgment must remain unscored",
                ));
            },
        }
        let changed = sqlx::query!("INSERT INTO eval_execution_measurements(execution_id,hard_failures,deterministic_checks,judgment,quality_milli,latency_ms,input_tokens,output_tokens,tool_calls,attempted_cost_microdollars,accounting_status,verified_success) SELECT x.id,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15 FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE x.id=$1 AND e.owner_id=$2 AND x.lease_owner=$3 AND x.fencing_token=$4 AND x.status='completed' ON CONFLICT(execution_id) DO NOTHING",
            lease.execution_id.as_str(), owner.as_str(), lease.worker_id.as_str(), lease.fencing_token, &measurement.hard_failures, serde_json::to_value(&measurement.checks)?, serde_json::to_value(&measurement.judgment)?, measurement.quality_milli.map(i32::try_from).transpose().map_err(|error| invalid(&format!("Quality overflow: {error}")))?, i64::try_from(measurement.latency_ms).map_err(|error| invalid(&format!("Latency overflow: {error}")))?, measurement.input_tokens.map(i64::try_from).transpose().map_err(|error| invalid(&format!("Token overflow: {error}")))?, measurement.output_tokens.map(i64::try_from).transpose().map_err(|error| invalid(&format!("Token overflow: {error}")))?, i64::try_from(measurement.tool_calls).map_err(|error| invalid(&format!("Tool count overflow: {error}")))?, measurement.attempted_cost_microdollars, &measurement.accounting_status, measurement.verified_success).execute(&self.pool).await?;
        if changed.rows_affected() != 1 {
            return Err(crate::experiments::conflict(
                "Measurement is immutable or execution is unavailable",
            ));
        }
        Ok(())
    }

    pub async fn comparison(
        &self,
        owner: &UserId,
        experiment: &EvalExperimentId,
    ) -> Result<ComparisonReport> {
        let row = sqlx::query!(r#"SELECT count(*) AS "attempted!",count(*) FILTER(WHERE x.status='completed') AS "completed!",count(*) FILTER(WHERE cardinality(COALESCE(m.hard_failures,ARRAY[]::TEXT[]))>0) AS "hard_failures!",count(*) FILTER(WHERE m.quality_milli IS NULL) AS "unscored!",count(*) FILTER(WHERE m.verified_success) AS "successes!",COALESCE(sum(m.attempted_cost_microdollars),0)::BIGINT AS "cost!",count(*) FILTER(WHERE m.accounting_status='complete') AS "accounting_complete!",count(m.execution_id) AS "accounting_total!",jsonb_agg(jsonb_build_object('execution_id',x.id,'variant',x.variant_index,'case_revision_id',x.case_revision_id,'repetition',x.repetition,'status',x.status,'measurement',to_jsonb(m)) ORDER BY x.variant_index,x.case_revision_id,x.repetition) AS "variants!" FROM eval_experiments e JOIN eval_executions x ON x.experiment_id=e.id LEFT JOIN eval_execution_measurements m ON m.execution_id=x.id WHERE e.owner_id=$1 AND e.id=$2 GROUP BY e.id"#,
            owner.as_str(), experiment.as_str()).fetch_optional(&self.pool).await?.ok_or_else(|| crate::experiments::missing("Experiment unavailable in this scope"))?;
        let successes = row.successes;
        let cost = row.cost;
        Ok(ComparisonReport {
            experiment_id: experiment.clone(),
            attempted: row.attempted,
            completed: row.completed,
            hard_failures: row.hard_failures,
            unscored: row.unscored,
            verified_successes: successes,
            attempted_cost_microdollars: cost,
            cost_per_verified_success_microdollars: (successes > 0).then(|| cost / successes),
            accounting_complete: row.accounting_complete,
            accounting_total: row.accounting_total,
            variants: row.variants,
        })
    }
}
