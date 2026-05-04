//! Aggregate tool-usage statistics queries.

use sqlx::PgPool;

use crate::error::McpDomainResult;
use crate::models::{ExecutionStatus, ToolStats};

pub(super) async fn list_tool_stats(pool: &PgPool, limit: i64) -> McpDomainResult<Vec<ToolStats>> {
    let success_status = ExecutionStatus::Success.as_str();
    let failed_status = ExecutionStatus::Failed.as_str();
    let rows = sqlx::query!(
        r#"SELECT
            tool_name as "tool_name!",
            server_name as "server_name!",
            COUNT(*)::bigint as "total_executions!",
            COUNT(*) FILTER (WHERE status = $1)::bigint as "success_count!",
            COUNT(*) FILTER (WHERE status = $2)::bigint as "error_count!",
            AVG(execution_time_ms)::bigint as avg_duration_ms,
            MIN(execution_time_ms)::bigint as min_duration_ms,
            MAX(execution_time_ms)::bigint as max_duration_ms
        FROM mcp_tool_executions
        GROUP BY tool_name, server_name
        ORDER BY COUNT(*) DESC
        LIMIT $3"#,
        success_status,
        failed_status,
        limit
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ToolStats {
            tool_name: r.tool_name,
            server_name: r.server_name,
            total_executions: r.total_executions,
            success_count: r.success_count,
            error_count: r.error_count,
            avg_duration_ms: r.avg_duration_ms,
            min_duration_ms: r.min_duration_ms,
            max_duration_ms: r.max_duration_ms,
        })
        .collect())
}
