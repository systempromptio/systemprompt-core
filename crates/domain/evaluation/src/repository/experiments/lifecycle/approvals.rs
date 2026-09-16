//! Approval and development-only suggestion persistence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    ApprovalAuthorization, ApprovalDecision, EvalApprovalId, EvalExecutionId,
    EvaluationLifecycleRepository, ExecutionApproval, ExecutionLease, Result, UserId, invalid,
};
use crate::models::ApprovalStatus;

#[derive(Debug, Clone, Copy)]
pub struct ApprovalVerdict<'a> {
    pub actor: &'a UserId,
    pub approval: &'a EvalApprovalId,
    pub decision: ApprovalDecision,
    pub observed_precondition: &'a str,
}

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
            status: ApprovalStatus::Pending,
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
            match ApprovalStatus::parse(&row.status)? {
                ApprovalStatus::Approved => {
                    sqlx::query!("UPDATE eval_execution_approvals SET status='consumed' WHERE id=$1 AND status='approved'", id.as_str()).execute(&mut *tx).await?;
                    tx.commit().await?;
                    return Ok(ApprovalAuthorization::Authorized(id));
                },
                ApprovalStatus::Pending => { tx.commit().await?; return Ok(ApprovalAuthorization::Pending(id)); },
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
        verdict: &ApprovalVerdict<'_>,
    ) -> Result<()> {
        let ApprovalVerdict {
            actor,
            approval,
            decision,
            observed_precondition,
        } = *verdict;
        if observed_precondition.len() != 64
            || !observed_precondition
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(invalid("Approval requires a 64-character digest"));
        }
        let mut tx = self.pool.begin().await?;
        let status = match decision {
            ApprovalDecision::Approve => ApprovalStatus::Approved,
            ApprovalDecision::Deny => ApprovalStatus::Denied,
        }
        .as_str();
        let previous=sqlx::query!("SELECT status,decided_by,precondition_digest FROM eval_execution_approvals WHERE id=$1 AND owner_id=$2 FOR UPDATE",approval.as_str(),owner.as_str()).fetch_optional(&mut *tx).await?.ok_or_else(||crate::experiments::conflict("Approval unavailable in this scope"))?;
        if previous.status == status
            && previous.decided_by.as_deref() == Some(actor.as_str())
            && previous.precondition_digest == observed_precondition
        {
            tx.commit().await?;
            return Ok(());
        }
        let row = sqlx::query!("UPDATE eval_execution_approvals SET status=CASE WHEN expires_at<=NOW() THEN 'expired' ELSE $4 END,decided_by=$2,decided_at=NOW() WHERE id=$1 AND owner_id=$3 AND status='pending' AND precondition_digest=$5 RETURNING execution_id,status",
            approval.as_str(), actor.as_str(), owner.as_str(), status, observed_precondition).fetch_optional(&mut *tx).await?.ok_or_else(|| crate::experiments::conflict("Approval is stale, foreign, or precondition changed"))?;
        let final_status = ApprovalStatus::parse(&row.status)?;
        if final_status == ApprovalStatus::Approved {
            sqlx::query!("UPDATE eval_executions SET status='queued',lease_owner=NULL WHERE id=$1 AND status='awaiting_approval'", row.execution_id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        if final_status == ApprovalStatus::Expired {
            return Err(crate::experiments::conflict("Approval expired"));
        }
        Ok(())
    }
}
