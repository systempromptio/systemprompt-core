//! Paginated, filtered log-row queries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_identifiers::{
    ClientId, ContextId, InstanceId, LogId, SessionId, TaskId, TraceId, UserId,
};

use systemprompt_traits::RepositoryError;

use crate::models::{LogEntry, LogFilter, LogLevel, LoggingError};

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
    context_id_text: Option<String>,
    client_id: Option<ClientId>,
    instance_id: Option<InstanceId>,
}

fn row_to_entry(r: LogRow) -> Result<LogEntry, LoggingError> {
    let level = r
        .level
        .parse::<LogLevel>()
        .map_err(|e| RepositoryError::decode(format!("log {} level", r.id), e))?;
    let metadata = r
        .metadata
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|e| RepositoryError::decode(format!("log {} metadata", r.id), e))?;
    let context_id = r
        .context_id_text
        .as_deref()
        .map(ContextId::try_new)
        .transpose()
        .map_err(|e| RepositoryError::decode(format!("log {} context_id", r.id), e))?;
    Ok(LogEntry {
        id: r.id,
        timestamp: r.timestamp,
        level,
        module: r.module,
        message: r.message,
        metadata,
        user_id: r.user_id,
        session_id: r.session_id,
        task_id: r.task_id,
        trace_id: r.trace_id,
        context_id,
        client_id: r.client_id,
        instance_id: r.instance_id,
    })
}

pub(in crate::repository) async fn get_log(
    pool: &PgPool,
    id: &LogId,
) -> Result<Option<LogEntry>, LoggingError> {
    let id_str = id.as_str();

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
            client_id as "client_id: ClientId",
            instance_id as "instance_id: InstanceId"
        FROM logs WHERE id = $1
        "#,
        id_str
    )
    .fetch_optional(pool)
    .await?;

    row.map(row_to_entry).transpose()
}

pub(in crate::repository) async fn list_logs(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<LogEntry>, LoggingError> {
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
            client_id as "client_id: ClientId",
            instance_id as "instance_id: InstanceId"
        FROM logs ORDER BY timestamp DESC LIMIT $1
        "#,
        limit
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(row_to_entry).collect()
}

pub(in crate::repository) async fn list_logs_paginated(
    pool: &PgPool,
    filter: &LogFilter,
) -> Result<(Vec<LogEntry>, i64), LoggingError> {
    let (offset, per_page) = calculate_pagination(filter);
    let level_filter = filter.level();
    let module_filter = filter.module();
    let message_pattern = filter.message().map(|m| format!("%{m}%"));
    let since_filter = filter.since();

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
            client_id as "client_id: ClientId",
            instance_id as "instance_id: InstanceId"
        FROM logs
        WHERE ($1::VARCHAR IS NULL OR level = $1)
        AND ($2::VARCHAR IS NULL OR module = $2)
        AND ($3::VARCHAR IS NULL OR message LIKE $3)
        AND ($4::TIMESTAMPTZ IS NULL OR timestamp >= $4)
        ORDER BY timestamp DESC LIMIT $5 OFFSET $6
        "#,
        level_filter,
        module_filter,
        message_pattern,
        since_filter,
        per_page,
        offset
    )
    .fetch_all(pool)
    .await?;

    let count = fetch_filtered_count(
        pool,
        level_filter.map(str::to_owned),
        module_filter.map(str::to_owned),
        message_pattern,
        since_filter,
    )
    .await?;
    let entries = rows
        .into_iter()
        .map(row_to_entry)
        .collect::<Result<Vec<_>, _>>()?;

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
    since: Option<DateTime<Utc>>,
) -> Result<i64, LoggingError> {
    sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!" FROM logs
        WHERE ($1::VARCHAR IS NULL OR level = $1)
        AND ($2::VARCHAR IS NULL OR module = $2)
        AND ($3::VARCHAR IS NULL OR message LIKE $3)
        AND ($4::TIMESTAMPTZ IS NULL OR timestamp >= $4)
        "#,
        level,
        module,
        message_pattern,
        since
    )
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

pub(in crate::repository) async fn list_logs_by_module_patterns(
    pool: &PgPool,
    patterns: &[String],
    limit: i64,
) -> Result<Vec<LogEntry>, LoggingError> {
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
            client_id as "client_id: ClientId",
            instance_id as "instance_id: InstanceId"
        FROM logs
        WHERE module LIKE ANY($1)
        ORDER BY timestamp DESC LIMIT $2
        "#,
        patterns,
        limit
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(row_to_entry).collect()
}
