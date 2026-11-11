//! Owner-scoped task reads and the notification-driven state write.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{TaskRepository, apply_notification_status, find_task};
use crate::models::a2a::{Message, Task};
use crate::repository::context::message::get_messages_by_task;
use crate::repository::{OwnedTask, RunClaim, RuntimeTask};
use systemprompt_identifiers::{AgentRunId, ContextId, TaskId, UserId};
use systemprompt_traits::RepositoryError;

impl TaskRepository {
    pub async fn owned_task(
        &self,
        task_id: &TaskId,
        user_id: &UserId,
    ) -> Result<OwnedTask, RepositoryError> {
        let row = sqlx::query!(
            r#"SELECT t.task_id as "task_id!: TaskId", t.context_id as "context_id!: ContextId"
            FROM agent_tasks t
            JOIN user_contexts c ON t.context_id = c.context_id
            WHERE t.task_id = $1 AND c.user_id = $2"#,
            task_id.as_str(),
            user_id.as_str()
        )
        .fetch_optional(self.pool.as_ref())
        .await?
        .ok_or_else(|| RepositoryError::not_found("task", task_id.as_str()))?;

        Ok(OwnedTask::new(row.task_id, row.context_id, user_id.clone()))
    }

    pub async fn run_task_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
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
            r#"SELECT t.task_id as "task_id!: TaskId", t.context_id as "context_id!: ContextId"
            FROM agent_runs r
            JOIN agent_tasks t ON t.task_id = r.task_id
            JOIN user_contexts c ON c.context_id = t.context_id AND c.user_id = r.user_id
            WHERE r.id = $1
              AND (($2::text IS NULL AND r.claimed_by IS NULL)
                OR (r.claimed_by = $2 AND r.attempt = $3 AND r.claimed_at = $4))"#,
            run_id.as_str(),
            claimed_by,
            attempt,
            claimed_at
        )
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| RepositoryError::not_found("agent run task", run_id.as_str()))?;

        Ok(RuntimeTask::new(row.task_id, row.context_id))
    }

    pub async fn find_owned_task(&self, owned: &OwnedTask) -> Result<Task, RepositoryError> {
        find_task(&self.constructor, owned.task_id())
            .await?
            .ok_or_else(|| RepositoryError::not_found("task", owned.task_id().as_str()))
    }

    pub async fn owned_task_messages(
        &self,
        owned: &OwnedTask,
    ) -> Result<Vec<Message>, RepositoryError> {
        get_messages_by_task(&self.pool, owned.task_id()).await
    }

    pub async fn apply_notification_status(
        &self,
        owned: &OwnedTask,
        state: &str,
        timestamp: &chrono::DateTime<chrono::Utc>,
    ) -> Result<(), RepositoryError> {
        apply_notification_status(&self.write_pool, owned.task_id(), state, timestamp).await
    }
}
