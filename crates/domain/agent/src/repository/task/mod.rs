//! Persistence for A2A tasks, their messages, parts, and execution state.
//!
//! [`TaskRepository`] is the repository facade over the `agent_tasks` table and
//! its satellites (`task_messages`, `message_parts`, `task_execution_steps`).
//! It splits reads and writes across separate pools, optionally drives session
//! analytics and file-upload providers, and delegates aggregate reassembly to
//! [`TaskConstructor`]. Query, mutation, and state-transition helpers live in
//! the sibling submodules and are re-exported here.

pub mod constructor;
mod mutations;
mod queries;
mod state;
mod task_messages;
mod task_updates;

pub use constructor::TaskConstructor;
pub use mutations::{
    CreateTaskParams, create_task, task_state_to_db_string, track_agent_in_context,
};
pub use queries::{
    TaskContextInfo, get_task, get_task_context_info, get_tasks_by_user_id, list_tasks_by_context,
};
pub use state::{apply_notification_status, update_task_failed_with_error, update_task_state};
pub use task_updates::UpdateTaskAndSaveMessagesParams;

use crate::models::a2a::{Task, TaskState};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, TraceId, UserId};
use systemprompt_traits::{DynFileUploadProvider, DynSessionAnalyticsProvider, RepositoryError};

#[expect(
    missing_debug_implementations,
    reason = "params struct holds non-Debug references"
)]
pub struct RepoCreateTaskParams<'a> {
    pub task: &'a Task,
    pub user_id: &'a UserId,
    pub session_id: &'a SessionId,
    pub trace_id: &'a TraceId,
    pub agent_name: &'a str,
}

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
            .field("pool", &"<PgPool>")
            .field("write_pool", &"<PgPool>")
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
    pub fn new(db: &DbPool) -> Result<Self, crate::error::AgentError> {
        let pool = db
            .pool_arc()
            .map_err(|e| crate::error::AgentError::Init(e.to_string()))?;
        let write_pool = db
            .write_pool_arc()
            .map_err(|e| crate::error::AgentError::Init(e.to_string()))?;
        Ok(Self {
            pool,
            write_pool,
            db_pool: Arc::clone(db),
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

    pub(crate) const fn db_pool(&self) -> &DbPool {
        &self.db_pool
    }

    pub async fn create_task(
        &self,
        params: RepoCreateTaskParams<'_>,
    ) -> Result<String, RepositoryError> {
        let result = create_task(CreateTaskParams {
            pool: &self.write_pool,
            task: params.task,
            user_id: params.user_id,
            session_id: params.session_id,
            trace_id: params.trace_id,
            agent_name: params.agent_name,
        })
        .await?;

        if let Some(ref provider) = self.session_analytics_provider {
            if let Err(e) = provider.increment_task_count(params.session_id).await {
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

    pub async fn list_tasks_by_context(
        &self,
        context_id: &systemprompt_identifiers::ContextId,
    ) -> Result<Vec<Task>, RepositoryError> {
        list_tasks_by_context(&self.pool, &self.db_pool, context_id).await
    }

    pub async fn get_tasks_by_user_id(
        &self,
        user_id: &UserId,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<Task>, RepositoryError> {
        get_tasks_by_user_id(&self.pool, &self.db_pool, user_id, limit, offset).await
    }

    pub async fn track_agent_in_context(
        &self,
        context_id: &systemprompt_identifiers::ContextId,
        agent_name: &str,
    ) -> Result<(), RepositoryError> {
        track_agent_in_context(&self.write_pool, context_id, agent_name).await
    }

    pub async fn update_task_state(
        &self,
        task_id: &systemprompt_identifiers::TaskId,
        state: TaskState,
        timestamp: &chrono::DateTime<chrono::Utc>,
    ) -> Result<(), RepositoryError> {
        update_task_state(&self.write_pool, task_id, state, timestamp).await
    }

    pub async fn apply_notification_status(
        &self,
        task_id: &systemprompt_identifiers::TaskId,
        state: &str,
        timestamp: &chrono::DateTime<chrono::Utc>,
    ) -> Result<(), RepositoryError> {
        apply_notification_status(&self.write_pool, task_id, state, timestamp).await
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

    pub async fn validate_task_ownership(
        &self,
        task_id: &systemprompt_identifiers::TaskId,
        user_id: &UserId,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query_scalar!(
            "SELECT t.task_id FROM agent_tasks t JOIN user_contexts c ON t.context_id = \
             c.context_id WHERE t.task_id = $1 AND c.user_id = $2",
            task_id.as_str(),
            user_id.as_str()
        )
        .fetch_optional(self.pool.as_ref())
        .await
        .map_err(RepositoryError::database)?;

        match result {
            Some(_) => Ok(()),
            None => Err(RepositoryError::NotFound(format!(
                "Task {task_id} not found or user {user_id} does not have access"
            ))),
        }
    }
}
