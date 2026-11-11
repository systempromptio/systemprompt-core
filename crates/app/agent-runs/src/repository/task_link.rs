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

use sqlx::{Postgres, Transaction};
use systemprompt_agent::repository::RunClaim;
use systemprompt_agent::repository::task::TaskRepository;
use systemprompt_identifiers::{AgentRunId, TaskId};
use systemprompt_models::a2a::TaskState;

use super::AgentRunRepository;
use crate::error::AgentRunsResult;
use crate::outcome::RunStatus;

pub async fn transition_task(
    tx: &mut Transaction<'_, Postgres>,
    run_id: &AgentRunId,
    claim: RunClaim<'_>,
    status: RunStatus,
) -> AgentRunsResult<()> {
    let state = TaskState::from(status);
    let transitioned = match TaskRepository::run_task_in_tx(tx, run_id, claim).await {
        Ok(task) => {
            TaskRepository::transition_task_in_tx(tx, &task, state, &chrono::Utc::now()).await
        },
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
