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
use systemprompt_database::DbPool;
use systemprompt_traits::{DynFileUploadProvider, DynSessionAnalyticsProvider, RepositoryError};

#[derive(Clone)]
pub struct TaskRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
    db_pool: DbPool,
    pub(crate) session_analytics_provider: Option<DynSessionAnalyticsProvider>,
    pub(crate) file_upload_provider: Option<DynFileUploadProvider>,
}

impl std::fmt::Debug for TaskRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskRepository")
            .field("db_pool", &"<DbPool>")
            .field(
                "session_analytics_provider",
                &self.session_analytics_provider.is_some(),
            )
            .field("file_upload_provider", &self.file_upload_provider.is_some())
            .finish()
    }
}

impl TaskRepository {
    pub fn new(db: &DbPool) -> anyhow::Result<Self> {
        let pool = db.pool_arc()?;
        let write_pool = db.write_pool_arc()?;
        Ok(Self {
            pool,
            write_pool,
            db_pool: db.clone(),
            session_analytics_provider: None,
            file_upload_provider: None,
        })
    }

    #[must_use]
    pub fn with_session_analytics_provider(
        mut self,
        provider: DynSessionAnalyticsProvider,
    ) -> Self {
        self.session_analytics_provider = Some(provider);
        self
    }

    #[must_use]
    pub fn with_file_upload_provider(mut self, provider: DynFileUploadProvider) -> Self {
        self.file_upload_provider = Some(provider);
        self
    }

    pub(crate) fn db_pool(&self) -> &DbPool {
        &self.db_pool
    }

    pub async fn create_task(
        &self,
        task: &Task,
        user_id: &systemprompt_identifiers::UserId,
        session_id: &systemprompt_identifiers::SessionId,
        trace_id: &systemprompt_identifiers::TraceId,
        agent_name: &str,
    ) -> Result<String, RepositoryError> {
        let result = create_task(&self.write_pool, task, user_id, session_id, trace_id, agent_name).await?;

        if let Some(ref provider) = self.session_analytics_provider {
            if let Err(e) = provider.increment_task_count(session_id).await {
                tracing::warn!(error = %e, "Failed to increment analytics task count");
            }
        }

        Ok(result)
    }

    pub async fn get_task(
        &self,
        task_id: &systemprompt_identifiers::TaskId,
    ) -> Result<Option<Task>, RepositoryError> {
        get_task(&self.pool, &self.db_pool, task_id).await
    }

    pub async fn get_task_by_str(&self, task_id: &str) -> Result<Option<Task>, RepositoryError> {
        let task_id_typed = systemprompt_identifiers::TaskId::new(task_id);
        self.get_task(&task_id_typed).await
    }

    pub async fn list_tasks_by_context(
        &self,
        context_id: &systemprompt_identifiers::ContextId,
    ) -> Result<Vec<Task>, RepositoryError> {
        list_tasks_by_context(&self.pool, &self.db_pool, context_id).await
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
        get_tasks_by_user_id(&self.pool, &self.db_pool, user_id, limit, offset).await
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
        track_agent_in_context(&self.write_pool, context_id, agent_name).await
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
        update_task_state(&self.write_pool, task_id, state, timestamp).await
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
        update_task_failed_with_error(&self.write_pool, task_id, error_message, timestamp).await
    }

    pub async fn get_task_context_info(
        &self,
        task_id: &systemprompt_identifiers::TaskId,
    ) -> Result<Option<TaskContextInfo>, RepositoryError> {
        get_task_context_info(&self.pool, task_id).await
    }
}

