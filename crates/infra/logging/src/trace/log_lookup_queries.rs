//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::models::LoggingError;
pub(super) type Result<T> = std::result::Result<T, LoggingError>;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_identifiers::{ClientId, ContextId, LogId, SessionId, TaskId, TraceId, UserId};

use crate::models::{LogEntry, LogLevel};

struct LogRow {
    id: LogId,
    timestamp: DateTime<Utc>,
    level: String,
    module: String,
    message: String,
    metadata: Option<String>,
    user_id: UserId,
    session_id: SessionId,
    task_id: Option<TaskId>,
    trace_id: TraceId,
    // Decoded as raw text and validated in row_to_entry: ContextId requires a
    // UUID, but historical log rows may carry malformed values that must be
    // skipped rather than fail the whole query.
    context_id_text: Option<String>,
    client_id: Option<ClientId>,
}

fn row_to_entry(r: LogRow) -> LogEntry {
    LogEntry {
        id: r.id,
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
        user_id: r.user_id,
        session_id: r.session_id,
        task_id: r.task_id,
        trace_id: r.trace_id,
        context_id: r.context_id_text.and_then(|s| {
            ContextId::try_new(&s)
                .map_err(|e| {
                    tracing::warn!(error = %e, raw = %s, "Skipping non-UUID context_id from log row");
                    e
                })
                .ok()
        }),
        client_id: r.client_id,
    }
}

pub(super) async fn find_log_by_id(pool: &Arc<PgPool>, id: &str) -> Result<Option<LogEntry>> {
    let row = sqlx::query_as!(
        LogRow,
        r#"
        SELECT
            id as "id!: LogId", timestamp as "timestamp!", level as "level!", module as "module!",
            message as "message!", metadata,
            user_id as "user_id!: UserId",
            session_id as "session_id!: SessionId",
            task_id as "task_id: TaskId",
            trace_id as "trace_id!: TraceId",
            context_id as "context_id_text",
            client_id as "client_id: ClientId"
        FROM logs WHERE id = $1
        "#,
        id
    )
    .fetch_optional(&**pool)
    .await?;

    Ok(row.map(row_to_entry))
}

pub(super) async fn find_log_by_partial_id(
    pool: &Arc<PgPool>,
    id_prefix: &str,
) -> Result<Option<LogEntry>> {
    let pattern = format!("{id_prefix}%");
    let row = sqlx::query_as!(
        LogRow,
        r#"
        SELECT
            id as "id!: LogId", timestamp as "timestamp!", level as "level!", module as "module!",
            message as "message!", metadata,
            user_id as "user_id!: UserId",
            session_id as "session_id!: SessionId",
            task_id as "task_id: TaskId",
            trace_id as "trace_id!: TraceId",
            context_id as "context_id_text",
            client_id as "client_id: ClientId"
        FROM logs
        WHERE id LIKE $1
        ORDER BY timestamp DESC
        LIMIT 1
        "#,
        pattern
    )
    .fetch_optional(&**pool)
    .await?;

    Ok(row.map(row_to_entry))
}

pub(super) async fn find_logs_by_trace_id(
    pool: &Arc<PgPool>,
    trace_id: &TraceId,
) -> Result<Vec<LogEntry>> {
    let rows = sqlx::query_as!(
        LogRow,
        r#"
        SELECT
            id as "id!: LogId", timestamp as "timestamp!", level as "level!", module as "module!",
            message as "message!", metadata,
            user_id as "user_id!: UserId",
            session_id as "session_id!: SessionId",
            task_id as "task_id: TaskId",
            trace_id as "trace_id!: TraceId",
            context_id as "context_id_text",
            client_id as "client_id: ClientId"
        FROM logs
        WHERE trace_id = $1
        ORDER BY timestamp ASC
        "#,
        trace_id.as_str()
    )
    .fetch_all(&**pool)
    .await?;

    if !rows.is_empty() {
        return Ok(rows.into_iter().map(row_to_entry).collect());
    }

    let pattern = format!("{}%", trace_id.as_str());
    let rows = sqlx::query_as!(
        LogRow,
        r#"
        SELECT
            id as "id!: LogId", timestamp as "timestamp!", level as "level!", module as "module!",
            message as "message!", metadata,
            user_id as "user_id!: UserId",
            session_id as "session_id!: SessionId",
            task_id as "task_id: TaskId",
            trace_id as "trace_id!: TraceId",
            context_id as "context_id_text",
            client_id as "client_id: ClientId"
        FROM logs
        WHERE trace_id LIKE $1
        ORDER BY timestamp ASC
        LIMIT 100
        "#,
        pattern
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows.into_iter().map(row_to_entry).collect())
}

pub(super) async fn list_logs_filtered(
    pool: &Arc<PgPool>,
    since: Option<DateTime<Utc>>,
    level: Option<&str>,
    limit: i64,
) -> Result<Vec<LogEntry>> {
    let rows = sqlx::query_as!(
        LogRow,
        r#"
        SELECT
            id as "id!: LogId", timestamp as "timestamp!", level as "level!", module as "module!",
            message as "message!", metadata,
            user_id as "user_id!: UserId",
            session_id as "session_id!: SessionId",
            task_id as "task_id: TaskId",
            trace_id as "trace_id!: TraceId",
            context_id as "context_id_text",
            client_id as "client_id: ClientId"
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
    .await?;

    Ok(rows.into_iter().map(row_to_entry).collect())
}
