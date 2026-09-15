//! Approval, measurement, suggestion, cleanup and restart reconciliation state.
//!
//! Recorded request usage belongs to the AI domain and is read through
//! `AiRequestTrace`; every other row here is owned by this crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use sqlx::PgPool;
use systemprompt_identifiers::{
    AiRequestId, EvalApprovalId, EvalExecutionId, EvalSuggestionId, UserId,
};

use super::{BudgetRepository, ExecutionLease};
use systemprompt_traits::DynAiRequestTrace;

mod accounting;
mod approvals;
mod comparison;
mod models;
mod recovery;
mod suggestion_operations;
use crate::Result;
use crate::experiments::invalid;
pub use approvals::ApprovalVerdict;
pub use comparison::{ComparisonReport, MeasurementRow, RetainedMeasurement};
pub use models::{
    ApprovalAuthorization, ApprovalDecision, DeterministicMeasurement, ExecutionAccounting,
    ExecutionApproval, GeneratedSuggestion, SuggestionRequest,
};
pub use recovery::CleanupReport;

#[derive(Clone)]
pub struct EvaluationLifecycleRepository {
    pub(crate) pool: PgPool,
    budgets: BudgetRepository,
    trace: DynAiRequestTrace,
    admission: std::sync::Arc<dyn crate::capabilities::ExecutionAdmission>,
}

impl std::fmt::Debug for EvaluationLifecycleRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvaluationLifecycleRepository")
            .finish_non_exhaustive()
    }
}

impl EvaluationLifecycleRepository {
    pub fn new(pool: PgPool, budgets: BudgetRepository, trace: DynAiRequestTrace) -> Self {
        Self::with_admission(
            pool,
            budgets,
            trace,
            std::sync::Arc::new(crate::capabilities::VerifiedExecutionAdmission),
        )
    }

    pub const fn with_admission(
        pool: PgPool,
        budgets: BudgetRepository,
        trace: DynAiRequestTrace,
        admission: std::sync::Arc<dyn crate::capabilities::ExecutionAdmission>,
    ) -> Self {
        Self {
            pool,
            budgets,
            trace,
            admission,
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
        let changed = sqlx::query!("INSERT INTO eval_execution_measurements(execution_id,hard_failures,deterministic_checks,judgment,quality_milli,latency_ms,input_tokens,output_tokens,tool_calls,attempted_cost_microdollars,accounting_status,verified_success) SELECT x.id,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15 FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE x.id=$1 AND e.owner_id=$2 AND x.lease_owner=$3 AND x.fencing_token=$4 AND x.status IN ('completed','blocked') ON CONFLICT(execution_id) DO NOTHING",
            lease.execution_id.as_str(), owner.as_str(), lease.worker_id.as_str(), lease.fencing_token, &measurement.hard_failures, serde_json::to_value(&measurement.checks)?, serde_json::to_value(&measurement.judgment)?, measurement.quality_milli.map(i32::try_from).transpose().map_err(|error| invalid(&format!("Quality overflow: {error}")))?, i64::try_from(measurement.latency_ms).map_err(|error| invalid(&format!("Latency overflow: {error}")))?, measurement.input_tokens.map(i64::try_from).transpose().map_err(|error| invalid(&format!("Token overflow: {error}")))?, measurement.output_tokens.map(i64::try_from).transpose().map_err(|error| invalid(&format!("Token overflow: {error}")))?, i64::try_from(measurement.tool_calls).map_err(|error| invalid(&format!("Tool count overflow: {error}")))?, measurement.attempted_cost_microdollars, measurement.accounting_status.as_str(), measurement.verified_success).execute(&self.pool).await?;
        if changed.rows_affected() != 1 {
            return Err(crate::experiments::conflict(
                "Measurement is immutable or execution is unavailable",
            ));
        }
        Ok(())
    }
}
