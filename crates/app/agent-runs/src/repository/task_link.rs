//! Runs linked to A2A tasks: the task transition a run status change makes
//! inside the run's own transaction, cancelling a run that never started,
//! and finding the run behind a task.
//!
//! The run row is authoritative. A task that is already terminal or gone
//! (`Conflict` / `NotFound` from the agent domain) is logged and left alone,
//! so a task cancelled from A2A never blocks the run from recording how it
//! actually ended; any other failure rolls the run's write back with it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};
use systemprompt_agent::repository::RuntimeTask;
use systemprompt_agent::repository::task::TaskRepository;
use systemprompt_identifiers::{AgentRunId, TaskId, UserId};
use systemprompt_models::a2a::TaskState;
use systemprompt_traits::RepositoryError;

use super::AgentRunRepository;
use crate::error::AgentRunsResult;
use crate::outcome::RunStatus;

/// The caller's claim on an agent run when resolving its task: the lease it
/// holds, or `Unclaimed` for a run no worker holds (queued, released or
/// reclaimed in the same transaction).
#[derive(Debug, Clone, Copy)]
pub enum RunClaim<'a> {
    Lease {
        claimed_by: &'a str,
        attempt: i32,
        claimed_at: DateTime<Utc>,
    },
    Unclaimed,
}

pub async fn run_task_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    run_id: &AgentRunId,
    claim: RunClaim<'_>,
) -> Result<RuntimeTask, RepositoryError> {
    let (claimed_by, attempt, claimed_at) = match claim {
        RunClaim::Lease {
            claimed_by,
            attempt,
            claimed_at,
        } => (Some(claimed_by), Some(attempt), Some(claimed_at)),
        RunClaim::Unclaimed => (None, None, None),
    };
    let row = sqlx::query!(
        r#"SELECT task_id AS "task_id!: TaskId", user_id AS "user_id!: UserId"
        FROM agent_runs
        WHERE id = $1
          AND task_id IS NOT NULL
          AND (($2::text IS NULL AND claimed_by IS NULL)
            OR (claimed_by = $2 AND attempt = $3 AND claimed_at = $4))"#,
        run_id.as_str(),
        claimed_by,
        attempt,
        claimed_at
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| RepositoryError::not_found("agent run task", run_id.as_str()))?;
    TaskRepository::run_task_for_user_in_tx(tx, &row.task_id, &row.user_id).await
}

pub async fn transition_task(
    tx: &mut Transaction<'_, Postgres>,
    run_id: &AgentRunId,
    claim: RunClaim<'_>,
    status: RunStatus,
) -> AgentRunsResult<()> {
    let state = TaskState::from(status);
    let transitioned = match run_task_in_tx(tx, run_id, claim).await {
        Ok(task) => TaskRepository::transition_task_in_tx(tx, &task, state, &Utc::now()).await,
        Err(e) => Err(e),
    };
    match transitioned {
        Ok(()) => Ok(()),
        Err(e) if e.is_conflict() || e.is_not_found() => {
            tracing::warn!(run_id = %run_id, ?state, error = %e, "Run left its task unchanged");
            Ok(())
        },
        Err(e) => Err(e.into()),
    }
}

impl AgentRunRepository {
    pub async fn cancel_queued(&self, id: &AgentRunId) -> AgentRunsResult<bool> {
        let mut tx = self.write_pool().begin().await?;
        let cancelled = sqlx::query_scalar!(
            r#"
            UPDATE agent_runs
            SET status = 'cancelled', error_class = 'ok', error = 'cancelled before start',
                cancel_requested_at = COALESCE(cancel_requested_at, NOW()), finished_at = NOW()
            WHERE id = $1 AND status = 'queued'
            RETURNING task_id AS "task_id: TaskId"
            "#,
            id.as_str(),
        )
        .fetch_optional(&mut *tx)
        .await?;
        let Some(task_id) = cancelled else {
            return Ok(false);
        };
        if task_id.is_some() {
            transition_task(&mut tx, id, RunClaim::Unclaimed, RunStatus::Cancelled).await?;
        }
        tx.commit().await?;
        Ok(true)
    }

    pub async fn find_run_for_task(&self, task_id: &TaskId) -> AgentRunsResult<Option<AgentRunId>> {
        let id = sqlx::query_scalar!(
            r#"
            SELECT id AS "id: AgentRunId" FROM agent_runs
            WHERE task_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            task_id.as_str(),
        )
        .fetch_optional(self.write_pool())
        .await?;
        Ok(id)
    }
}
