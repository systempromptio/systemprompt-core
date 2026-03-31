use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;

use systemprompt_identifiers::LogId;

use super::models::{LogSearchItem, ToolExecutionItem};

struct LogRow {
    id: String,
    trace_id: String,
    timestamp: DateTime<Utc>,
    level: String,
    module: String,
    message: String,
    metadata: Option<String>,
}

struct ToolRow {
    timestamp: DateTime<Utc>,
    trace_id: String,
    tool_name: String,
    server_name: Option<String>,
    status: String,
    execution_time_ms: Option<i32>,
}

pub async fn search_logs(
    pool: &Arc<PgPool>,
    pattern: &str,
    since: Option<DateTime<Utc>>,
    level: Option<&str>,
    limit: i64,
) -> Result<Vec<LogSearchItem>> {
    let rows = sqlx::query_as!(
        LogRow,
        r#"
        SELECT
            id as "id!",
            trace_id as "trace_id!",
            timestamp as "timestamp!",
            level as "level!",
            module as "module!",
            message as "message!",
            metadata
        FROM logs
        WHERE message ILIKE $1
          AND ($2::timestamptz IS NULL OR timestamp >= $2)
          AND ($3::text IS NULL OR UPPER(level) = $3)
        ORDER BY timestamp DESC
        LIMIT $4
        "#,
        pattern,
        since,
        level,
        limit
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| LogSearchItem {
            id: LogId::new(r.id),
            trace_id: r.trace_id.into(),
            timestamp: r.timestamp,
            level: r.level,
            module: r.module,
            message: r.message,
            metadata: r.metadata,
        })
        .collect())
}

pub async fn search_tool_executions(
    pool: &Arc<PgPool>,
    pattern: &str,
    since: Option<DateTime<Utc>>,
    limit: i64,
) -> Result<Vec<ToolExecutionItem>> {
    let rows = sqlx::query_as!(
        ToolRow,
        r#"
        SELECT
            started_at as "timestamp!",
            trace_id as "trace_id!",
            tool_name as "tool_name!",
            server_name,
            status as "status!",
            execution_time_ms
        FROM mcp_tool_executions
        WHERE (tool_name ILIKE $1 OR server_name ILIKE $1)
          AND ($2::timestamptz IS NULL OR started_at >= $2)
        ORDER BY started_at DESC
        LIMIT $3
        "#,
        pattern,
        since,
        limit
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ToolExecutionItem {
            timestamp: r.timestamp,
            trace_id: r.trace_id.into(),
            tool_name: r.tool_name,
            server_name: r.server_name,
            status: r.status,
            execution_time_ms: r.execution_time_ms,
        })
        .collect())
}
