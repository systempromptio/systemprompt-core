//! Transactional task completion: message history and the guarded state
//! transition land in one transaction, so a task is never marked complete
//! without its messages.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::TaskRepository;
use super::state::transition_in_tx;
use crate::models::a2a::{Message, Task};
use crate::repository::context::message::{
    PersistMessageSqlxParams, get_next_sequence_number_sqlx, persist_message_sqlx,
};
use systemprompt_identifiers::{ContextId, SessionId, TaskId, TraceId, UserId};
use systemprompt_traits::RepositoryError;

#[expect(
    missing_debug_implementations,
    reason = "params struct holds non-Debug references"
)]
pub struct UpdateTaskAndSaveMessagesParams<'a> {
    pub task: &'a Task,
    pub user_message: &'a Message,
    pub agent_message: &'a Message,
    pub user_id: Option<&'a UserId>,
    pub session_id: &'a SessionId,
    pub trace_id: &'a TraceId,
}

#[expect(
    missing_debug_implementations,
    reason = "params struct holds non-Debug references"
)]
pub struct PersistMessagesTxParams<'a> {
    pub task_id: &'a TaskId,
    pub context_id: &'a ContextId,
    pub messages: &'a [Message],
    pub user_id: Option<&'a UserId>,
    pub session_id: &'a SessionId,
    pub trace_id: &'a TraceId,
}

impl TaskRepository {
    pub async fn update_task_and_save_messages(
        &self,
        params: UpdateTaskAndSaveMessagesParams<'_>,
    ) -> Result<Task, RepositoryError> {
        let UpdateTaskAndSaveMessagesParams {
            task,
            user_message,
            agent_message,
            user_id,
            session_id,
            trace_id,
        } = params;
        let messages = [user_message.clone(), agent_message.clone()];
        let mut tx = self
            .write_pool
            .begin()
            .await
            .map_err(RepositoryError::database)?;

        persist_messages_in_tx(
            &mut tx,
            &PersistMessagesTxParams {
                task_id: &task.id,
                context_id: &task.context_id,
                messages: &messages,
                user_id,
                session_id,
                trace_id,
            },
        )
        .await?;
        update_task_metadata(&mut tx, task).await?;
        let timestamp = task.status.timestamp.unwrap_or_else(chrono::Utc::now);
        transition_in_tx(&mut tx, &task.id, task.status.state, &timestamp).await?;

        tx.commit().await.map_err(RepositoryError::database)?;

        self.count_messages(session_id, messages.len()).await;

        self.get_task(&task.id).await?.ok_or_else(|| {
            RepositoryError::NotFound(format!("Task not found after update: {}", task.id))
        })
    }

    pub async fn persist_messages(
        &self,
        params: PersistMessagesTxParams<'_>,
    ) -> Result<Vec<i32>, RepositoryError> {
        let mut tx = self
            .write_pool
            .begin()
            .await
            .map_err(RepositoryError::database)?;
        let sequence_numbers = persist_messages_in_tx(&mut tx, &params).await?;
        tx.commit().await.map_err(RepositoryError::database)?;

        self.count_messages(params.session_id, params.messages.len())
            .await;
        Ok(sequence_numbers)
    }

    async fn count_messages(&self, session_id: &SessionId, count: usize) {
        for _ in 0..count {
            if let Err(e) = self.sessions.increment_message_count(session_id).await {
                tracing::warn!(error = %e, session_id = %session_id, "Failed to increment session message count");
            }
        }
    }

    pub async fn delete_task(&self, task_id: &TaskId) -> Result<(), RepositoryError> {
        let task_id_str = task_id.as_str();
        let mut tx = self
            .write_pool
            .begin()
            .await
            .map_err(RepositoryError::database)?;

        sqlx::query!(
            "DELETE FROM message_parts WHERE message_id IN (SELECT message_id FROM task_messages \
             WHERE task_id = $1)",
            task_id_str
        )
        .execute(&mut *tx)
        .await
        .map_err(RepositoryError::database)?;

        sqlx::query!("DELETE FROM task_messages WHERE task_id = $1", task_id_str)
            .execute(&mut *tx)
            .await
            .map_err(RepositoryError::database)?;

        sqlx::query!(
            "DELETE FROM task_execution_steps WHERE task_id = $1",
            task_id_str
        )
        .execute(&mut *tx)
        .await
        .map_err(RepositoryError::database)?;

        sqlx::query!("DELETE FROM agent_tasks WHERE task_id = $1", task_id_str)
            .execute(&mut *tx)
            .await
            .map_err(RepositoryError::database)?;

        tx.commit().await.map_err(RepositoryError::database)
    }
}

async fn persist_messages_in_tx(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    params: &PersistMessagesTxParams<'_>,
) -> Result<Vec<i32>, RepositoryError> {
    let mut sequence_numbers = Vec::with_capacity(params.messages.len());
    for message in params.messages {
        let sequence_number = get_next_sequence_number_sqlx(tx, params.task_id).await?;
        persist_message_sqlx(PersistMessageSqlxParams {
            tx,
            message,
            task_id: params.task_id,
            context_id: params.context_id,
            sequence_number,
            user_id: params.user_id,
            session_id: params.session_id,
            trace_id: params.trace_id,
        })
        .await?;
        sequence_numbers.push(sequence_number);
    }
    Ok(sequence_numbers)
}

async fn update_task_metadata(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    task: &Task,
) -> Result<(), RepositoryError> {
    let metadata_json = match &task.metadata {
        Some(metadata) => serde_json::to_value(metadata).map_err(RepositoryError::Serialization)?,
        None => serde_json::json!({}),
    };

    let result = sqlx::query!(
        r#"UPDATE agent_tasks SET metadata = $1, updated_at = CURRENT_TIMESTAMP WHERE task_id = $2"#,
        metadata_json,
        task.id.as_str()
    )
    .execute(&mut **tx)
    .await
    .map_err(RepositoryError::database)?;

    if result.rows_affected() == 0 {
        return Err(RepositoryError::NotFound(format!(
            "Task not found for update: {}",
            task.id
        )));
    }

    Ok(())
}
