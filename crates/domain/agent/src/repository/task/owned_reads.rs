//! Owner-scoped task reads and the notification-driven state write.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{TaskRepository, apply_notification_status, find_task};
use crate::models::a2a::{Message, Task};
use crate::repository::OwnedTask;
use crate::repository::context::message::get_messages_by_task;
use systemprompt_identifiers::{ContextId, TaskId, UserId};
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
