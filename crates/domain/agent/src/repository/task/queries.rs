use crate::models::TaskRow;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TaskId, TraceId, UserId};
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
            task_id as "task_id!: TaskId",
            context_id as "context_id!: ContextId",
            status as "status!",
            status_timestamp,
            user_id as "user_id?: UserId",
            session_id as "session_id?: SessionId",
            trace_id as "trace_id?: TraceId",
            agent_name as "agent_name?: AgentName",
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
    .map_err(RepositoryError::database)?;

    let Some(_row) = row else {
        return Ok(None);
    };

    let constructor = TaskConstructor::new(db_pool)?;
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
            task_id as "task_id!: TaskId",
            context_id as "context_id!: ContextId",
            status as "status!",
            status_timestamp,
            user_id as "user_id?: UserId",
            session_id as "session_id?: SessionId",
            trace_id as "trace_id?: TraceId",
            agent_name as "agent_name?: AgentName",
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
    .map_err(RepositoryError::database)?;

    let constructor = TaskConstructor::new(db_pool)?;
    let task_ids: Vec<TaskId> = rows.iter().map(|r| r.task_id.clone()).collect();
    let tasks = constructor.construct_tasks_batch(&task_ids).await?;

    Ok(tasks)
}

pub async fn get_tasks_by_user_id(
    pool: &Arc<PgPool>,
    db_pool: &DbPool,
    user_id: &UserId,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<Vec<Task>, RepositoryError> {
    let lim = limit.map_or(1000, i64::from);
    let off = offset.map_or(0, i64::from);
    let user_id_str = user_id.as_str();

    let rows = sqlx::query_as!(
        TaskRow,
        r#"SELECT
            task_id as "task_id!: TaskId",
            context_id as "context_id!: ContextId",
            status as "status!",
            status_timestamp,
            user_id as "user_id?: UserId",
            session_id as "session_id?: SessionId",
            trace_id as "trace_id?: TraceId",
            agent_name as "agent_name?: AgentName",
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
    .map_err(RepositoryError::database)?;

    let constructor = TaskConstructor::new(db_pool)?;
    let task_ids: Vec<TaskId> = rows.iter().map(|r| r.task_id.clone()).collect();
    let tasks = constructor.construct_tasks_batch(&task_ids).await?;

    Ok(tasks)
}

#[derive(Debug, Clone)]
pub struct TaskContextInfo {
    pub context_id: ContextId,
    pub user_id: UserId,
}

pub async fn get_task_context_info(
    pool: &Arc<PgPool>,
    task_id: &TaskId,
) -> Result<Option<TaskContextInfo>, RepositoryError> {
    let task_id_str = task_id.as_str();
    let row = sqlx::query!(
        r#"SELECT
            context_id as "context_id!: ContextId",
            user_id as "user_id?: UserId"
        FROM agent_tasks WHERE task_id = $1"#,
        task_id_str
    )
    .fetch_optional(pool.as_ref())
    .await
    .map_err(RepositoryError::database)?;

    Ok(row.map(|r| TaskContextInfo {
        context_id: r.context_id,
        user_id: r.user_id.unwrap_or_else(|| UserId::new("")),
    }))
}
