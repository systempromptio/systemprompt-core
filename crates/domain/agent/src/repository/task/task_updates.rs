//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{TaskRepository, task_state_to_db_string};
use crate::models::a2a::{Message, Task, TaskState};
use crate::repository::context::message::{
    PersistMessageSqlxParams, get_next_sequence_number_sqlx, persist_message_sqlx,
};
use systemprompt_traits::RepositoryError;

#[expect(
    missing_debug_implementations,
    reason = "params struct holds non-Debug references"
)]
pub struct UpdateTaskAndSaveMessagesParams<'a> {
    pub task: &'a Task,
    pub user_message: &'a Message,
    pub agent_message: &'a Message,
    pub user_id: Option<&'a systemprompt_identifiers::UserId>,
    pub session_id: &'a systemprompt_identifiers::SessionId,
    pub trace_id: &'a systemprompt_identifiers::TraceId,
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
        let mut tx = self
            .write_pool
            .begin()
            .await
            .map_err(RepositoryError::database)?;

        update_task_row(&mut tx, task).await?;

        let context_id_ref = &task.context_id;

        for message in [user_message, agent_message] {
            let sequence_number = get_next_sequence_number_sqlx(&mut tx, &task.id).await?;
            persist_message_sqlx(PersistMessageSqlxParams {
                tx: &mut tx,
                message,
                task_id: &task.id,
                context_id: context_id_ref,
                sequence_number,
                user_id,
                session_id,
                trace_id,
            })
            .await?;
        }

        tx.commit().await.map_err(RepositoryError::database)?;

        for _ in 0..2 {
            if let Err(e) = self.sessions.increment_message_count(session_id).await {
                tracing::warn!(error = %e, session_id = %session_id, "Failed to increment session message count");
            }
        }

        let updated_task = self.get_task(&task.id).await?.ok_or_else(|| {
            RepositoryError::NotFound(format!("Task not found after update: {}", task.id))
        })?;

        Ok(updated_task)
    }

    pub async fn delete_task(
        &self,
        task_id: &systemprompt_identifiers::TaskId,
    ) -> Result<(), RepositoryError> {
        let task_id_str = task_id.as_str();

        sqlx::query!(
            "DELETE FROM message_parts WHERE message_id IN (SELECT message_id FROM task_messages \
             WHERE task_id = $1)",
            task_id_str
        )
        .execute(&*self.write_pool)
        .await
        .map_err(RepositoryError::database)?;

        sqlx::query!("DELETE FROM task_messages WHERE task_id = $1", task_id_str)
            .execute(&*self.write_pool)
            .await
            .map_err(RepositoryError::database)?;

        sqlx::query!(
            "DELETE FROM task_execution_steps WHERE task_id = $1",
            task_id_str
        )
        .execute(&*self.write_pool)
        .await
        .map_err(RepositoryError::database)?;

        sqlx::query!("DELETE FROM agent_tasks WHERE task_id = $1", task_id_str)
            .execute(&*self.write_pool)
            .await
            .map_err(RepositoryError::database)?;

        Ok(())
    }
}

async fn update_task_row(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    task: &Task,
) -> Result<(), RepositoryError> {
    let status = task_state_to_db_string(task.status.state);
    let metadata_json = task.metadata.as_ref().map_or_else(
        || serde_json::json!({}),
        |m| {
            serde_json::to_value(m).unwrap_or_else(|e| {
                tracing::warn!(error = %e, task_id = %task.id, "Failed to serialize task metadata");
                serde_json::json!({})
            })
        },
    );

    let task_id_str = task.id.as_str();
    let is_completed = task.status.state == TaskState::Completed;

    let result = if is_completed {
        sqlx::query!(
            r#"UPDATE agent_tasks SET
                    status = $1,
                    status_timestamp = $2,
                    metadata = $3,
                    updated_at = CURRENT_TIMESTAMP,
                    completed_at = CURRENT_TIMESTAMP,
                    started_at = COALESCE(started_at, CURRENT_TIMESTAMP),
                    execution_time_ms = EXTRACT(EPOCH FROM (CURRENT_TIMESTAMP - COALESCE(started_at, CURRENT_TIMESTAMP))) * 1000
                WHERE task_id = $4"#,
            status,
            task.status.timestamp,
            metadata_json,
            task_id_str
        )
        .execute(&mut **tx)
        .await
        .map_err(RepositoryError::database)?
    } else {
        sqlx::query!(
            r#"UPDATE agent_tasks SET status = $1, status_timestamp = $2, metadata = $3, updated_at = CURRENT_TIMESTAMP WHERE task_id = $4"#,
            status,
            task.status.timestamp,
            metadata_json,
            task_id_str
        )
        .execute(&mut **tx)
        .await
        .map_err(RepositoryError::database)?
    };

    if result.rows_affected() == 0 {
        return Err(RepositoryError::NotFound(format!(
            "Task not found for update: {}",
            task.id
        )));
    }

    Ok(())
}
