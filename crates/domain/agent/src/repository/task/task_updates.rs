use super::{task_state_to_db_string, TaskRepository};
use crate::models::a2a::{Message, Task, TaskState};
use crate::repository::context::message::{
    get_next_sequence_number_sqlx, persist_message_sqlx, FileUploadContext,
};
use systemprompt_traits::RepositoryError;

impl TaskRepository {
    pub async fn update_task_and_save_messages(
        &self,
        task: &Task,
        user_message: &Message,
        agent_message: &Message,
        user_id: Option<&systemprompt_identifiers::UserId>,
        session_id: &systemprompt_identifiers::SessionId,
        trace_id: &systemprompt_identifiers::TraceId,
    ) -> Result<Task, RepositoryError> {
        let pool = self.get_pg_pool()?;
        let mut tx = pool
            .begin()
            .await
            .map_err(|e| RepositoryError::database(e))?;

        let status = task_state_to_db_string(task.status.state.clone());
        let metadata_json = task
            .metadata
            .as_ref()
            .map(|m| {
                serde_json::to_value(m).unwrap_or_else(|e| {
                    tracing::warn!(error = %e, task_id = %task.id, "Failed to serialize task metadata");
                    serde_json::json!({})
                })
            })
            .unwrap_or_else(|| serde_json::json!({}));

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
            .execute(&mut *tx)
            .await
            .map_err(|e| RepositoryError::database(e))?
        } else {
            sqlx::query!(
                r#"UPDATE agent_tasks SET status = $1, status_timestamp = $2, metadata = $3, updated_at = CURRENT_TIMESTAMP WHERE task_id = $4"#,
                status,
                task.status.timestamp,
                metadata_json,
                task_id_str
            )
            .execute(&mut *tx)
            .await
            .map_err(|e| RepositoryError::database(e))?
        };

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound(format!(
                "Task not found for update: {}",
                task.id
            )));
        }

        let upload_ctx = self.file_upload_provider.as_ref().map(|svc| FileUploadContext {
            upload_provider: svc,
            context_id: &task.context_id,
            user_id,
            session_id: Some(session_id),
            trace_id: Some(trace_id),
        });

        let user_seq = get_next_sequence_number_sqlx(&mut tx, &task.id).await?;
        persist_message_sqlx(
            &mut tx,
            user_message,
            &task.id,
            &task.context_id,
            user_seq,
            user_id,
            session_id,
            trace_id,
            upload_ctx.as_ref(),
        )
        .await?;

        let agent_seq = get_next_sequence_number_sqlx(&mut tx, &task.id).await?;
        persist_message_sqlx(
            &mut tx,
            agent_message,
            &task.id,
            &task.context_id,
            agent_seq,
            user_id,
            session_id,
            trace_id,
            upload_ctx.as_ref(),
        )
        .await?;

        tx.commit()
            .await
            .map_err(|e| RepositoryError::database(e))?;

        if let Some(ref analytics_provider) = self.session_analytics_provider {
            for _ in 0..2 {
                if let Err(e) = analytics_provider.increment_message_count(session_id).await {
                    tracing::warn!(error = %e, "Failed to increment analytics message count");
                }
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
        let pool = self.get_pg_pool()?;
        let task_id_str = task_id.as_str();

        sqlx::query!(
            "DELETE FROM message_parts WHERE message_id IN (SELECT message_id FROM task_messages \
             WHERE task_id = $1)",
            task_id_str
        )
        .execute(&*pool)
        .await
        .map_err(|e| RepositoryError::database(e))?;

        sqlx::query!("DELETE FROM task_messages WHERE task_id = $1", task_id_str)
            .execute(&*pool)
            .await
            .map_err(|e| RepositoryError::database(e))?;

        sqlx::query!(
            "DELETE FROM task_execution_steps WHERE task_id = $1",
            task_id_str
        )
        .execute(&*pool)
        .await
        .map_err(|e| RepositoryError::database(e))?;

        sqlx::query!("DELETE FROM agent_tasks WHERE task_id = $1", task_id_str)
            .execute(&*pool)
            .await
            .map_err(|e| RepositoryError::database(e))?;

        Ok(())
    }
}
