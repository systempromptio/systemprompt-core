use crate::models::TaskRow;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContextId, TaskId, UserId};
use systemprompt_traits::RepositoryError;

use super::constructor::TaskConstructor;
use crate::models::a2a::Task;

pub async fn get_task(
    pool: &Arc<PgPool>,
    db_pool: &DbPool,
    task_id: &TaskId,
) -> Result<Option<Task>, RepositoryError> {
    let task_id_str = task_id.as_str();
    let row = sqlx::query_as!(
        TaskRow,
        r#"SELECT
            task_id as "task_id!",
            context_id as "context_id!",
            status as "status!",
            status_timestamp,
            user_id,
            session_id,
            trace_id,
            agent_name,
            started_at,
            completed_at,
            execution_time_ms,
            error_message,
            metadata,
            created_at as "created_at!",
            updated_at as "updated_at!"
        FROM agent_tasks WHERE task_id = $1"#,
        task_id_str
    )
    .fetch_optional(pool.as_ref())
    .await
    .map_err(|e| RepositoryError::Database(e.to_string()))?;

    let Some(_row) = row else {
        return Ok(None);
    };

    let constructor = TaskConstructor::new(db_pool.clone());
    let task = constructor.construct_task_from_task_id(task_id).await?;

    Ok(Some(task))
}

pub async fn list_tasks_by_context(
    pool: &Arc<PgPool>,
    db_pool: &DbPool,
    context_id: &ContextId,
) -> Result<Vec<Task>, RepositoryError> {
    let context_id_str = context_id.as_str();
    let rows = sqlx::query_as!(
        TaskRow,
        r#"SELECT
            task_id as "task_id!",
            context_id as "context_id!",
            status as "status!",
            status_timestamp,
            user_id,
            session_id,
            trace_id,
            agent_name,
            started_at,
            completed_at,
            execution_time_ms,
            error_message,
            metadata,
            created_at as "created_at!",
            updated_at as "updated_at!"
        FROM agent_tasks WHERE context_id = $1 ORDER BY created_at ASC"#,
        context_id_str
    )
    .fetch_all(pool.as_ref())
    .await
    .map_err(|e| RepositoryError::Database(e.to_string()))?;

    let constructor = TaskConstructor::new(db_pool.clone());
    let mut tasks = Vec::new();

    for row in rows {
        let task_id = TaskId::new(&row.task_id);
        tasks.push(constructor.construct_task_from_task_id(&task_id).await?);
    }

    Ok(tasks)
}

pub async fn get_tasks_by_user_id(
    pool: &Arc<PgPool>,
    db_pool: &DbPool,
    user_id: &UserId,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<Vec<Task>, RepositoryError> {
    let lim = limit.map(i64::from).unwrap_or(1000);
    let off = offset.map(i64::from).unwrap_or(0);
    let user_id_str = user_id.as_str();

    let rows = sqlx::query_as!(
        TaskRow,
        r#"SELECT
            task_id as "task_id!",
            context_id as "context_id!",
            status as "status!",
            status_timestamp,
            user_id,
            session_id,
            trace_id,
            agent_name,
            started_at,
            completed_at,
            execution_time_ms,
            error_message,
            metadata,
            created_at as "created_at!",
            updated_at as "updated_at!"
        FROM agent_tasks WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3"#,
        user_id_str,
        lim,
        off
    )
    .fetch_all(pool.as_ref())
    .await
    .map_err(|e| RepositoryError::Database(e.to_string()))?;

    let constructor = TaskConstructor::new(db_pool.clone());
    let mut tasks = Vec::new();

    for row in &rows {
        let task_id = TaskId::new(&row.task_id);
        tasks.push(constructor.construct_task_from_task_id(&task_id).await?);
    }

    Ok(tasks)
}

#[derive(Debug, Clone)]
pub struct TaskContextInfo {
    pub context_id: String,
    pub user_id: String,
}

impl TaskContextInfo {
    pub fn context_id(&self) -> ContextId {
        ContextId::new(&self.context_id)
    }

    pub fn user_id(&self) -> UserId {
        UserId::new(&self.user_id)
    }
}

pub async fn get_task_context_info(
    pool: &Arc<PgPool>,
    task_id: &TaskId,
) -> Result<Option<TaskContextInfo>, RepositoryError> {
    let task_id_str = task_id.as_str();
    let row = sqlx::query!(
        r#"SELECT
            context_id as "context_id!",
            user_id
        FROM agent_tasks WHERE task_id = $1"#,
        task_id_str
    )
    .fetch_optional(pool.as_ref())
    .await
    .map_err(|e| RepositoryError::Database(e.to_string()))?;

    Ok(row.map(|r| TaskContextInfo {
        context_id: r.context_id,
        user_id: r.user_id.unwrap_or_default(),
    }))
}
