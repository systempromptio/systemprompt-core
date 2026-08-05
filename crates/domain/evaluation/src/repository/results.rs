//! Repository for evaluation results.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use std::str::FromStr;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AiRequestId, EvalCaseId, EvalResultId, EvalRunId};

use crate::error::{EvaluationError, Result};
use crate::models::{EvalResult, NewResultParams, Verdict};

#[derive(Debug, Clone)]
pub struct EvalResultRepository {
    pool: Arc<PgPool>,
}

impl EvalResultRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        Ok(Self {
            pool: db.write_pool_arc()?,
        })
    }

    pub async fn insert(&self, params: &NewResultParams) -> Result<EvalResultId> {
        let id = EvalResultId::generate();
        sqlx::query!(
            r#"
            INSERT INTO eval_results (
                id, run_id, ai_request_id, case_id, provider, model,
                overall_score, dimension_scores, verdict, rationale, repair_hint,
                prompt_excerpt, response_excerpt, judge_cost_microdollars,
                repaired, replay_of_result_id, judge_ai_request_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
            "#,
            id.as_str(),
            params.run_id.as_str(),
            params.ai_request_id.as_ref().map(AiRequestId::as_str),
            params.case_id.as_ref().map(EvalCaseId::as_str),
            params.provider,
            params.model,
            params.overall_score,
            params.dimension_scores,
            params.verdict.as_str(),
            params.rationale.as_deref(),
            params.repair_hint.as_deref(),
            params.prompt_excerpt.as_deref(),
            params.response_excerpt.as_deref(),
            params.judge_cost_microdollars,
            params.repaired,
            params
                .replay_of_result_id
                .as_ref()
                .map(EvalResultId::as_str),
            params.judge_ai_request_id.as_ref().map(AiRequestId::as_str)
        )
        .execute(self.pool.as_ref())
        .await?;
        Ok(id)
    }

    pub async fn list_by_run(&self, run_id: &EvalRunId) -> Result<Vec<EvalResult>> {
        let rows = sqlx::query!(
            r#"
            SELECT id, run_id, ai_request_id, case_id, provider, model,
                   overall_score, dimension_scores, verdict, rationale, repair_hint,
                   prompt_excerpt, response_excerpt, latency_ms,
                   cost_microdollars, judge_cost_microdollars, created_at,
                   repaired, replay_of_result_id, judge_ai_request_id
            FROM eval_results
            WHERE run_id = $1
            ORDER BY created_at
            "#,
            run_id.as_str()
        )
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(EvalResult {
                    id: EvalResultId::new(row.id),
                    run_id: EvalRunId::new(row.run_id),
                    ai_request_id: row.ai_request_id.map(AiRequestId::new),
                    case_id: row.case_id.map(EvalCaseId::new),
                    provider: row.provider,
                    model: row.model,
                    overall_score: row.overall_score,
                    dimension_scores: row.dimension_scores,
                    verdict: Verdict::from_str(&row.verdict)
                        .map_err(EvaluationError::JudgeParse)?,
                    rationale: row.rationale,
                    repair_hint: row.repair_hint,
                    prompt_excerpt: row.prompt_excerpt,
                    response_excerpt: row.response_excerpt,
                    latency_ms: row.latency_ms,
                    cost_microdollars: row.cost_microdollars,
                    judge_cost_microdollars: row.judge_cost_microdollars,
                    created_at: row.created_at,
                    repaired: row.repaired,
                    replay_of_result_id: row.replay_of_result_id.map(EvalResultId::new),
                    judge_ai_request_id: row.judge_ai_request_id.map(AiRequestId::new),
                })
            })
            .collect()
    }

    pub async fn failures_for_replay(&self, run_id: &EvalRunId) -> Result<Vec<EvalResult>> {
        let results = self.list_by_run(run_id).await?;
        Ok(results
            .into_iter()
            .filter(|r| matches!(r.verdict, Verdict::Fail | Verdict::Partial) && !r.repaired)
            .collect())
    }

    pub async fn mark_repaired(&self, id: &EvalResultId) -> Result<()> {
        sqlx::query!(
            "UPDATE eval_results SET repaired = TRUE WHERE id = $1",
            id.as_str()
        )
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }
}
