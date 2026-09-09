//! Gateway request accounting bound to server-attested execution sessions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{BudgetRepository, ExecutionLease, ReservationAdmission};
use crate::Result;
use crate::experiments::{conflict, missing};
use sqlx::PgPool;
use systemprompt_identifiers::{
    AiRequestId, EvalBudgetId, EvalReservationId, ModelId, SessionId, UserId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestAdmission {
    Ordinary,
    Reserved(EvalReservationId),
}

#[derive(Debug, Clone)]
pub struct GatewayEvaluationRepository {
    pool: PgPool,
    budgets: BudgetRepository,
}

impl GatewayEvaluationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self {
            budgets: BudgetRepository::new(pool.clone()),
            pool,
        }
    }

    pub async fn is_evaluation_session(&self, session: &SessionId) -> Result<bool> {
        Ok(sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM eval_session_bindings WHERE session_id=$1)",
            session.as_str()
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(false))
    }

    pub async fn bind_session(
        &self,
        owner: &UserId,
        lease: &ExecutionLease,
        session: &SessionId,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        super::lock_owner(&mut tx, owner).await?;
        let eligible = sqlx::query_scalar!(
            "SELECT x.id FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id JOIN user_sessions s ON s.user_id=e.owner_id WHERE s.session_id=$1 AND e.owner_id=$2 AND e.status='running' AND x.id=$3 AND x.lease_owner=$4 AND x.fencing_token=$5 AND x.status='running' AND x.lease_expires_at>NOW() AND x.deadline_at>NOW()",
            session.as_str(), owner.as_str(), lease.execution_id.as_str(), lease.worker_id.as_str(), lease.fencing_token
        ).fetch_optional(&mut *tx).await?;
        if eligible.is_none() {
            return Err(conflict("Session or execution lease is unavailable"));
        }
        let inserted = sqlx::query!(
            "INSERT INTO eval_session_bindings(session_id,execution_id,owner_id,fencing_token) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING",
            session.as_str(), lease.execution_id.as_str(), owner.as_str(), lease.fencing_token
        ).execute(&mut *tx).await?;
        if inserted.rows_affected() != 1 {
            return Err(conflict("Session already bound to an execution"));
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn admit(&self, input: &AdmissionRequest<'_>) -> Result<RequestAdmission> {
        let mut tx = self.pool.begin().await?;
        super::lock_owner(&mut tx, input.owner).await?;
        let bound = sqlx::query!(
            "SELECT b.owner_id,b.execution_id,b.fencing_token,x.fencing_token AS current_fence,x.status,(x.lease_expires_at>NOW() AND x.deadline_at>NOW()) AS live,e.status AS experiment_status,e.budget_id,e.spec->'variants'->x.variant_index->>'model' AS model FROM eval_session_bindings b JOIN eval_executions x ON x.id=b.execution_id JOIN eval_experiments e ON e.id=x.experiment_id WHERE b.session_id=$1",
            input.session.as_str()
        ).fetch_optional(&mut *tx).await?;
        let Some(bound) = bound else {
            return Ok(RequestAdmission::Ordinary);
        };
        if bound.owner_id != input.owner.as_str()
            || bound.fencing_token != bound.current_fence
            || bound.status != "running"
            || bound.live != Some(true)
            || bound.experiment_status != "running"
            || bound.model.as_deref() != Some(input.model.as_str())
        {
            return Err(conflict(
                "Execution session is stale, cancelled, foreign or requests another model",
            ));
        }
        let audited = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM ai_requests WHERE id=$1 AND user_id=$2 AND session_id=$3 AND status='pending')",
            input.request.as_str(), input.owner.as_str(), input.session.as_str()
        ).fetch_one(&mut *tx).await?.unwrap_or(false);
        if !audited {
            return Err(conflict(
                "Evaluation requires a pending audit record owned by its session",
            ));
        }
        let admission = BudgetRepository::reserve_in(
            &mut tx,
            input.owner,
            &EvalBudgetId::new(bound.budget_id),
            input.request.as_str(),
            input.bound_microdollars,
        )
        .await?;
        let ReservationAdmission::Admitted(reservation) = admission else {
            return Err(conflict(
                "Request was already admitted; reconcile rather than dispatch twice",
            ));
        };
        sqlx::query!(
            "INSERT INTO eval_request_reservations(request_id,execution_id,reservation_id) VALUES($1,$2,$3)",
            input.request.as_str(), bound.execution_id, reservation.as_str()
        ).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(RequestAdmission::Reserved(reservation))
    }

    pub async fn settle_recorded(&self, owner: &UserId, request: &AiRequestId) -> Result<bool> {
        let reservation = sqlx::query!(
            "SELECT m.reservation_id,r.cost_microdollars,r.status,r.completed_at,r.tokens_used FROM eval_request_reservations m JOIN eval_executions x ON x.id=m.execution_id JOIN eval_experiments e ON e.id=x.experiment_id JOIN ai_requests r ON r.id=m.request_id WHERE m.request_id=$1 AND e.owner_id=$2 AND r.user_id=$2",
            request.as_str(), owner.as_str()
        ).fetch_optional(&self.pool).await?;
        let Some(record) = reservation else {
            return Ok(false);
        };
        if record.status != "completed"
            || record.completed_at.is_none()
            || record.tokens_used.unwrap_or(0) <= 0
        {
            return Err(missing(
                "Provider usage is not yet complete; reservation remains held",
            ));
        }
        self.budgets
            .settle(
                owner,
                &EvalReservationId::new(record.reservation_id),
                request,
                record.cost_microdollars,
            )
            .await?;
        Ok(true)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AdmissionRequest<'a> {
    pub owner: &'a UserId,
    pub session: &'a SessionId,
    pub request: &'a AiRequestId,
    pub model: &'a ModelId,
    pub bound_microdollars: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct AdmissionRequestBuilder<'a> {
    owner: &'a UserId,
    session: &'a SessionId,
    request: Option<&'a AiRequestId>,
    model: Option<&'a ModelId>,
    bound_microdollars: Option<i64>,
}

impl<'a> AdmissionRequest<'a> {
    pub const fn builder(owner: &'a UserId, session: &'a SessionId) -> AdmissionRequestBuilder<'a> {
        AdmissionRequestBuilder {
            owner,
            session,
            request: None,
            model: None,
            bound_microdollars: None,
        }
    }
}

impl<'a> AdmissionRequestBuilder<'a> {
    pub const fn request(mut self, request: &'a AiRequestId) -> Self {
        self.request = Some(request);
        self
    }
    pub const fn model(mut self, model: &'a ModelId) -> Self {
        self.model = Some(model);
        self
    }
    pub const fn bound_microdollars(mut self, amount: i64) -> Self {
        self.bound_microdollars = Some(amount);
        self
    }
    pub fn build(self) -> Result<AdmissionRequest<'a>> {
        let bound_microdollars = self
            .bound_microdollars
            .filter(|amount| *amount > 0)
            .ok_or_else(|| crate::experiments::invalid("Positive reservation bound required"))?;
        Ok(AdmissionRequest {
            owner: self.owner,
            session: self.session,
            request: self
                .request
                .ok_or_else(|| crate::experiments::invalid("Request ID required"))?,
            model: self
                .model
                .ok_or_else(|| crate::experiments::invalid("Model required"))?,
            bound_microdollars,
        })
    }
}
