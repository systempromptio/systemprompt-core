//! Task read queries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::models::TaskRow;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TaskId, TraceId, UserId};
use systemprompt_models::api::{KeysetCursor, KeysetPage};
use systemprompt_traits::RepositoryError;

use super::constructor::TaskConstructor;
use crate::models::a2a::Task;

pub(crate) async fn find_task(
    constructor: &TaskConstructor,
    task_id: &TaskId,
) -> Result<Option<Task>, RepositoryError> {
    constructor.construct_task_from_task_id(task_id).await
}

pub(super) async fn list_tasks_by_context(
    pool: &Arc<PgPool>,
    constructor: &TaskConstructor,
    context_id: &ContextId,
) -> Result<Vec<Task>, RepositoryError> {
    let rows = sqlx::query_as!(
        TaskRow,
        r#"SELECT
            task_id as "task_id!: TaskId",
            context_id as "context_id!: ContextId",
            status as "status!: crate::models::a2a::TaskState",
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
        context_id as _
    )
    .fetch_all(pool.as_ref())
    .await?;

    let task_ids: Vec<TaskId> = rows.iter().map(|r| r.task_id.clone()).collect();
    let tasks = constructor.construct_tasks_batch(&task_ids).await?;

    Ok(tasks)
}

#[derive(Debug, Clone, Copy)]
pub struct ListTasksFilter<'a> {
    pub user_id: &'a UserId,
    pub context_id: Option<&'a ContextId>,
    pub status: Option<crate::models::a2a::TaskState>,
    pub status_timestamp_after: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn list_tasks(
    pool: &Arc<PgPool>,
    constructor: &TaskConstructor,
    filter: ListTasksFilter<'_>,
    after: Option<&KeysetCursor<TaskId>>,
    limit: usize,
) -> Result<KeysetPage<Task, TaskId>, RepositoryError> {
    let lookahead = i64::try_from(limit).unwrap_or(i64::MAX).saturating_add(1);
    let keys = sqlx::query!(
        r#"SELECT task_id as "task_id!: TaskId", created_at as "created_at!"
        FROM agent_tasks
        WHERE user_id = $1
          AND ($2::text IS NULL OR context_id = $2)
          AND ($3::text IS NULL OR status = $3)
          AND ($4::timestamptz IS NULL OR COALESCE(status_timestamp, updated_at) >= $4)
          AND ($5::timestamptz IS NULL OR (created_at, task_id) < ($5, $6::text))
        ORDER BY created_at DESC, task_id DESC
        LIMIT $7"#,
        filter.user_id as _,
        filter.context_id as _,
        filter.status.map(crate::models::a2a::TaskState::as_str),
        filter.status_timestamp_after,
        after.map(|cursor| cursor.at),
        after.map(|cursor| &cursor.id) as _,
        lookahead
    )
    .fetch_all(pool.as_ref())
    .await?;

    let page = KeysetPage::from_lookahead(keys, limit, |key| {
        KeysetCursor::new(key.created_at, key.task_id.clone())
    });
    let task_ids: Vec<TaskId> = page.items.iter().map(|key| key.task_id.clone()).collect();
    let mut tasks = constructor.construct_tasks_batch(&task_ids).await?;
    tasks.sort_by_key(|task| {
        task_ids
            .iter()
            .position(|id| *id == task.id)
            .unwrap_or(usize::MAX)
    });
    Ok(KeysetPage {
        items: tasks,
        next_cursor: page.next_cursor,
    })
}
