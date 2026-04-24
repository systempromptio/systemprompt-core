use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_traits::RepositoryError;

use crate::models::a2a::{Task, TaskState};

pub const fn task_state_to_db_string(state: TaskState) -> &'static str {
    match state {
        TaskState::Pending => "TASK_STATE_PENDING",
        TaskState::Submitted => "TASK_STATE_SUBMITTED",
        TaskState::Working => "TASK_STATE_WORKING",
        TaskState::InputRequired => "TASK_STATE_INPUT_REQUIRED",
        TaskState::Completed => "TASK_STATE_COMPLETED",
        TaskState::Canceled => "TASK_STATE_CANCELED",
        TaskState::Failed => "TASK_STATE_FAILED",
        TaskState::Rejected => "TASK_STATE_REJECTED",
        TaskState::AuthRequired => "TASK_STATE_AUTH_REQUIRED",
        TaskState::Unknown => "TASK_STATE_UNKNOWN",
    }
}

#[allow(missing_debug_implementations)]
pub struct CreateTaskParams<'a> {
    pub pool: &'a Arc<PgPool>,
    pub task: &'a Task,
    pub user_id: &'a systemprompt_identifiers::UserId,
    pub session_id: &'a systemprompt_identifiers::SessionId,
    pub trace_id: &'a systemprompt_identifiers::TraceId,
    pub agent_name: &'a str,
}

pub async fn create_task(params: CreateTaskParams<'_>) -> Result<String, RepositoryError> {
    let CreateTaskParams {
        pool,
        task,
        user_id,
        session_id,
        trace_id,
        agent_name,
    } = params;
    let metadata_json = task.metadata.as_ref().map_or_else(
        || serde_json::json!({}),
        |m| {
            serde_json::to_value(m).unwrap_or_else(|e| {
                tracing::warn!(error = %e, task_id = %task.id, "Failed to serialize task metadata");
                serde_json::json!({})
            })
        },
    );

    let status = task_state_to_db_string(task.status.state);
    let task_id_str = task.id.as_str();
    let context_id_str = task.context_id.as_str();
    let user_id_str = user_id.as_ref();
    let session_id_str = session_id.as_ref();
    let trace_id_str = trace_id.as_ref();

    sqlx::query!(
        r#"INSERT INTO agent_tasks (task_id, context_id, status, status_timestamp, user_id, session_id, trace_id, metadata, agent_name)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
        task_id_str,
        context_id_str,
        status,
        task.status.timestamp,
        user_id_str,
        session_id_str,
        trace_id_str,
        metadata_json,
        agent_name
    )
    .execute(pool.as_ref())
    .await
    .map_err(RepositoryError::database)?;

    Ok(task.id.to_string())
}

pub async fn track_agent_in_context(
    pool: &Arc<PgPool>,
    context_id: &systemprompt_identifiers::ContextId,
    agent_name: &str,
) -> Result<(), RepositoryError> {
    let context_id_str = context_id.as_str();
    sqlx::query!(
        r#"INSERT INTO context_agents (context_id, agent_name) VALUES ($1, $2)
        ON CONFLICT (context_id, agent_name) DO NOTHING"#,
        context_id_str,
        agent_name
    )
    .execute(pool.as_ref())
    .await
    .map_err(RepositoryError::database)?;

    Ok(())
}

pub async fn update_task_state(
    pool: &Arc<PgPool>,
    task_id: &systemprompt_identifiers::TaskId,
    state: TaskState,
    timestamp: &chrono::DateTime<chrono::Utc>,
) -> Result<(), RepositoryError> {
    let status = task_state_to_db_string(state);
    let task_id_str = task_id.as_str();

    let mut tx = pool.begin().await.map_err(RepositoryError::database)?;

    let current = sqlx::query!(
        r#"SELECT status, version FROM agent_tasks WHERE task_id = $1 FOR UPDATE"#,
        task_id_str
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(RepositoryError::database)?
    .ok_or_else(|| RepositoryError::NotFound(format!("task {task_id_str}")))?;

    let current_state: TaskState = current.status.parse().map_err(|e: String| {
        RepositoryError::InvalidData(format!("unrecognised stored task state: {e}"))
    })?;

    if current_state == state {
        tx.commit().await.map_err(RepositoryError::database)?;
        return Ok(());
    }

    if !current_state.can_transition_to(&state) {
        return Err(RepositoryError::ConstraintViolation(format!(
            "invalid task state transition for {task_id_str}: {current_state:?} -> {state:?}"
        )));
    }

    let expected_version = current.version;

    let rows_affected = if state == TaskState::Completed {
        sqlx::query!(
            r#"UPDATE agent_tasks
               SET status = $1,
                   status_timestamp = $2,
                   updated_at = CURRENT_TIMESTAMP,
                   completed_at = CURRENT_TIMESTAMP,
                   started_at = COALESCE(started_at, CURRENT_TIMESTAMP),
                   execution_time_ms = EXTRACT(EPOCH FROM (CURRENT_TIMESTAMP - COALESCE(started_at, CURRENT_TIMESTAMP))) * 1000,
                   version = version + 1
               WHERE task_id = $3 AND version = $4"#,
            status,
            timestamp,
            task_id_str,
            expected_version
        )
        .execute(&mut *tx)
        .await
        .map_err(RepositoryError::database)?
        .rows_affected()
    } else if state == TaskState::Working {
        sqlx::query!(
            r#"UPDATE agent_tasks
               SET status = $1,
                   status_timestamp = $2,
                   updated_at = CURRENT_TIMESTAMP,
                   started_at = COALESCE(started_at, CURRENT_TIMESTAMP),
                   version = version + 1
               WHERE task_id = $3 AND version = $4"#,
            status,
            timestamp,
            task_id_str,
            expected_version
        )
        .execute(&mut *tx)
        .await
        .map_err(RepositoryError::database)?
        .rows_affected()
    } else {
        sqlx::query!(
            r#"UPDATE agent_tasks
               SET status = $1,
                   status_timestamp = $2,
                   updated_at = CURRENT_TIMESTAMP,
                   version = version + 1
               WHERE task_id = $3 AND version = $4"#,
            status,
            timestamp,
            task_id_str,
            expected_version
        )
        .execute(&mut *tx)
        .await
        .map_err(RepositoryError::database)?
        .rows_affected()
    };

    if rows_affected == 0 {
        return Err(RepositoryError::ConstraintViolation(format!(
            "stale task update for {task_id_str}: expected version {expected_version}"
        )));
    }

    tx.commit().await.map_err(RepositoryError::database)?;
    Ok(())
}

pub async fn update_task_failed_with_error(
    pool: &Arc<PgPool>,
    task_id: &systemprompt_identifiers::TaskId,
    error_message: &str,
    timestamp: &chrono::DateTime<chrono::Utc>,
) -> Result<(), RepositoryError> {
    let task_id_str = task_id.as_str();

    let mut tx = pool.begin().await.map_err(RepositoryError::database)?;

    let current = sqlx::query!(
        r#"SELECT status, version FROM agent_tasks WHERE task_id = $1 FOR UPDATE"#,
        task_id_str
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(RepositoryError::database)?
    .ok_or_else(|| RepositoryError::NotFound(format!("task {task_id_str}")))?;

    let current_state: TaskState = current.status.parse().map_err(|e: String| {
        RepositoryError::InvalidData(format!("unrecognised stored task state: {e}"))
    })?;

    if current_state == TaskState::Failed {
        tx.commit().await.map_err(RepositoryError::database)?;
        return Ok(());
    }

    if !current_state.can_transition_to(&TaskState::Failed) {
        return Err(RepositoryError::ConstraintViolation(format!(
            "invalid task state transition for {task_id_str}: {current_state:?} -> Failed"
        )));
    }

    let expected_version = current.version;

    let rows_affected = sqlx::query!(
        r#"UPDATE agent_tasks SET
            status = 'TASK_STATE_FAILED',
            status_timestamp = $1,
            error_message = $2,
            updated_at = CURRENT_TIMESTAMP,
            completed_at = CURRENT_TIMESTAMP,
            started_at = COALESCE(started_at, CURRENT_TIMESTAMP),
            execution_time_ms = EXTRACT(EPOCH FROM (CURRENT_TIMESTAMP - COALESCE(started_at, CURRENT_TIMESTAMP))) * 1000,
            version = version + 1
        WHERE task_id = $3 AND version = $4"#,
        timestamp,
        error_message,
        task_id_str,
        expected_version
    )
    .execute(&mut *tx)
    .await
    .map_err(RepositoryError::database)?
    .rows_affected();

    if rows_affected == 0 {
        return Err(RepositoryError::ConstraintViolation(format!(
            "stale task update for {task_id_str}: expected version {expected_version}"
        )));
    }

    tx.commit().await.map_err(RepositoryError::database)?;
    Ok(())
}
