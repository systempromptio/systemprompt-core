//! Approval and development-only suggestion persistence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    ApprovalAuthorization, ApprovalDecision, EvalApprovalId, EvalExecutionId, EvalSuggestionId,
    EvaluationLifecycleRepository, ExecutionApproval, ExecutionLease, ReservationAdmission, Result,
    SuggestionRequest, UserId, invalid,
};

impl EvaluationLifecycleRepository {
    pub async fn request_approval(
        &self,
        owner: &UserId,
        lease: &ExecutionLease,
        operation: serde_json::Value,
        precondition_digest: &str,
    ) -> Result<ExecutionApproval> {
        if precondition_digest.len() != 64 || serde_jcs::to_vec(&operation)?.len() > 65_536 {
            return Err(invalid("Invalid approval operation or precondition"));
        }
        let operation_digest = crate::experiments::content_digest(&operation)?;
        let id = EvalApprovalId::generate();
        let mut tx = self.pool.begin().await?;
        let changed = sqlx::query!("UPDATE eval_executions x SET status='awaiting_approval',lease_expires_at=NULL FROM eval_experiments e WHERE x.experiment_id=e.id AND e.owner_id=$1 AND x.id=$2 AND x.lease_owner=$3 AND x.fencing_token=$4 AND x.status='running' AND x.lease_expires_at>NOW()",
            owner.as_str(), lease.execution_id.as_str(), lease.worker_id.as_str(), lease.fencing_token).execute(&mut *tx).await?;
        if changed.rows_affected() != 1 {
            return Err(crate::experiments::conflict(
                "Approval requires a live fenced execution",
            ));
        }
        sqlx::query!("INSERT INTO eval_execution_approvals(id,execution_id,owner_id,fencing_token,operation,operation_digest,precondition_digest) VALUES($1,$2,$3,$4,$5,$6,$7)",
            id.as_str(), lease.execution_id.as_str(), owner.as_str(), lease.fencing_token, &operation, operation_digest, precondition_digest).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(ExecutionApproval {
            id,
            execution_id: lease.execution_id.clone(),
            operation,
            precondition_digest: precondition_digest.to_owned(),
            status: "pending".to_owned(),
        })
    }

    pub async fn authorize_operation(
        &self,
        owner: &UserId,
        execution: &EvalExecutionId,
        operation: &serde_json::Value,
        precondition_digest: &str,
    ) -> Result<ApprovalAuthorization> {
        if precondition_digest.len() != 64 || serde_jcs::to_vec(operation)?.len() > 65_536 {
            return Err(invalid("Invalid approval operation or precondition"));
        }
        let operation_digest = crate::experiments::content_digest(operation)?;
        let mut tx = self.pool.begin().await?;
        super::super::lock_owner(&mut tx, owner).await?;
        if let Some(row) = sqlx::query!("SELECT id,status,operation FROM eval_execution_approvals WHERE owner_id=$1 AND execution_id=$2 AND operation_digest=$3 AND precondition_digest=$4 ORDER BY requested_at DESC LIMIT 1 FOR UPDATE",
            owner.as_str(), execution.as_str(), &operation_digest, precondition_digest).fetch_optional(&mut *tx).await? {
            let id = EvalApprovalId::new(row.id); let stored = row.operation;
            if &stored != operation { return Err(crate::experiments::conflict("Approval operation changed")); }
            match row.status.as_str() {
                "approved" | "consumed" => { tx.commit().await?; return Ok(ApprovalAuthorization::Authorized(id)); },
                "pending" => { tx.commit().await?; return Ok(ApprovalAuthorization::Pending(id)); },
                _ => return Err(crate::experiments::conflict("Approval was denied, expired, or consumed")),
            }
        }
        let live = sqlx::query!("SELECT x.fencing_token FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE e.owner_id=$1 AND x.id=$2 AND e.status='running' AND x.status='running' AND x.lease_expires_at>NOW() AND x.deadline_at>NOW() FOR UPDATE OF x",
            owner.as_str(), execution.as_str()).fetch_optional(&mut *tx).await?.ok_or_else(|| crate::experiments::conflict("Privileged operation requires a live execution"))?;
        let id = EvalApprovalId::generate();
        sqlx::query!("UPDATE eval_executions SET status='awaiting_approval',lease_expires_at=NULL WHERE id=$1", execution.as_str()).execute(&mut *tx).await?;
        sqlx::query!("INSERT INTO eval_execution_approvals(id,execution_id,owner_id,fencing_token,operation,operation_digest,precondition_digest) VALUES($1,$2,$3,$4,$5,$6,$7)",
            id.as_str(), execution.as_str(), owner.as_str(), live.fencing_token, operation, operation_digest, precondition_digest).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(ApprovalAuthorization::Pending(id))
    }

    pub async fn decide_approval(
        &self,
        owner: &UserId,
        actor: &UserId,
        approval: &EvalApprovalId,
        decision: ApprovalDecision,
        observed_precondition: &str,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let status = match decision {
            ApprovalDecision::Approve => "approved",
            ApprovalDecision::Deny => "denied",
        };
        let row = sqlx::query!("UPDATE eval_execution_approvals SET status=CASE WHEN expires_at<=NOW() THEN 'expired' ELSE $4 END,decided_by=$2,decided_at=NOW() WHERE id=$1 AND owner_id=$3 AND status='pending' AND precondition_digest=$5 RETURNING execution_id,status",
            approval.as_str(), actor.as_str(), owner.as_str(), status, observed_precondition).fetch_optional(&mut *tx).await?.ok_or_else(|| crate::experiments::conflict("Approval is stale, foreign, or precondition changed"))?;
        let final_status = row.status;
        if final_status == "approved" {
            sqlx::query!("UPDATE eval_executions SET status='queued',lease_owner=NULL WHERE id=$1 AND status='awaiting_approval'", row.execution_id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        if final_status == "expired" {
            return Err(crate::experiments::conflict("Approval expired"));
        }
        Ok(())
    }

    pub async fn create_suggestion(
        &self,
        owner: &UserId,
        request: &SuggestionRequest,
    ) -> Result<EvalSuggestionId> {
        if request.hypothesis.trim().is_empty()
            || request.hypothesis.len() > 4000
            || request.supporting_execution_ids.is_empty()
        {
            return Err(invalid(
                "Suggestion requires development failures and a hypothesis",
            ));
        }
        let valid = sqlx::query_scalar!("SELECT count(*) FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id JOIN eval_resource_revisions c ON c.id=x.case_revision_id WHERE e.owner_id=$1 AND e.id=$2 AND x.id=ANY($3) AND c.content->'content'->>'partition'='development'",
            owner.as_str(), request.experiment_id.as_str(), &request.supporting_execution_ids.iter().map(|value| value.as_str().to_owned()).collect::<Vec<_>>()).fetch_one(&self.pool).await?.unwrap_or(0);
        if usize::try_from(valid).ok() != Some(request.supporting_execution_ids.len()) {
            return Err(invalid("Suggestions may use development evidence only"));
        }
        let reservation = match self
            .budgets
            .reserve(
                owner,
                &request.budget_id,
                &format!("suggestion:{}", request.operation_key),
                request.maximum_cost_microdollars,
            )
            .await?
        {
            ReservationAdmission::Admitted(id) | ReservationAdmission::AlreadyReserved(id) => id,
        };
        let id = EvalSuggestionId::generate();
        sqlx::query!("INSERT INTO eval_suggestions(id,owner_id,experiment_id,supporting_execution_ids,proposed_changes,hypothesis,reservation_id,originating_evidence) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(owner_id,id) DO NOTHING",
            id.as_str(), owner.as_str(), request.experiment_id.as_str(), &request.supporting_execution_ids.iter().map(|value| value.as_str().to_owned()).collect::<Vec<_>>(), &request.proposed_changes, &request.hypothesis, reservation.as_str(), &request.originating_evidence).execute(&self.pool).await?;
        Ok(id)
    }
}
