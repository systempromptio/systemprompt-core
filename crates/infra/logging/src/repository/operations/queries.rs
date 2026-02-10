#![allow(clippy::print_stdout)]

use anyhow::Context;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_identifiers::{ClientId, ContextId, LogId, SessionId, TaskId, TraceId, UserId};

use crate::models::{LogEntry, LogFilter, LogLevel, LoggingError};

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

pub async fn get_log(pool: &PgPool, id: &LogId) -> Result<Option<LogEntry>, LoggingError> {
    let id_str = id.as_str();

    let row = sqlx::query_as!(
        LogRow,
        r#"
        SELECT
            id as "id!", timestamp as "timestamp!", level as "level!", module as "module!",
            message as "message!", metadata, user_id as "user_id!", session_id as "session_id!",
            task_id, trace_id as "trace_id!", context_id, client_id
        FROM logs WHERE id = $1
        "#,
        id_str
    )
    .fetch_optional(pool)
    .await
    .context("Failed to get log by id")?;

    Ok(row.map(row_to_entry))
}

pub async fn list_logs(pool: &PgPool, limit: i64) -> Result<Vec<LogEntry>, LoggingError> {
    let rows = sqlx::query_as!(
        LogRow,
        r#"
        SELECT
            id as "id!", timestamp as "timestamp!", level as "level!", module as "module!",
            message as "message!", metadata, user_id as "user_id!", session_id as "session_id!",
            task_id, trace_id as "trace_id!", context_id, client_id
        FROM logs ORDER BY timestamp DESC LIMIT $1
        "#,
        limit
    )
    .fetch_all(pool)
    .await
    .context("Failed to list logs")?;

    Ok(rows.into_iter().map(row_to_entry).collect())
}

pub async fn list_logs_paginated(
    pool: &PgPool,
    filter: &LogFilter,
) -> Result<(Vec<LogEntry>, i64), LoggingError> {
    let (offset, per_page) = calculate_pagination(filter);
    let level_filter = filter.level();
    let module_filter = filter.module();
    let message_pattern = filter.message().map(|m| format!("%{m}%"));

    let rows = sqlx::query_as!(
        LogRow,
        r#"
        SELECT
            id as "id!", timestamp as "timestamp!", level as "level!", module as "module!",
            message as "message!", metadata, user_id as "user_id!", session_id as "session_id!",
            task_id, trace_id as "trace_id!", context_id, client_id
        FROM logs
        WHERE ($1::VARCHAR IS NULL OR level = $1)
        AND ($2::VARCHAR IS NULL OR module = $2)
        AND ($3::VARCHAR IS NULL OR message LIKE $3)
        ORDER BY timestamp DESC LIMIT $4 OFFSET $5
        "#,
        level_filter,
        module_filter,
        message_pattern,
        per_page,
        offset
    )
    .fetch_all(pool)
    .await
    .context("Failed to get paginated logs")?;

    let count = fetch_filtered_count(
        pool,
        level_filter.map(ToString::to_string),
        module_filter.map(ToString::to_string),
        message_pattern,
    )
    .await?;
    let entries = rows.into_iter().map(row_to_entry).collect();

    Ok((entries, count))
}

fn calculate_pagination(filter: &LogFilter) -> (i64, i64) {
    let offset = i64::from(
        filter
            .page()
            .saturating_sub(1)
            .saturating_mul(filter.per_page()),
    );
    let per_page = i64::from(filter.per_page());
    (offset, per_page)
}

async fn fetch_filtered_count(
    pool: &PgPool,
    level: Option<String>,
    module: Option<String>,
    message_pattern: Option<String>,
) -> Result<i64, LoggingError> {
    sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!" FROM logs
        WHERE ($1::VARCHAR IS NULL OR level = $1)
        AND ($2::VARCHAR IS NULL OR module = $2)
        AND ($3::VARCHAR IS NULL OR message LIKE $3)
        "#,
        level,
        module,
        message_pattern
    )
    .fetch_one(pool)
    .await
    .context("Failed to count logs")
    .map_err(Into::into)
}

pub async fn list_logs_by_module_patterns(
    pool: &PgPool,
    patterns: &[String],
    limit: i64,
) -> Result<Vec<LogEntry>, LoggingError> {
    let rows = sqlx::query_as!(
        LogRow,
        r#"
        SELECT
            id as "id!", timestamp as "timestamp!", level as "level!", module as "module!",
            message as "message!", metadata, user_id as "user_id!", session_id as "session_id!",
            task_id, trace_id as "trace_id!", context_id, client_id
        FROM logs
        WHERE module LIKE ANY($1)
        ORDER BY timestamp DESC LIMIT $2
        "#,
        patterns,
        limit
    )
    .fetch_all(pool)
    .await
    .context("Failed to list logs by module patterns")?;

    Ok(rows.into_iter().map(row_to_entry).collect())
}
