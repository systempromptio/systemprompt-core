//! Fenced worker leases reject stale completion and preserve uncertain
//! accounting.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::ExperimentRepository;
use crate::Result;
use crate::experiments::invalid;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{EvalExecutionId, EvalWorkerId, UserId};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionLease {
    pub execution_id: EvalExecutionId,
    pub worker_id: EvalWorkerId,
    pub fencing_token: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalOutcome {
    Completed,
    Error,
    Blocked,
    Cancelled,
    BudgetExhausted,
}

impl TerminalOutcome {
    const fn name(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Error => "error",
            Self::Blocked => "blocked",
            Self::Cancelled => "cancelled",
            Self::BudgetExhausted => "budget_exhausted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionCompletion {
    pub outcome: TerminalOutcome,
    pub summary: String,
}

impl ExperimentRepository {
    pub async fn heartbeat(&self, owner: &UserId, lease: &ExecutionLease) -> Result<()> {
        let changed = sqlx::query!(
            "UPDATE eval_executions x SET lease_expires_at=LEAST(x.deadline_at,NOW()+INTERVAL '60 seconds') FROM eval_experiments e WHERE x.experiment_id=e.id AND e.owner_id=$1 AND e.status='running' AND x.id=$2 AND x.lease_owner=$3 AND x.fencing_token=$4 AND x.lease_expires_at>NOW() AND x.deadline_at>NOW() AND x.status='running'",
            owner.as_str(), lease.execution_id.as_str(), lease.worker_id.as_str(), lease.fencing_token
        ).execute(&self.pool).await?;
        if changed.rows_affected() != 1 {
            return Err(crate::experiments::conflict(
                "Execution lease expired, was cancelled or belongs to another worker",
            ));
        }
        Ok(())
    }

    pub async fn complete(
        &self,
        owner: &UserId,
        lease: &ExecutionLease,
        completion: &ExecutionCompletion,
    ) -> Result<()> {
        if completion.summary.trim().is_empty() || completion.summary.len() > 16_000 {
            return Err(invalid("Completion requires a summary of 1–16000 bytes"));
        }
        let mut tx = self.pool.begin().await?;
        super::lock_owner(&mut tx, owner).await?;
        let existing = sqlx::query!(
            r#"SELECT x.result AS "result: sqlx::types::Json<ExecutionCompletion>" FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE e.owner_id=$1 AND x.id=$2 AND x.lease_owner=$3 AND x.fencing_token=$4"#,
            owner.as_str(), lease.execution_id.as_str(), lease.worker_id.as_str(), lease.fencing_token
        ).fetch_optional(&mut *tx).await?;
        if existing
            .and_then(|row| row.result)
            .is_some_and(|result| result.0 == *completion)
        {
            return Ok(());
        }
        let changed = sqlx::query!(
            "UPDATE eval_executions x SET status=$5,result=$6,finished_at=NOW(),lease_expires_at=NULL FROM eval_experiments e WHERE x.experiment_id=e.id AND e.owner_id=$1 AND e.status='running' AND x.id=$2 AND x.lease_owner=$3 AND x.fencing_token=$4 AND x.lease_expires_at>NOW() AND x.deadline_at>NOW() AND x.status='running' RETURNING x.experiment_id",
            owner.as_str(), lease.execution_id.as_str(), lease.worker_id.as_str(), lease.fencing_token,
            completion.outcome.name(), sqlx::types::Json(completion) as _
        ).fetch_optional(&mut *tx).await?.ok_or_else(|| crate::experiments::conflict("Completion rejected: stale or foreign lease"))?;
        sqlx::query!(
            "UPDATE eval_experiments e SET status='completed' WHERE id=$1 AND NOT EXISTS(SELECT 1 FROM eval_executions x WHERE x.experiment_id=e.id AND x.status IN ('queued','running','awaiting_approval'))",
            changed.experiment_id
        ).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct ExecutionLeaseBuilder {
    execution_id: EvalExecutionId,
    worker_id: EvalWorkerId,
    fencing_token: Option<i64>,
}

impl ExecutionLease {
    pub const fn builder(
        execution_id: EvalExecutionId,
        worker_id: EvalWorkerId,
    ) -> ExecutionLeaseBuilder {
        ExecutionLeaseBuilder {
            execution_id,
            worker_id,
            fencing_token: None,
        }
    }
}

impl ExecutionLeaseBuilder {
    pub const fn fencing_token(mut self, token: i64) -> Self {
        self.fencing_token = Some(token);
        self
    }
    pub fn build(self) -> Result<ExecutionLease> {
        Ok(ExecutionLease {
            execution_id: self.execution_id,
            worker_id: self.worker_id,
            fencing_token: self
                .fencing_token
                .filter(|token| *token > 0)
                .ok_or_else(|| invalid("Positive fencing token required"))?,
        })
    }
}
