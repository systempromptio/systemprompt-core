pub mod constructor;
mod mutations;
mod queries;
mod task_messages;
mod task_updates;

pub use constructor::TaskConstructor;
pub use mutations::{
    create_task, task_state_to_db_string, track_agent_in_context, update_task_failed_with_error,
    update_task_state,
};
pub use queries::{
    get_task, get_task_context_info, get_tasks_by_user_id, list_tasks_by_context, TaskContextInfo,
};

use crate::models::a2a::{Task, TaskState};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_analytics::SessionRepository;
use systemprompt_database::DbPool;
use systemprompt_files::{FileUploadService, FilesConfig};
use systemprompt_traits::{Repository as RepositoryTrait, RepositoryError};

#[derive(Debug, Clone)]
pub struct TaskRepository {
    db_pool: DbPool,
    pub(crate) analytics_session_repo: SessionRepository,
    pub(crate) upload_service: Option<FileUploadService>,
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

    pub(crate) fn get_pg_pool(&self) -> Result<Arc<PgPool>, RepositoryError> {
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

    pub async fn update_task_failed_with_error(
        &self,
        task_id: &systemprompt_identifiers::TaskId,
        error_message: &str,
        timestamp: &chrono::DateTime<chrono::Utc>,
    ) -> Result<(), RepositoryError> {
        let pool = self.get_pg_pool()?;
        update_task_failed_with_error(&pool, task_id, error_message, timestamp).await
    }

    pub async fn get_task_context_info(
        &self,
        task_id: &systemprompt_identifiers::TaskId,
    ) -> Result<Option<TaskContextInfo>, RepositoryError> {
        let pool = self.get_pg_pool()?;
        get_task_context_info(&pool, task_id).await
    }
}

impl RepositoryTrait for TaskRepository {
    type Pool = DbPool;
    type Error = RepositoryError;

    fn pool(&self) -> &Self::Pool {
        &self.db_pool
    }
}
