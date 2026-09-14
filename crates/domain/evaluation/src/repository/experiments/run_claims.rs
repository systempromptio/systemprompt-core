//! Fenced evaluator execution claiming.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{EvalWorkerId, ExecutionRecord, ExperimentRepository, Json, Result, UserId, invalid};

impl ExperimentRepository {
    pub async fn claim(
        &self,
        owner: &UserId,
        worker: &EvalWorkerId,
    ) -> Result<Option<ExecutionRecord>> {
        if worker.as_str().trim().is_empty() || worker.as_str().len() > 255 {
            return Err(invalid("Worker identity required"));
        }
        let mut tx = self.pool.begin().await?;
        super::super::lock_owner(&mut tx, owner).await?;
        let expired = super::super::ExecutionCompletion {
            outcome: super::super::TerminalOutcome::Error,
            summary: "Worker lease expired; billing reservations remain held until reconciled"
                .to_owned(),
        };
        sqlx::query!(
            "UPDATE eval_executions x SET status='error',result=$2,finished_at=NOW() FROM eval_experiments e WHERE x.experiment_id=e.id AND e.owner_id=$1 AND x.status='running' AND (x.lease_expires_at<NOW() OR x.deadline_at<=NOW())",
            owner.as_str(), Json(&expired) as _
        ).execute(&mut *tx).await?;
        let active = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE e.owner_id=$1 AND x.status='running' AND x.lease_expires_at>NOW() AND x.deadline_at>NOW()",
            owner.as_str()
        ).fetch_one(&mut *tx).await?.unwrap_or(0);
        if active >= 2 {
            tx.commit().await?;
            return Ok(None);
        }
        let row = sqlx::query!("SELECT x.id,x.experiment_id FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE e.owner_id=$1 AND e.status IN ('queued','running') AND x.status='queued' AND x.active_runtime_ms<1800000 ORDER BY x.created_at,x.variant_index,x.repetition FOR UPDATE OF x SKIP LOCKED LIMIT 1", owner.as_str())
            .fetch_optional(&mut *tx).await?;
        let Some(row) = row else {
            sqlx::query!(
                "UPDATE eval_experiments e SET status='completed' WHERE owner_id=$1 AND status='running' AND NOT EXISTS(SELECT 1 FROM eval_executions x WHERE x.experiment_id=e.id AND x.status IN ('queued','running','awaiting_approval'))",
                owner.as_str()
            ).execute(&mut *tx).await?;
            tx.commit().await?;
            return Ok(None);
        };
        super::super::admission::execution(
            &mut tx,
            owner,
            &systemprompt_identifiers::EvalExecutionId::new(row.id.clone()),
            self.admission.as_ref(),
        )
        .await?;
        let execution = sqlx::query_scalar!(r#"UPDATE eval_executions SET status='running',lease_owner=$2,lease_expires_at=NOW()+INTERVAL '60 seconds',deadline_at=NOW()+((1800000-active_runtime_ms)::TEXT || ' milliseconds')::INTERVAL,last_heartbeat_at=NOW(),fencing_token=fencing_token+1 WHERE id=$1 RETURNING to_jsonb(eval_executions) AS "record!: Json<ExecutionRecord>""#, row.id, worker.as_str())
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
