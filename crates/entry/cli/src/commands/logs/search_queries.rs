use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;

pub struct SearchRow {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub module: String,
    pub message: String,
    pub metadata: Option<String>,
}

pub struct ToolExecutionRow {
    pub timestamp: DateTime<Utc>,
    pub trace_id: String,
    pub tool_name: String,
    pub server_name: Option<String>,
    pub status: String,
    pub execution_time_ms: Option<i32>,
}

pub async fn search_logs(
    pool: &Arc<PgPool>,
    pattern: &str,
    since: Option<DateTime<Utc>>,
    level: Option<&str>,
    limit: i64,
) -> Result<Vec<SearchRow>> {
    let rows = match (since, level) {
        (Some(since_ts), Some(lvl)) => {
            sqlx::query_as!(
                SearchRow,
                r#"
                SELECT timestamp as "timestamp!", level as "level!", module as "module!",
                       message as "message!", metadata
                FROM logs
                WHERE message ILIKE $1 AND timestamp >= $2 AND UPPER(level) = $3
                ORDER BY timestamp DESC LIMIT $4
                "#,
                pattern,
                since_ts,
                lvl,
                limit
            )
            .fetch_all(pool.as_ref())
            .await?
        },
        (Some(since_ts), None) => {
            sqlx::query_as!(
                SearchRow,
                r#"
                SELECT timestamp as "timestamp!", level as "level!", module as "module!",
                       message as "message!", metadata
                FROM logs
                WHERE message ILIKE $1 AND timestamp >= $2
                ORDER BY timestamp DESC LIMIT $3
                "#,
                pattern,
                since_ts,
                limit
            )
            .fetch_all(pool.as_ref())
            .await?
        },
        (None, Some(lvl)) => {
            sqlx::query_as!(
                SearchRow,
                r#"
                SELECT timestamp as "timestamp!", level as "level!", module as "module!",
                       message as "message!", metadata
                FROM logs
                WHERE message ILIKE $1 AND UPPER(level) = $2
                ORDER BY timestamp DESC LIMIT $3
                "#,
                pattern,
                lvl,
                limit
            )
            .fetch_all(pool.as_ref())
            .await?
        },
        (None, None) => {
            sqlx::query_as!(
                SearchRow,
                r#"
                SELECT timestamp as "timestamp!", level as "level!", module as "module!",
                       message as "message!", metadata
                FROM logs
                WHERE message ILIKE $1
                ORDER BY timestamp DESC LIMIT $2
                "#,
                pattern,
                limit
            )
            .fetch_all(pool.as_ref())
            .await?
        },
    };
    Ok(rows)
}

pub async fn search_tools(
    pool: &Arc<PgPool>,
    pattern: &str,
    since: Option<DateTime<Utc>>,
    limit: i64,
) -> Result<Vec<ToolExecutionRow>> {
    let rows = match since {
        Some(since_ts) => {
            sqlx::query_as!(
                ToolExecutionRow,
                r#"
                SELECT started_at as "timestamp!", trace_id as "trace_id!",
                       tool_name as "tool_name!", server_name, status as "status!",
                       execution_time_ms
                FROM mcp_tool_executions
                WHERE (tool_name ILIKE $1 OR server_name ILIKE $1) AND started_at >= $2
                ORDER BY started_at DESC LIMIT $3
                "#,
                pattern,
                since_ts,
                limit
            )
            .fetch_all(pool.as_ref())
            .await?
        },
        None => {
            sqlx::query_as!(
                ToolExecutionRow,
                r#"
                SELECT started_at as "timestamp!", trace_id as "trace_id!",
                       tool_name as "tool_name!", server_name, status as "status!",
                       execution_time_ms
                FROM mcp_tool_executions
                WHERE tool_name ILIKE $1 OR server_name ILIKE $1
                ORDER BY started_at DESC LIMIT $2
                "#,
                pattern,
                limit
            )
            .fetch_all(pool.as_ref())
            .await?
        },
    };
    Ok(rows)
}
