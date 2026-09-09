//! Owner-scoped experiment creation, cancellation and durable worker claiming.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;
use crate::experiments::records::{ExecutionRecord, ExperimentDetail, ExperimentRecord};
use crate::experiments::resources::ResourceContent;
use sqlx::PgPool;
use sqlx::types::Json;
use systemprompt_identifiers::{EvalBudgetId, EvalExecutionId, EvalExperimentId, UserId};

use super::RevisionRepository;
use crate::experiments::{ExperimentSpec, content_digest, invalid};

#[derive(Clone, Debug)]
pub struct ExperimentRepository {
    pub(super) pool: PgPool,
    revisions: RevisionRepository,
}

impl ExperimentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self {
            revisions: RevisionRepository::new(pool.clone()),
            pool,
        }
    }

    pub async fn create(
        &self,
        owner: &UserId,
        key: &str,
        spec: &ExperimentSpec,
    ) -> Result<EvalExperimentId> {
        spec.validate()?;
        if key.trim().is_empty() || key.len() > 255 {
            return Err(invalid("An idempotency key is required"));
        }
        if !matches!(
            self.revisions.get(owner, &spec.rubric).await?,
            ResourceContent::Rubric(_)
        ) {
            return Err(invalid("Rubric reference must identify a rubric revision"));
        }
        for case in &spec.cases {
            if !matches!(
                self.revisions.get(owner, case).await?,
                ResourceContent::Case(_)
            ) {
                return Err(invalid("Case reference must identify a case revision"));
            }
        }
        let digest = content_digest(spec)?;
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
            format!("eval:{}:{key}", owner.as_str())
        )
        .execute(&mut *tx)
        .await?;
        let existing = sqlx::query!(
            "SELECT id,spec_digest FROM eval_experiments WHERE owner_id=$1 AND idempotency_key=$2",
            owner.as_str(),
            key
        )
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(existing) = existing {
            if existing.spec_digest != digest {
                return Err(crate::experiments::conflict(
                    "Idempotency key conflicts with another experiment",
                ));
            }
            return Ok(EvalExperimentId::new(existing.id));
        }
        let budget = EvalBudgetId::generate();
        sqlx::query!(
            "INSERT INTO eval_budget_accounts(id,owner_id,cap) VALUES($1,$2,$3)",
            budget.as_str(),
            owner.as_str(),
            spec.budget_microdollars
        )
        .execute(&mut *tx)
        .await?;
        let id = EvalExperimentId::generate();
        sqlx::query!("INSERT INTO eval_experiments(id,owner_id,spec,spec_digest,budget_id,idempotency_key) VALUES($1,$2,$3,$4,$5,$6)", id.as_str(), owner.as_str(), Json(spec) as _, digest, budget.as_str(), key)
            .execute(&mut *tx).await?;
        Self::insert_executions(&mut tx, &id, spec).await?;
        tx.commit().await?;
        Ok(id)
    }

    async fn insert_executions(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: &EvalExperimentId,
        spec: &ExperimentSpec,
    ) -> Result<()> {
        for (variant, _) in spec.variants.iter().enumerate() {
            let variant = i32::try_from(variant)
                .map_err(|error| invalid(&format!("Too many variants: {error}")))?;
            for case in &spec.cases {
                for repetition in 0..spec.repetitions {
                    let execution_id = EvalExecutionId::generate();
                    let repetition = i32::try_from(repetition)
                        .map_err(|error| invalid(&format!("Repetition overflow: {error}")))?;
                    sqlx::query!("INSERT INTO eval_executions(id,experiment_id,variant_index,case_revision_id,repetition) VALUES($1,$2,$3,$4,$5)", execution_id.as_str(), id.as_str(), variant, case.as_str(), repetition)
            .execute(&mut **tx).await?;
                }
            }
        }
        Ok(())
    }

    pub async fn list(&self, owner: &UserId) -> Result<Vec<ExperimentRecord>> {
        Ok(sqlx::query_scalar!(
            r#"SELECT to_jsonb(e) || jsonb_build_object('accounting',to_jsonb(b)) AS "record!: Json<ExperimentRecord>" FROM eval_experiments e JOIN eval_budget_accounts b ON b.id=e.budget_id WHERE e.owner_id=$1 ORDER BY e.created_at DESC LIMIT 100"#,
            owner.as_str()
        ).fetch_all(&self.pool).await?.into_iter().map(|record| record.0).collect())
    }

    pub async fn get(&self, owner: &UserId, id: &EvalExperimentId) -> Result<ExperimentDetail> {
        let experiment = sqlx::query_scalar!(
            r#"SELECT to_jsonb(e) || jsonb_build_object('accounting',to_jsonb(b)) AS "record!: Json<ExperimentRecord>" FROM eval_experiments e JOIN eval_budget_accounts b ON b.id=e.budget_id WHERE e.id=$1 AND e.owner_id=$2"#,
            id.as_str(), owner.as_str()
        ).fetch_optional(&self.pool).await?.ok_or_else(|| crate::experiments::missing("Experiment unavailable in this scope"))?.0;
        let executions = sqlx::query_scalar!(
            r#"SELECT to_jsonb(x) AS "record!: Json<ExecutionRecord>" FROM eval_executions x WHERE experiment_id=$1 ORDER BY variant_index,case_revision_id,repetition"#,
            id.as_str()
        ).fetch_all(&self.pool).await?.into_iter().map(|record| record.0).collect();
        Ok(ExperimentDetail {
            experiment,
            executions,
        })
    }

    pub async fn cancel(&self, owner: &UserId, id: &EvalExperimentId) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let budget: Option<String> = sqlx::query_scalar!("UPDATE eval_experiments SET status='cancelled' WHERE id=$1 AND owner_id=$2 AND status IN ('queued','running','cancelled','blocked') RETURNING budget_id", id.as_str(), owner.as_str())
            .fetch_optional(&mut *tx).await?;
        let budget =
            budget.ok_or_else(|| crate::experiments::conflict("Experiment cannot be cancelled"))?;
        sqlx::query!(
            "UPDATE eval_budget_accounts SET frozen=TRUE WHERE id=$1",
            budget
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!("UPDATE eval_executions SET status='cancelled',finished_at=NOW() WHERE experiment_id=$1 AND status='queued'", id.as_str())
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn claim(&self, owner: &UserId, worker: &UserId) -> Result<Option<ExecutionRecord>> {
        if worker.as_str().trim().is_empty() || worker.as_str().len() > 255 {
            return Err(invalid("Worker identity required"));
        }
        let mut tx = self.pool.begin().await?;
        let expired = super::ExecutionCompletion {
            outcome: super::TerminalOutcome::Error,
            summary: "Worker lease expired; billing reservations remain held until reconciled"
                .to_owned(),
        };
        sqlx::query!(
            "UPDATE eval_executions x SET status='error',result=$2,finished_at=NOW() FROM eval_experiments e WHERE x.experiment_id=e.id AND e.owner_id=$1 AND x.status='running' AND x.lease_expires_at<NOW()",
            owner.as_str(), Json(&expired) as _
        ).execute(&mut *tx).await?;
        let row = sqlx::query!("SELECT x.id,x.experiment_id FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE e.owner_id=$1 AND e.status IN ('queued','running') AND x.status='queued' ORDER BY x.created_at FOR UPDATE OF x SKIP LOCKED LIMIT 1", owner.as_str())
            .fetch_optional(&mut *tx).await?;
        let Some(row) = row else {
            sqlx::query!(
                "UPDATE eval_experiments e SET status='completed' WHERE owner_id=$1 AND status='running' AND NOT EXISTS(SELECT 1 FROM eval_executions x WHERE x.experiment_id=e.id AND x.status IN ('queued','running','awaiting_approval'))",
                owner.as_str()
            ).execute(&mut *tx).await?;
            tx.commit().await?;
            return Ok(None);
        };
        let execution = sqlx::query_scalar!(r#"UPDATE eval_executions SET status='running',lease_owner=$2,lease_expires_at=NOW()+INTERVAL '60 seconds',fencing_token=fencing_token+1 WHERE id=$1 RETURNING to_jsonb(eval_executions) AS "record!: Json<ExecutionRecord>""#, row.id, worker.as_str())
            .fetch_one(&mut *tx).await?;
        sqlx::query!(
            "UPDATE eval_experiments SET status='running' WHERE id=$1 AND status='queued'",
            row.experiment_id
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Some(execution.0))
    }
}
