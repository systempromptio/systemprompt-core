#![allow(clippy::print_stdout)]

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::{ClientId, ContextId, LogId, TaskId};

use crate::models::{LogEntry, LoggingError};

pub async fn create_log(db_pool: &DbPool, entry: &LogEntry) -> Result<(), LoggingError> {
    let metadata_json = entry
        .metadata
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .context("Failed to serialize log metadata")?;

    let level_str = entry.level.to_string();
    let pool = db_pool.pool_arc().context("Failed to get database pool")?;

    let user_id = entry.user_id.as_str();
    let session_id = entry.session_id.as_str();
    let task_id = entry.task_id.as_ref().map(TaskId::as_str);
    let trace_id = entry.trace_id.as_str();
    let context_id = entry.context_id.as_ref().map(ContextId::as_str);
    let client_id = entry.client_id.as_ref().map(ClientId::as_str);

    let entry_id = entry.id.as_str();

    sqlx::query!(
        r"
        INSERT INTO logs (id, timestamp, level, module, message, metadata, user_id, session_id, task_id, trace_id, context_id, client_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        ",
        entry_id,
        entry.timestamp,
        level_str,
        entry.module,
        entry.message,
        metadata_json,
        user_id,
        session_id,
        task_id,
        trace_id,
        context_id,
        client_id
    )
    .execute(pool.as_ref())
    .await
    .context("Failed to create log entry")?;

    Ok(())
}

pub async fn update_log(
    db_pool: &DbPool,
    id: &LogId,
    entry: &LogEntry,
) -> Result<bool, LoggingError> {
    let metadata_json = entry
        .metadata
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .context("Failed to serialize log metadata")?;

    let level_str = entry.level.to_string();
    let pool = db_pool.pool_arc().context("Failed to get database pool")?;

    let id_str = id.as_str();

    let result = sqlx::query!(
        r"
        UPDATE logs
        SET level = $1, module = $2, message = $3, metadata = $4
        WHERE id = $5
        ",
        level_str,
        entry.module,
        entry.message,
        metadata_json,
        id_str
    )
    .execute(pool.as_ref())
    .await
    .context("Failed to update log entry")?;

    Ok(result.rows_affected() > 0)
}

pub async fn delete_log(db_pool: &DbPool, id: &LogId) -> Result<bool, LoggingError> {
    let pool = db_pool.pool_arc().context("Failed to get database pool")?;
    let id_str = id.as_str();

    let result = sqlx::query!("DELETE FROM logs WHERE id = $1", id_str)
        .execute(pool.as_ref())
        .await
        .context("Failed to delete log entry")?;

    Ok(result.rows_affected() > 0)
}

pub async fn delete_logs_multiple(db_pool: &DbPool, ids: &[LogId]) -> Result<u64, LoggingError> {
    if ids.is_empty() {
        return Ok(0);
    }

    let pool = db_pool.pool_arc().context("Failed to get database pool")?;
    let id_strs: Vec<String> = ids.iter().map(ToString::to_string).collect();

    let result = sqlx::query!("DELETE FROM logs WHERE id = ANY($1)", &id_strs)
        .execute(pool.as_ref())
        .await
        .context("Failed to delete multiple log entries")?;

    Ok(result.rows_affected())
}

pub async fn clear_all_logs(db_pool: &DbPool) -> Result<u64, LoggingError> {
    let pool = db_pool.pool_arc().context("Failed to get database pool")?;

    let result = sqlx::query!("DELETE FROM logs")
        .execute(pool.as_ref())
        .await
        .context("Failed to clear all logs")?;

    Ok(result.rows_affected())
}

pub async fn cleanup_logs_before(
    db_pool: &DbPool,
    cutoff: DateTime<Utc>,
) -> Result<u64, LoggingError> {
    let pool = db_pool.pool_arc().context("Failed to get database pool")?;

    let result = sqlx::query!("DELETE FROM logs WHERE timestamp < $1", cutoff)
        .execute(pool.as_ref())
        .await
        .context("Failed to cleanup old logs")?;

    Ok(result.rows_affected())
}

pub async fn count_logs_before(
    db_pool: &DbPool,
    cutoff: DateTime<Utc>,
) -> Result<u64, LoggingError> {
    let pool = db_pool.pool_arc().context("Failed to get database pool")?;

    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!" FROM logs WHERE timestamp < $1"#,
        cutoff
    )
    .fetch_one(pool.as_ref())
    .await
    .context("Failed to count logs before cutoff")?;

    Ok(count as u64)
}
