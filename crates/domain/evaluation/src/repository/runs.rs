//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use std::str::FromStr;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{EvalRubricId, EvalRunId, UserId};

use crate::error::{EvaluationError, Result};
use crate::models::{EvalRun, EvalRunKind, EvalRunStatus, NewRunParams, TriggerSource};

#[derive(Debug, Clone)]
pub struct EvalRunRepository {
    pool: Arc<PgPool>,
}

impl EvalRunRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        Ok(Self {
            pool: db.write_pool_arc()?,
        })
    }

    pub async fn create(&self, params: &NewRunParams) -> Result<EvalRunId> {
        let id = EvalRunId::generate();
        sqlx::query!(
            r#"
            INSERT INTO eval_runs (
                id, kind, judge_provider, judge_model, sample_size,
                created_by, rubric_id, trigger_source
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            id.as_str(),
            params.kind.as_str(),
            params.judge_provider,
            params.judge_model,
            params.sample_size,
            params.created_by.as_str(),
            params.rubric_id.as_ref().map(EvalRubricId::as_str),
            params.trigger_source.as_str()
        )
        .execute(self.pool.as_ref())
        .await?;
        Ok(id)
    }

    pub async fn get(&self, id: &EvalRunId) -> Result<EvalRun> {
        let row = sqlx::query!(
            r#"
            SELECT id, kind, status, judge_provider, judge_model, sample_size,
                   scored_count, failed_count, cost_microdollars, created_by,
                   created_at, completed_at, error_message, rubric_id, trigger_source
            FROM eval_runs
            WHERE id = $1
            "#,
            id.as_str()
        )
        .fetch_optional(self.pool.as_ref())
        .await?
        .ok_or_else(|| EvaluationError::RunNotFound(id.as_str().to_owned()))?;

        Ok(EvalRun {
            id: EvalRunId::new(row.id),
            kind: EvalRunKind::from_str(&row.kind).map_err(EvaluationError::JudgeParse)?,
            status: match row.status.as_str() {
                "completed" => EvalRunStatus::Completed,
                "failed" => EvalRunStatus::Failed,
                _ => EvalRunStatus::Running,
            },
            judge_provider: row.judge_provider,
            judge_model: row.judge_model,
            sample_size: row.sample_size,
            scored_count: row.scored_count,
            failed_count: row.failed_count,
            cost_microdollars: row.cost_microdollars,
            created_by: UserId::new(row.created_by),
            created_at: row.created_at,
            completed_at: row.completed_at,
            error_message: row.error_message,
            rubric_id: row.rubric_id.map(EvalRubricId::new),
            trigger_source: match row.trigger_source.as_str() {
                "scheduled" => TriggerSource::Scheduled,
                "cli" => TriggerSource::Cli,
                _ => TriggerSource::Manual,
            },
        })
    }

    pub async fn list_recent(&self, limit: i64) -> Result<Vec<EvalRun>> {
        let ids = sqlx::query_scalar!(
            "SELECT id FROM eval_runs ORDER BY created_at DESC LIMIT $1",
            limit
        )
        .fetch_all(self.pool.as_ref())
        .await?;

        let mut runs = Vec::with_capacity(ids.len());
        for id in ids {
            runs.push(self.get(&EvalRunId::new(id)).await?);
        }
        Ok(runs)
    }

    pub async fn record_scored(&self, id: &EvalRunId, failed: bool, cost: i64) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE eval_runs
            SET scored_count = scored_count + 1,
                failed_count = failed_count + CASE WHEN $2 THEN 1 ELSE 0 END,
                cost_microdollars = cost_microdollars + $3
            WHERE id = $1
            "#,
            id.as_str(),
            failed,
            cost
        )
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn complete(&self, id: &EvalRunId) -> Result<()> {
        self.finish(id, EvalRunStatus::Completed, None).await
    }

    pub async fn fail(&self, id: &EvalRunId, error_message: &str) -> Result<()> {
        self.finish(id, EvalRunStatus::Failed, Some(error_message))
            .await
    }

    async fn finish(
        &self,
        id: &EvalRunId,
        status: EvalRunStatus,
        error_message: Option<&str>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE eval_runs
            SET status = $2, error_message = $3, completed_at = CURRENT_TIMESTAMP
            WHERE id = $1
            "#,
            id.as_str(),
            status.as_str(),
            error_message
        )
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }
}
