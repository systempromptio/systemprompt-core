//! Restart reconciliation, generated suggestions, and cleanup status.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    AiRequestId, EvalExecutionId, EvalSuggestionId, EvaluationLifecycleRepository, ExecutionLease,
    GeneratedSuggestion, Result, UserId, invalid,
};

#[derive(Debug, Clone, Copy)]
pub struct CleanupReport<'a> {
    pub container_id: Option<&'a str>,
    pub network_id: Option<&'a str>,
    pub succeeded: bool,
    pub error: Option<&'a str>,
}

impl EvaluationLifecycleRepository {
    pub async fn record_generated_suggestion(
        &self,
        owner: &UserId,
        execution: &EvalExecutionId,
        request: &AiRequestId,
        suggestion: &GeneratedSuggestion,
    ) -> Result<EvalSuggestionId> {
        if suggestion.hypothesis.trim().is_empty()
            || suggestion.hypothesis.len() > 4000
            || suggestion.supporting_failures.is_empty()
            || suggestion.originating_evidence.is_empty()
            || serde_jcs::to_vec(&suggestion.proposed_changes)?.len() > 262_144
        {
            return Err(invalid(
                "Generated suggestion requires bounded changes, a hypothesis, failures and evidence",
            ));
        }
        let row = sqlx::query!("SELECT e.id AS experiment_id,m.reservation_id FROM eval_request_reservations m JOIN eval_executions x ON x.id=m.execution_id JOIN eval_experiments e ON e.id=x.experiment_id JOIN eval_resource_revisions c ON c.id=x.case_revision_id AND c.owner_id=e.owner_id WHERE e.owner_id=$1 AND x.id=$2 AND m.request_id=$3 AND m.traffic_class='suggestion' AND c.content->'content'->>'partition'='development'",
            owner.as_str(), execution.as_str(), request.as_str()).fetch_optional(&self.pool).await?
            .ok_or_else(|| invalid("Generated suggestions require a metered development-only inference request"))?;
        let id = EvalSuggestionId::generate();
        sqlx::query!("INSERT INTO eval_suggestions(id,owner_id,experiment_id,supporting_execution_ids,proposed_changes,hypothesis,reservation_id,originating_evidence) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
            id.as_str(), owner.as_str(), row.experiment_id, &vec![execution.as_str().to_owned()], &suggestion.proposed_changes, &suggestion.hypothesis, row.reservation_id, serde_json::json!({"references":suggestion.originating_evidence,"failures":suggestion.supporting_failures,"request_id":request})).execute(&self.pool).await?;
        Ok(id)
    }

    pub async fn reconcile_restart(&self, owner: &UserId) -> Result<u64> {
        let changed = sqlx::query!("WITH expired AS (UPDATE eval_executions x SET status='error',finished_at=NOW(),lease_expires_at=NULL,result=jsonb_build_object('outcome','error','summary','Supervisor restart invalidated lease; uncertain writes require review') FROM eval_experiments e WHERE x.experiment_id=e.id AND e.owner_id=$1 AND x.status='running' AND x.lease_expires_at<=NOW() RETURNING x.id) INSERT INTO eval_execution_cleanup(execution_id,status,attempts) SELECT id,'retrying',1 FROM expired ON CONFLICT(execution_id) DO UPDATE SET status='retrying',attempts=eval_execution_cleanup.attempts+1,updated_at=NOW()",
            owner.as_str()).execute(&self.pool).await?;
        let retained = self.budgets.retain_orphaned(owner).await?;
        if retained > 0 {
            tracing::warn!(
                owner = owner.as_str(),
                retained,
                "restart retained the reserved bound of unsettled requests on finished executions"
            );
        }
        Ok(changed.rows_affected())
    }

    pub async fn execution_is_live(
        &self,
        owner: &UserId,
        execution: &EvalExecutionId,
    ) -> Result<bool> {
        Ok(sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE e.owner_id=$1 AND x.id=$2 AND e.status='running' AND x.status='running' AND x.lease_expires_at>NOW() AND x.deadline_at>NOW())",
            owner.as_str(), execution.as_str()).fetch_one(&self.pool).await?.unwrap_or(false))
    }

    pub async fn with_cleanup_fence<T>(
        &self,
        owner: &UserId,
        lease: &ExecutionLease,
        operation: impl FnOnce() -> T,
    ) -> Result<T> {
        let mut tx = self.pool.begin().await?;
        super::super::lock_owner(&mut tx, owner).await?;
        let eligible = sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE e.owner_id=$1 AND x.id=$2 AND x.lease_owner=$3 AND x.fencing_token=$4)", owner.as_str(), lease.execution_id.as_str(), lease.worker_id.as_str(), lease.fencing_token).fetch_one(&mut *tx).await?.unwrap_or(false);
        if !eligible {
            return Err(crate::experiments::conflict(
                "Cleanup requires the current owned fence",
            ));
        }
        let outcome = operation();
        tx.commit().await?;
        Ok(outcome)
    }

    pub async fn record_cleanup(
        &self,
        owner: &UserId,
        lease: &ExecutionLease,
        report: &CleanupReport<'_>,
    ) -> Result<()> {
        let CleanupReport {
            container_id,
            network_id,
            succeeded,
            error,
        } = *report;
        let mut tx = self.pool.begin().await?;
        super::super::lock_owner(&mut tx, owner).await?;
        let eligible = sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE e.owner_id=$1 AND x.id=$2 AND x.lease_owner=$3 AND x.fencing_token=$4)",
            owner.as_str(), lease.execution_id.as_str(), lease.worker_id.as_str(), lease.fencing_token).fetch_one(&mut *tx).await?.unwrap_or(false);
        if !eligible {
            return Err(crate::experiments::conflict(
                "Cleanup status requires the current owned fence",
            ));
        }
        let status = if succeeded { "verified" } else { "failed" };
        sqlx::query!("INSERT INTO eval_execution_cleanup(execution_id,container_id,network_id,status,attempts,last_error) VALUES($1,$2,$3,$4,1,$5) ON CONFLICT(execution_id) DO UPDATE SET container_id=EXCLUDED.container_id,network_id=EXCLUDED.network_id,status=EXCLUDED.status,attempts=eval_execution_cleanup.attempts+1,last_error=EXCLUDED.last_error,updated_at=NOW()",
            lease.execution_id.as_str(), container_id, network_id, status, error).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}
