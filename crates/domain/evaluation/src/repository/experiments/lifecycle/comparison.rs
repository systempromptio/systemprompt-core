//! Retained per-execution measurements aggregated into a paired comparison.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use systemprompt_identifiers::{EvalExecutionId, EvalExperimentId, EvalRevisionId, UserId};

use super::{EvaluationLifecycleRepository, Result};
use crate::experiments::records::ExecutionStatus;
use crate::models::AccountingStatus;

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RetainedMeasurement {
    pub hard_failures: Vec<String>,
    pub quality_milli: Option<u32>,
    pub latency_ms: Option<u64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub attempted_cost_microdollars: i64,
    pub accounting_status: AccountingStatus,
    pub verified_success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MeasurementRow {
    pub execution_id: EvalExecutionId,
    pub variant: usize,
    pub case_revision_id: EvalRevisionId,
    pub repetition: i32,
    pub status: ExecutionStatus,
    pub measurement: Option<RetainedMeasurement>,
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
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
    pub variants: Vec<MeasurementRow>,
}

impl EvaluationLifecycleRepository {
    pub async fn comparison(
        &self,
        owner: &UserId,
        experiment: &EvalExperimentId,
    ) -> Result<ComparisonReport> {
        let row = sqlx::query!(r#"SELECT count(*) AS "attempted!",count(*) FILTER(WHERE x.status='completed') AS "completed!",count(*) FILTER(WHERE cardinality(COALESCE(m.hard_failures,ARRAY[]::TEXT[]))>0) AS "hard_failures!",count(*) FILTER(WHERE m.quality_milli IS NULL) AS "unscored!",count(*) FILTER(WHERE m.verified_success) AS "successes!",COALESCE(sum(m.attempted_cost_microdollars),0)::BIGINT AS "cost!",count(*) FILTER(WHERE m.accounting_status='complete') AS "accounting_complete!",count(m.execution_id) AS "accounting_total!",jsonb_agg(jsonb_build_object('execution_id',x.id,'variant',x.variant_index,'case_revision_id',x.case_revision_id,'repetition',x.repetition,'status',x.status,'measurement',to_jsonb(m)) ORDER BY x.variant_index,x.case_revision_id,x.repetition) AS "variants!: Json<Vec<MeasurementRow>>" FROM eval_experiments e JOIN eval_executions x ON x.experiment_id=e.id LEFT JOIN eval_execution_measurements m ON m.execution_id=x.id WHERE e.owner_id=$1 AND e.id=$2 GROUP BY e.id"#,
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
            variants: row.variants.0,
        })
    }
}
