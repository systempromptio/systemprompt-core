//! Optimistic-concurrency state transitions for `agent_tasks`.
//!
//! Each transition reads the current row `FOR UPDATE`, validates the move
//! against [`TaskState::can_transition_to`], and guards the write with a
//! version check so concurrent updates fail loudly rather than clobber.

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_identifiers::TaskId;
use systemprompt_traits::RepositoryError;

use super::mutations::task_state_to_db_string;
use crate::models::a2a::TaskState;

pub async fn update_task_state(
    pool: &Arc<PgPool>,
    task_id: &TaskId,
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

pub async fn apply_notification_status(
    pool: &Arc<PgPool>,
    task_id: &TaskId,
    state: &str,
    timestamp: &chrono::DateTime<chrono::Utc>,
) -> Result<(), RepositoryError> {
    if state == "completed" {
        sqlx::query!(
            r#"UPDATE agent_tasks SET
                status = 'completed',
                updated_at = $1,
                completed_at = CURRENT_TIMESTAMP,
                started_at = COALESCE(started_at, CURRENT_TIMESTAMP),
                execution_time_ms = EXTRACT(EPOCH FROM (CURRENT_TIMESTAMP - COALESCE(started_at, CURRENT_TIMESTAMP))) * 1000
            WHERE task_id = $2"#,
            timestamp,
            task_id.as_str(),
        )
        .execute(pool.as_ref())
        .await
        .map_err(RepositoryError::database)?;
    } else {
        sqlx::query!(
            "UPDATE agent_tasks SET status = $1, updated_at = $2 WHERE task_id = $3",
            state,
            timestamp,
            task_id.as_str(),
        )
        .execute(pool.as_ref())
        .await
        .map_err(RepositoryError::database)?;
    }
    Ok(())
}

pub async fn update_task_failed_with_error(
    pool: &Arc<PgPool>,
    task_id: &TaskId,
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
