use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_identifiers::{ClientId, ContextId, LogId, SessionId, TaskId, TraceId, UserId};

use crate::models::{LogEntry, LogLevel};

struct LogRow {
    id: String,
    timestamp: DateTime<Utc>,
    level: String,
    module: String,
    message: String,
    metadata: Option<String>,
    user_id: String,
    session_id: String,
    task_id: Option<String>,
    trace_id: String,
    context_id: Option<String>,
    client_id: Option<String>,
}

fn row_to_entry(r: LogRow) -> LogEntry {
    LogEntry {
        id: LogId::new(r.id),
        timestamp: r.timestamp,
        level: r.level.parse().unwrap_or(LogLevel::Info),
        module: r.module,
        message: r.message,
        metadata: r.metadata.as_ref().and_then(|m| {
            serde_json::from_str(m)
                .map_err(|e| {
                    tracing::warn!(error = %e, raw = %m, "Failed to parse log metadata JSON");
                    e
                })
                .ok()
        }),
        user_id: UserId::new(r.user_id),
        session_id: SessionId::new(r.session_id),
        task_id: r.task_id.map(TaskId::new),
        trace_id: TraceId::new(r.trace_id),
        context_id: r.context_id.map(ContextId::new),
        client_id: r.client_id.map(ClientId::new),
    }
}

pub async fn find_log_by_id(pool: &Arc<PgPool>, id: &str) -> Result<Option<LogEntry>> {
    let row = sqlx::query_as!(
        LogRow,
        r#"
        SELECT
            id as "id!", timestamp as "timestamp!", level as "level!", module as "module!",
            message as "message!", metadata, user_id as "user_id!", session_id as "session_id!",
            task_id, trace_id as "trace_id!", context_id, client_id
        FROM logs WHERE id = $1
        "#,
        id
    )
    .fetch_optional(&**pool)
    .await
    .context("Failed to find log by id")?;

    Ok(row.map(row_to_entry))
}

pub async fn find_log_by_partial_id(
    pool: &Arc<PgPool>,
    id_prefix: &str,
) -> Result<Option<LogEntry>> {
    let pattern = format!("{id_prefix}%");
    let row = sqlx::query_as!(
        LogRow,
        r#"
        SELECT
            id as "id!", timestamp as "timestamp!", level as "level!", module as "module!",
            message as "message!", metadata, user_id as "user_id!", session_id as "session_id!",
            task_id, trace_id as "trace_id!", context_id, client_id
        FROM logs
        WHERE id LIKE $1
        ORDER BY timestamp DESC
        LIMIT 1
        "#,
        pattern
    )
    .fetch_optional(&**pool)
    .await
    .context("Failed to find log by partial id")?;

    Ok(row.map(row_to_entry))
}

pub async fn find_logs_by_trace_id(pool: &Arc<PgPool>, trace_id: &str) -> Result<Vec<LogEntry>> {
    let rows = sqlx::query_as!(
        LogRow,
        r#"
        SELECT
            id as "id!", timestamp as "timestamp!", level as "level!", module as "module!",
            message as "message!", metadata, user_id as "user_id!", session_id as "session_id!",
            task_id, trace_id as "trace_id!", context_id, client_id
        FROM logs
        WHERE trace_id = $1
        ORDER BY timestamp ASC
        "#,
        trace_id
    )
    .fetch_all(&**pool)
    .await
    .context("Failed to find logs by trace id")?;

    if !rows.is_empty() {
        return Ok(rows.into_iter().map(row_to_entry).collect());
    }

    let pattern = format!("{trace_id}%");
    let rows = sqlx::query_as!(
        LogRow,
        r#"
        SELECT
            id as "id!", timestamp as "timestamp!", level as "level!", module as "module!",
            message as "message!", metadata, user_id as "user_id!", session_id as "session_id!",
            task_id, trace_id as "trace_id!", context_id, client_id
        FROM logs
        WHERE trace_id LIKE $1
        ORDER BY timestamp ASC
        LIMIT 100
        "#,
        pattern
    )
    .fetch_all(&**pool)
    .await
    .context("Failed to find logs by partial trace id")?;

    Ok(rows.into_iter().map(row_to_entry).collect())
}

pub async fn list_logs_filtered(
    pool: &Arc<PgPool>,
    since: Option<DateTime<Utc>>,
    level: Option<&str>,
    limit: i64,
) -> Result<Vec<LogEntry>> {
    let rows = sqlx::query_as!(
        LogRow,
        r#"
        SELECT
            id as "id!", timestamp as "timestamp!", level as "level!", module as "module!",
            message as "message!", metadata, user_id as "user_id!", session_id as "session_id!",
            task_id, trace_id as "trace_id!", context_id, client_id
        FROM logs
        WHERE ($1::TIMESTAMPTZ IS NULL OR timestamp >= $1)
          AND ($2::TEXT IS NULL OR UPPER(level) = $2)
        ORDER BY timestamp DESC
        LIMIT $3
        "#,
        since,
        level,
        limit
    )
    .fetch_all(&**pool)
    .await
    .context("Failed to list filtered logs")?;

    Ok(rows.into_iter().map(row_to_entry).collect())
}
