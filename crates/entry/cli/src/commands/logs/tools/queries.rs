use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;

pub struct DbToolRow {
    pub timestamp: DateTime<Utc>,
    pub trace_id: String,
    pub tool_name: String,
    pub server_name: Option<String>,
    pub status: String,
    pub execution_time_ms: Option<i32>,
}

pub async fn query_tools(
    pool: &Arc<PgPool>,
    since: Option<DateTime<Utc>>,
    name: Option<&str>,
    server: Option<&str>,
    status: Option<&str>,
    limit: i64,
) -> Result<Vec<DbToolRow>> {
    let rows = match (since, name, server, status) {
        (Some(s), Some(n), Some(sv), Some(st)) => {
            query_all_filters(pool, s, n, sv, st, limit).await?
        },
        (Some(s), Some(n), None, None) => query_since_name(pool, s, n, limit).await?,
        (Some(s), None, Some(sv), None) => query_since_server(pool, s, sv, limit).await?,
        (Some(s), None, None, Some(st)) => query_since_status(pool, s, st, limit).await?,
        (Some(s), None, None, None) => query_since_only(pool, s, limit).await?,
        (None, Some(n), None, None) => query_name_only(pool, n, limit).await?,
        (None, None, Some(sv), None) => query_server_only(pool, sv, limit).await?,
        (None, None, None, Some(st)) => query_status_only(pool, st, limit).await?,
        _ => query_no_filters(pool, limit).await?,
    };
    Ok(rows)
}

async fn query_all_filters(
    pool: &Arc<PgPool>,
    since: DateTime<Utc>,
    name: &str,
    server: &str,
    status: &str,
    limit: i64,
) -> Result<Vec<DbToolRow>> {
    Ok(sqlx::query_as!(
        DbToolRow,
        r#"SELECT started_at as "timestamp!", trace_id as "trace_id!", tool_name as "tool_name!",
           server_name, status as "status!", execution_time_ms
           FROM mcp_tool_executions
           WHERE started_at >= $1 AND tool_name ILIKE $2 AND server_name ILIKE $3 AND status = $4
           ORDER BY started_at DESC LIMIT $5"#,
        since,
        name,
        server,
        status,
        limit
    )
    .fetch_all(pool.as_ref())
    .await?)
}

async fn query_since_name(
    pool: &Arc<PgPool>,
    since: DateTime<Utc>,
    name: &str,
    limit: i64,
) -> Result<Vec<DbToolRow>> {
    Ok(sqlx::query_as!(
        DbToolRow,
        r#"SELECT started_at as "timestamp!", trace_id as "trace_id!", tool_name as "tool_name!",
           server_name, status as "status!", execution_time_ms
           FROM mcp_tool_executions WHERE started_at >= $1 AND tool_name ILIKE $2
           ORDER BY started_at DESC LIMIT $3"#,
        since,
        name,
        limit
    )
    .fetch_all(pool.as_ref())
    .await?)
}

async fn query_since_server(
    pool: &Arc<PgPool>,
    since: DateTime<Utc>,
    server: &str,
    limit: i64,
) -> Result<Vec<DbToolRow>> {
    Ok(sqlx::query_as!(
        DbToolRow,
        r#"SELECT started_at as "timestamp!", trace_id as "trace_id!", tool_name as "tool_name!",
           server_name, status as "status!", execution_time_ms
           FROM mcp_tool_executions WHERE started_at >= $1 AND server_name ILIKE $2
           ORDER BY started_at DESC LIMIT $3"#,
        since,
        server,
        limit
    )
    .fetch_all(pool.as_ref())
    .await?)
}

async fn query_since_status(
    pool: &Arc<PgPool>,
    since: DateTime<Utc>,
    status: &str,
    limit: i64,
) -> Result<Vec<DbToolRow>> {
    Ok(sqlx::query_as!(
        DbToolRow,
        r#"SELECT started_at as "timestamp!", trace_id as "trace_id!", tool_name as "tool_name!",
           server_name, status as "status!", execution_time_ms
           FROM mcp_tool_executions WHERE started_at >= $1 AND status = $2
           ORDER BY started_at DESC LIMIT $3"#,
        since,
        status,
        limit
    )
    .fetch_all(pool.as_ref())
    .await?)
}

async fn query_since_only(
    pool: &Arc<PgPool>,
    since: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<DbToolRow>> {
    Ok(sqlx::query_as!(
        DbToolRow,
        r#"SELECT started_at as "timestamp!", trace_id as "trace_id!", tool_name as "tool_name!",
           server_name, status as "status!", execution_time_ms
           FROM mcp_tool_executions WHERE started_at >= $1
           ORDER BY started_at DESC LIMIT $2"#,
        since,
        limit
    )
    .fetch_all(pool.as_ref())
    .await?)
}

async fn query_name_only(pool: &Arc<PgPool>, name: &str, limit: i64) -> Result<Vec<DbToolRow>> {
    Ok(sqlx::query_as!(
        DbToolRow,
        r#"SELECT started_at as "timestamp!", trace_id as "trace_id!", tool_name as "tool_name!",
           server_name, status as "status!", execution_time_ms
           FROM mcp_tool_executions WHERE tool_name ILIKE $1
           ORDER BY started_at DESC LIMIT $2"#,
        name,
        limit
    )
    .fetch_all(pool.as_ref())
    .await?)
}

async fn query_server_only(pool: &Arc<PgPool>, server: &str, limit: i64) -> Result<Vec<DbToolRow>> {
    Ok(sqlx::query_as!(
        DbToolRow,
        r#"SELECT started_at as "timestamp!", trace_id as "trace_id!", tool_name as "tool_name!",
           server_name, status as "status!", execution_time_ms
           FROM mcp_tool_executions WHERE server_name ILIKE $1
           ORDER BY started_at DESC LIMIT $2"#,
        server,
        limit
    )
    .fetch_all(pool.as_ref())
    .await?)
}

async fn query_status_only(pool: &Arc<PgPool>, status: &str, limit: i64) -> Result<Vec<DbToolRow>> {
    Ok(sqlx::query_as!(
        DbToolRow,
        r#"SELECT started_at as "timestamp!", trace_id as "trace_id!", tool_name as "tool_name!",
           server_name, status as "status!", execution_time_ms
           FROM mcp_tool_executions WHERE status = $1
           ORDER BY started_at DESC LIMIT $2"#,
        status,
        limit
    )
    .fetch_all(pool.as_ref())
    .await?)
}

async fn query_no_filters(pool: &Arc<PgPool>, limit: i64) -> Result<Vec<DbToolRow>> {
    Ok(sqlx::query_as!(
        DbToolRow,
        r#"SELECT started_at as "timestamp!", trace_id as "trace_id!", tool_name as "tool_name!",
           server_name, status as "status!", execution_time_ms
           FROM mcp_tool_executions ORDER BY started_at DESC LIMIT $1"#,
        limit
    )
    .fetch_all(pool.as_ref())
    .await?)
}
