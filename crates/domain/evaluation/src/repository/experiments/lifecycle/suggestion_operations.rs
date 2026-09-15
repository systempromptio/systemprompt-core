//! Atomic suggestion operations retain exact retry identity and experiment
//! budget.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{EvaluationLifecycleRepository, SuggestionRequest};
use crate::Result;
use crate::experiments::{conflict, content_digest, invalid, missing};
use crate::repository::experiments::{BudgetRepository, ReservationAdmission};
use systemprompt_identifiers::{EvalSuggestionId, UserId};

impl EvaluationLifecycleRepository {
    pub async fn create_suggestion(
        &self,
        owner: &UserId,
        request: &SuggestionRequest,
    ) -> Result<EvalSuggestionId> {
        if request.hypothesis.trim().is_empty()
            || request.hypothesis.len() > 4000
            || request.operation_key.trim().is_empty()
            || request.operation_key.len() > 200
            || request.supporting_execution_ids.is_empty()
            || request.supporting_execution_ids.len() > 1000
            || serde_jcs::to_vec(request)?.len() > 262_144
        {
            return Err(invalid(
                "Suggestion requires bounded evidence, hypothesis and operation key",
            ));
        }
        let digest = content_digest(request)?;
        let mut tx = self.pool.begin().await?;
        super::super::lock_owner(&mut tx, owner).await?;
        if let Some(row) = sqlx::query!("SELECT id,operation_digest FROM eval_suggestions WHERE owner_id=$1 AND operation_key=$2",owner.as_str(),request.operation_key).fetch_optional(&mut *tx).await? {
            if row.operation_digest.as_deref()!=Some(digest.as_str()) { return Err(conflict("Suggestion operation key conflicts with retained input")); }
            tx.commit().await?;
            return Ok(EvalSuggestionId::new(row.id));
        }
        let experiment = sqlx::query!(
            "SELECT budget_id,spec FROM eval_experiments WHERE owner_id=$1 AND id=$2 FOR UPDATE",
            owner.as_str(),
            request.experiment_id.as_str()
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| missing("Experiment unavailable in this scope"))?;
        if experiment.budget_id != request.budget_id.as_str() {
            return Err(conflict("Suggestion must use the experiment budget"));
        }
        let spec = serde_json::from_value(experiment.spec)?;
        self.admission.admit(&spec)?;
        let executions: Vec<_> = request
            .supporting_execution_ids
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect();
        let valid=sqlx::query_scalar!("SELECT count(*) FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id JOIN eval_resource_revisions c ON c.id=x.case_revision_id AND c.owner_id=e.owner_id WHERE e.owner_id=$1 AND e.id=$2 AND x.id=ANY($3) AND c.content->'content'->>'partition'='development'",owner.as_str(),request.experiment_id.as_str(),&executions).fetch_one(&mut *tx).await?.unwrap_or(0);
        if usize::try_from(valid).ok() != Some(executions.len()) {
            return Err(invalid("Suggestions may use development evidence only"));
        }
        let reservation = match BudgetRepository::reserve_in(
            &mut tx,
            owner,
            &request.budget_id,
            &format!("suggestion:{}", request.operation_key),
            request.maximum_cost_microdollars,
        )
        .await?
        {
            ReservationAdmission::Admitted(id) => id,
            ReservationAdmission::AlreadyReserved(_) => {
                return Err(conflict(
                    "Suggestion operation has historical reservation without retained retry identity",
                ));
            },
        };
        let id = EvalSuggestionId::generate();
        sqlx::query!("INSERT INTO eval_suggestions(id,owner_id,experiment_id,supporting_execution_ids,proposed_changes,hypothesis,reservation_id,originating_evidence,operation_key,operation_digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",id.as_str(),owner.as_str(),request.experiment_id.as_str(),&executions,&request.proposed_changes,request.hypothesis,reservation.as_str(),&request.originating_evidence,request.operation_key,digest).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(id)
    }
    pub async fn suggestion(
        &self,
        owner: &UserId,
        id: &EvalSuggestionId,
    ) -> Result<crate::campaigns::suggestions::RetainedSuggestion> {
        let row=sqlx::query!("SELECT id,hypothesis,status,proposed_changes,originating_evidence FROM eval_suggestions WHERE owner_id=$1 AND id=$2",owner.as_str(),id.as_str()).fetch_optional(&self.pool).await?.ok_or_else(||missing("Suggestion unavailable in this scope"))?;
        Ok(crate::campaigns::suggestions::RetainedSuggestion {
            id: EvalSuggestionId::new(row.id),
            hypothesis: row.hypothesis,
            status: row.status,
            proposed_changes: row.proposed_changes,
            originating_evidence: row.originating_evidence,
        })
    }
    pub async fn approval(
        &self,
        owner: &UserId,
        id: &systemprompt_identifiers::EvalApprovalId,
    ) -> Result<super::ExecutionApproval> {
        let row=sqlx::query!("SELECT id,execution_id,operation,precondition_digest,status FROM eval_execution_approvals WHERE owner_id=$1 AND id=$2",owner.as_str(),id.as_str()).fetch_optional(&self.pool).await?.ok_or_else(||missing("Approval unavailable in this scope"))?;
        Ok(super::ExecutionApproval {
            id: systemprompt_identifiers::EvalApprovalId::new(row.id),
            execution_id: systemprompt_identifiers::EvalExecutionId::new(row.execution_id),
            operation: row.operation,
            precondition_digest: row.precondition_digest,
            status: crate::models::ApprovalStatus::parse(&row.status)?,
        })
    }
}
