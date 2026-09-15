//! Gateway request accounting bound to server-attested execution sessions.
//!
//! Session liveness and request audit state belong to other domains and are
//! read through `AiSessionProvider` and `AiRequestTrace`; the reservation
//! bookkeeping itself stays transactional under the owner lock.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{BudgetRepository, ExecutionLease, ReservationAdmission};
use crate::Result;
use crate::experiments::{conflict, missing};
use sqlx::PgPool;
use systemprompt_identifiers::{
    Actor, AiRequestId, EvalBudgetId, EvalReservationId, SessionId, UserId,
};

mod request;
pub use request::{
    AdmissionRequest, AdmissionRequestBuilder, EvaluationTrafficClass, RequestAdmission,
};
use systemprompt_traits::{DynAiRequestTrace, DynAiSessionProvider, TraceRequestStatus};

/// The foreign-domain reads the gateway repository performs.
#[derive(Clone)]
pub struct GatewaySeams {
    pub trace: DynAiRequestTrace,
    pub sessions: DynAiSessionProvider,
}

impl std::fmt::Debug for GatewaySeams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewaySeams").finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct GatewayEvaluationRepository {
    pool: PgPool,
    budgets: BudgetRepository,
    trace: DynAiRequestTrace,
    sessions: DynAiSessionProvider,
    admission: std::sync::Arc<dyn crate::capabilities::ExecutionAdmission>,
}

impl std::fmt::Debug for GatewayEvaluationRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayEvaluationRepository")
            .finish_non_exhaustive()
    }
}

impl GatewayEvaluationRepository {
    pub fn new(pool: PgPool, budgets: BudgetRepository, seams: GatewaySeams) -> Self {
        Self::with_admission(
            pool,
            budgets,
            seams,
            std::sync::Arc::new(crate::capabilities::VerifiedExecutionAdmission),
        )
    }

    pub fn with_admission(
        pool: PgPool,
        budgets: BudgetRepository,
        seams: GatewaySeams,
        admission: std::sync::Arc<dyn crate::capabilities::ExecutionAdmission>,
    ) -> Self {
        Self {
            pool,
            budgets,
            trace: seams.trace,
            sessions: seams.sessions,
            admission,
        }
    }

    async fn session_is_owned(&self, owner: &UserId, session: &SessionId) -> Result<bool> {
        let live = self.sessions.find_live_session(session).await?;
        Ok(live.is_some_and(|session| session.user_id.as_ref() == Some(owner)))
    }

    pub async fn execution_actor(
        &self,
        owner: &UserId,
        session: &SessionId,
    ) -> Result<Option<Actor>> {
        let execution = sqlx::query_scalar!(
            "SELECT execution_id FROM eval_session_bindings WHERE session_id=$1 AND owner_id=$2",
            session.as_str(),
            owner.as_str()
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(execution.map(|id| Actor::job(owner.clone(), format!("evaluation:{id}"))))
    }

    pub async fn is_evaluation_session(&self, session: &SessionId) -> Result<bool> {
        Ok(sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM eval_session_bindings WHERE session_id=$1) AS "exists!""#,
            session.as_str()
        )
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn set_traffic_class(
        &self,
        owner: &UserId,
        lease: &ExecutionLease,
        traffic_class: EvaluationTrafficClass,
    ) -> Result<()> {
        let changed = sqlx::query!("UPDATE eval_session_bindings b SET traffic_class=$5 FROM eval_executions x,eval_experiments e WHERE b.execution_id=x.id AND x.experiment_id=e.id AND b.owner_id=$1 AND b.execution_id=$2 AND x.lease_owner=$3 AND b.fencing_token=$4 AND x.fencing_token=b.fencing_token AND x.status='running' AND e.status='running' AND x.lease_expires_at>NOW() AND x.deadline_at>NOW()",
            owner.as_str(), lease.execution_id.as_str(), lease.worker_id.as_str(), lease.fencing_token, traffic_class.as_str()).execute(&self.pool).await?;
        if changed.rows_affected() != 1 {
            return Err(conflict(
                "Traffic classification requires the current live fence",
            ));
        }
        Ok(())
    }

    pub async fn bind_session(
        &self,
        owner: &UserId,
        lease: &ExecutionLease,
        session: &SessionId,
    ) -> Result<()> {
        if !self.session_is_owned(owner, session).await? {
            return Err(conflict("Session or execution lease is unavailable"));
        }
        let mut tx = self.pool.begin().await?;
        super::lock_owner(&mut tx, owner).await?;
        let eligible = sqlx::query_scalar!(
            "SELECT x.id FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE e.owner_id=$1 AND e.status='running' AND x.id=$2 AND x.lease_owner=$3 AND x.fencing_token=$4 AND x.status='running' AND x.lease_expires_at>NOW() AND x.deadline_at>NOW()",
            owner.as_str(), lease.execution_id.as_str(), lease.worker_id.as_str(), lease.fencing_token
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
            "SELECT b.owner_id,b.execution_id,b.fencing_token,b.traffic_class,x.fencing_token AS current_fence,x.status,(x.lease_expires_at>NOW() AND x.deadline_at>NOW()) AS live,e.status AS experiment_status,e.budget_id,e.spec->'variants'->x.variant_index->>'model' AS model,e.spec->'variants'->x.variant_index->>'provider' AS provider,EXISTS(SELECT 1 FROM eval_workers w WHERE w.id=x.lease_owner AND w.owner_id=e.owner_id AND w.enabled AND w.expires_at>NOW()) AS worker_live FROM eval_session_bindings b JOIN eval_executions x ON x.id=b.execution_id JOIN eval_experiments e ON e.id=x.experiment_id WHERE b.session_id=$1",
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
            || bound.provider.as_deref() != Some(input.provider.as_str())
            || bound.worker_live != Some(true)
        {
            return Err(conflict(
                "Execution session is stale, cancelled, foreign or requests another model",
            ));
        }
        super::admission::execution(
            &mut tx,
            input.owner,
            &systemprompt_identifiers::EvalExecutionId::new(bound.execution_id.clone()),
            self.admission.as_ref(),
        )
        .await?;
        let audited = self
            .trace
            .find_usage(input.owner, input.request)
            .await?
            .is_some_and(|usage| {
                usage.status == TraceRequestStatus::Pending
                    && usage.session_id.as_ref() == Some(input.session)
            });
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
            "INSERT INTO eval_request_reservations(request_id,execution_id,reservation_id,traffic_class) VALUES($1,$2,$3,$4)",
            input.request.as_str(), bound.execution_id, reservation.as_str(), bound.traffic_class
        ).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(RequestAdmission::Reserved(reservation))
    }

    pub async fn settle_recorded(&self, owner: &UserId, request: &AiRequestId) -> Result<bool> {
        let reservation = sqlx::query_scalar!(
            "SELECT m.reservation_id FROM eval_request_reservations m JOIN eval_executions x ON x.id=m.execution_id JOIN eval_experiments e ON e.id=x.experiment_id WHERE m.request_id=$1 AND e.owner_id=$2",
            request.as_str(), owner.as_str()
        ).fetch_optional(&self.pool).await?;
        let Some(reservation_id) = reservation else {
            return Ok(false);
        };
        let Some(record) = self.trace.find_usage(owner, request).await? else {
            return Ok(false);
        };
        if !record.is_settled() || record.tokens_used.unwrap_or(0) <= 0 {
            return Err(missing(
                "Provider usage is not yet complete; reservation remains held",
            ));
        }
        self.budgets
            .settle(
                owner,
                &EvalReservationId::new(reservation_id),
                request,
                record.cost_microdollars,
            )
            .await?;
        Ok(true)
    }
}
