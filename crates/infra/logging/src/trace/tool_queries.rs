use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;

use super::models::ToolExecutionItem;

struct DbRow {
    timestamp: DateTime<Utc>,
    trace_id: String,
    tool_name: String,
    server_name: Option<String>,
    status: String,
    execution_time_ms: Option<i32>,
}

pub async fn list_tool_executions(
    pool: &Arc<PgPool>,
    since: Option<DateTime<Utc>>,
    name: Option<&str>,
    server: Option<&str>,
    status: Option<&str>,
    limit: i64,
) -> Result<Vec<ToolExecutionItem>> {
    let rows = sqlx::query_as!(
        DbRow,
        r#"
        SELECT
            started_at as "timestamp!",
            trace_id as "trace_id!",
            tool_name as "tool_name!",
            server_name,
            status as "status!",
            execution_time_ms
        FROM mcp_tool_executions
        WHERE ($1::timestamptz IS NULL OR started_at >= $1)
          AND ($2::text IS NULL OR tool_name ILIKE $2)
          AND ($3::text IS NULL OR server_name ILIKE $3)
          AND ($4::text IS NULL OR status = $4)
        ORDER BY started_at DESC
        LIMIT $5
        "#,
        since,
        name,
        server,
        status,
        limit
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ToolExecutionItem {
            timestamp: r.timestamp,
            trace_id: r.trace_id,
            tool_name: r.tool_name,
            server_name: r.server_name,
            status: r.status,
            execution_time_ms: r.execution_time_ms,
        })
        .collect())
}
