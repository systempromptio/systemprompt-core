pub mod constructor;
mod mutations;
mod queries;

pub use constructor::TaskConstructor;
pub use mutations::{
    create_task, task_state_to_db_string, track_agent_in_context, update_task_failed_with_error,
    update_task_state,
};
pub use queries::{
    get_task, get_task_context_info, get_tasks_by_user_id, list_tasks_by_context, TaskContextInfo,
};

use crate::models::a2a::{Message, Part, Task, TaskState};
use crate::repository::context::message::{
    get_message_parts, get_messages_by_context, get_messages_by_task, get_next_sequence_number,
    get_next_sequence_number_in_tx, get_next_sequence_number_sqlx, persist_message_sqlx,
    persist_message_with_tx, FileUploadContext,
};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_core_analytics::SessionRepository;
use systemprompt_core_database::DbPool;
use systemprompt_core_files::{FileUploadService, FilesConfig};
use systemprompt_traits::{Repository as RepositoryTrait, RepositoryError};

#[derive(Debug, Clone)]
pub struct TaskRepository {
    db_pool: DbPool,
    analytics_session_repo: SessionRepository,
    upload_service: Option<FileUploadService>,
}

impl TaskRepository {
    #[must_use]
    pub fn new(db_pool: DbPool) -> Self {
        let analytics_session_repo = SessionRepository::new(db_pool.clone());
        let upload_service = Self::init_upload_service(&db_pool);
        Self {
            db_pool,
            analytics_session_repo,
            upload_service,
        }
    }

    fn init_upload_service(db_pool: &DbPool) -> Option<FileUploadService> {
        let files_config = FilesConfig::get_optional()?.clone();
        FileUploadService::new(db_pool, files_config)
            .map_err(|e| {
                tracing::warn!(error = %e, "Failed to initialize FileUploadService");
                e
            })
            .ok()
    }

    fn get_pg_pool(&self) -> Result<Arc<PgPool>, RepositoryError> {
        self.db_pool
            .as_ref()
            .get_postgres_pool()
            .ok_or_else(|| RepositoryError::Database("PostgreSQL pool not available".to_string()))
    }

    pub async fn create_task(
        &self,
        task: &Task,
        user_id: &systemprompt_identifiers::UserId,
        session_id: &systemprompt_identifiers::SessionId,
        trace_id: &systemprompt_identifiers::TraceId,
        agent_name: &str,
    ) -> Result<String, RepositoryError> {
        let pool = self.get_pg_pool()?;
        let result = create_task(&pool, task, user_id, session_id, trace_id, agent_name).await?;

        if let Err(e) = self
            .analytics_session_repo
            .increment_task_count(session_id)
            .await
        {
            tracing::warn!(error = %e, "Failed to increment analytics task count");
        }

        Ok(result)
    }

    pub async fn get_task(
        &self,
        task_id: &systemprompt_identifiers::TaskId,
    ) -> Result<Option<Task>, RepositoryError> {
        let pool = self.get_pg_pool()?;
        get_task(&pool, &self.db_pool, task_id).await
    }

    pub async fn get_task_by_str(&self, task_id: &str) -> Result<Option<Task>, RepositoryError> {
        let task_id_typed = systemprompt_identifiers::TaskId::new(task_id);
        self.get_task(&task_id_typed).await
    }

    pub async fn list_tasks_by_context(
        &self,
        context_id: &systemprompt_identifiers::ContextId,
    ) -> Result<Vec<Task>, RepositoryError> {
        let pool = self.get_pg_pool()?;
        list_tasks_by_context(&pool, &self.db_pool, context_id).await
    }

    pub async fn list_tasks_by_context_str(
        &self,
        context_id: &str,
    ) -> Result<Vec<Task>, RepositoryError> {
        let context_id_typed = systemprompt_identifiers::ContextId::new(context_id);
        self.list_tasks_by_context(&context_id_typed).await
    }

    pub async fn get_tasks_by_user_id(
        &self,
        user_id: &systemprompt_identifiers::UserId,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<Task>, RepositoryError> {
        let pool = self.get_pg_pool()?;
        get_tasks_by_user_id(&pool, &self.db_pool, user_id, limit, offset).await
    }

    pub async fn get_tasks_by_user_id_str(
        &self,
        user_id: &str,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<Task>, RepositoryError> {
        let user_id_typed = systemprompt_identifiers::UserId::new(user_id);
        self.get_tasks_by_user_id(&user_id_typed, limit, offset)
            .await
    }

    pub async fn track_agent_in_context(
        &self,
        context_id: &systemprompt_identifiers::ContextId,
        agent_name: &str,
    ) -> Result<(), RepositoryError> {
        let pool = self.get_pg_pool()?;
        track_agent_in_context(&pool, context_id, agent_name).await
    }

    pub async fn track_agent_in_context_str(
        &self,
        context_id: &str,
        agent_name: &str,
    ) -> Result<(), RepositoryError> {
        let context_id_typed = systemprompt_identifiers::ContextId::new(context_id);
        self.track_agent_in_context(&context_id_typed, agent_name)
            .await
    }

    pub async fn update_task_state(
        &self,
        task_id: &systemprompt_identifiers::TaskId,
        state: TaskState,
        timestamp: &chrono::DateTime<chrono::Utc>,
    ) -> Result<(), RepositoryError> {
        let pool = self.get_pg_pool()?;
        update_task_state(&pool, task_id, state, timestamp).await
    }

    pub async fn update_task_state_str(
        &self,
        task_id: &str,
        state: TaskState,
        timestamp: &chrono::DateTime<chrono::Utc>,
    ) -> Result<(), RepositoryError> {
        let task_id_typed = systemprompt_identifiers::TaskId::new(task_id);
        self.update_task_state(&task_id_typed, state, timestamp)
            .await
    }

    /// Update task to failed state with an error message for debugging
    pub async fn update_task_failed_with_error(
        &self,
        task_id: &systemprompt_identifiers::TaskId,
        error_message: &str,
        timestamp: &chrono::DateTime<chrono::Utc>,
    ) -> Result<(), RepositoryError> {
        let pool = self.get_pg_pool()?;
        update_task_failed_with_error(&pool, task_id, error_message, timestamp).await
    }

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
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

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
            .map_err(|e| RepositoryError::Database(e.to_string()))?
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
            .map_err(|e| RepositoryError::Database(e.to_string()))?
        };

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound(format!(
                "Task not found for update: {}",
                task.id
            )));
        }

        let upload_ctx = self.upload_service.as_ref().map(|svc| FileUploadContext {
            upload_service: svc,
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
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        for _ in 0..2 {
            if let Err(e) = self
                .analytics_session_repo
                .increment_message_count(session_id)
                .await
            {
                tracing::warn!(error = %e, "Failed to increment analytics message count");
            }
        }

        let updated_task = self.get_task(&task.id).await?.ok_or_else(|| {
            RepositoryError::NotFound(format!("Task not found after update: {}", task.id))
        })?;

        Ok(updated_task)
    }

    pub async fn get_next_sequence_number(
        &self,
        task_id: &systemprompt_identifiers::TaskId,
    ) -> Result<i32, RepositoryError> {
        let pool = self.get_pg_pool()?;
        get_next_sequence_number(&pool, task_id).await
    }

    pub async fn get_messages_by_task(
        &self,
        task_id: &systemprompt_identifiers::TaskId,
    ) -> Result<Vec<Message>, RepositoryError> {
        let pool = self.get_pg_pool()?;
        get_messages_by_task(&pool, task_id).await
    }

    pub async fn get_message_parts(
        &self,
        message_id: &systemprompt_identifiers::MessageId,
    ) -> Result<Vec<Part>, RepositoryError> {
        let pool = self.get_pg_pool()?;
        get_message_parts(&pool, message_id).await
    }

    pub async fn get_messages_by_context(
        &self,
        context_id: &systemprompt_identifiers::ContextId,
    ) -> Result<Vec<Message>, RepositoryError> {
        let pool = self.get_pg_pool()?;
        get_messages_by_context(&pool, context_id).await
    }

    pub async fn get_next_sequence_number_in_tx(
        &self,
        tx: &mut dyn systemprompt_core_database::DatabaseTransaction,
        task_id: &systemprompt_identifiers::TaskId,
    ) -> Result<i32, RepositoryError> {
        get_next_sequence_number_in_tx(tx, task_id).await
    }

    pub async fn persist_message_with_tx(
        &self,
        tx: &mut dyn systemprompt_core_database::DatabaseTransaction,
        message: &Message,
        task_id: &systemprompt_identifiers::TaskId,
        context_id: &systemprompt_identifiers::ContextId,
        sequence_number: i32,
        user_id: Option<&systemprompt_identifiers::UserId>,
        session_id: &systemprompt_identifiers::SessionId,
        trace_id: &systemprompt_identifiers::TraceId,
    ) -> Result<(), RepositoryError> {
        persist_message_with_tx(
            tx,
            message,
            task_id,
            context_id,
            sequence_number,
            user_id,
            session_id,
            trace_id,
        )
        .await
    }

    pub async fn get_task_context_info(
        &self,
        task_id: &systemprompt_identifiers::TaskId,
    ) -> Result<Option<TaskContextInfo>, RepositoryError> {
        let pool = self.get_pg_pool()?;
        get_task_context_info(&pool, task_id).await
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
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        sqlx::query!("DELETE FROM task_messages WHERE task_id = $1", task_id_str)
            .execute(&*pool)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        sqlx::query!(
            "DELETE FROM task_execution_steps WHERE task_id = $1",
            task_id_str
        )
        .execute(&*pool)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        sqlx::query!("DELETE FROM agent_tasks WHERE task_id = $1", task_id_str)
            .execute(&*pool)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(())
    }
}

impl RepositoryTrait for TaskRepository {
    type Pool = DbPool;
    type Error = RepositoryError;

    fn pool(&self) -> &Self::Pool {
        &self.db_pool
    }
}
