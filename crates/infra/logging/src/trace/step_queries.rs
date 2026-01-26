use anyhow::Result;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;

use super::models::{ExecutionStepSummary, McpExecutionSummary, TraceEvent};

pub async fn fetch_mcp_execution_summary(
    pool: &Arc<PgPool>,
    trace_id: &str,
) -> Result<McpExecutionSummary> {
    let row = sqlx::query!(
        r#"
        SELECT
            COUNT(*)::bigint as execution_count,
            COALESCE(SUM(execution_time_ms), 0)::bigint as total_execution_time_ms
        FROM mcp_tool_executions
        WHERE trace_id = $1
        "#,
        trace_id
    )
    .fetch_one(&**pool)
    .await?;

    Ok(McpExecutionSummary {
        execution_count: row.execution_count.unwrap_or(0),
        total_execution_time_ms: row.total_execution_time_ms.unwrap_or(0),
    })
}

pub async fn fetch_mcp_execution_events(
    pool: &Arc<PgPool>,
    trace_id: &str,
) -> Result<Vec<TraceEvent>> {
    let rows = sqlx::query!(
        r#"
        SELECT
            started_at as timestamp,
            tool_name,
            server_name,
            execution_time_ms,
            status,
            error_message,
            user_id,
            session_id,
            task_id,
            context_id
        FROM mcp_tool_executions
        WHERE trace_id = $1
        ORDER BY started_at ASC
        "#,
        trace_id
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let base_details = format!(
                "{}/{}: {} ({}ms)",
                row.server_name,
                row.tool_name,
                row.status,
                row.execution_time_ms.unwrap_or(0)
            );

            let details = if row.status == "failed" {
                if let Some(ref error) = row.error_message {
                    let truncated_error = if error.len() > 80 {
                        format!("{}...", &error[..80])
                    } else {
                        error.clone()
                    };
                    format!("{} | {}", base_details, truncated_error)
                } else {
                    base_details
                }
            } else {
                base_details
            };

            let metadata = json!({
                "execution_time_ms": row.execution_time_ms,
                "tool_name": row.tool_name,
                "server_name": row.server_name,
                "error_message": row.error_message
            });

            TraceEvent {
                event_type: "MCP".to_string(),
                timestamp: row.timestamp,
                details,
                user_id: Some(row.user_id),
                session_id: row.session_id,
                task_id: row.task_id,
                context_id: row.context_id,
                metadata: Some(metadata.to_string()),
            }
        })
        .collect())
}

pub async fn fetch_task_id_for_trace(pool: &Arc<PgPool>, trace_id: &str) -> Result<Option<String>> {
    let row = sqlx::query!(
        "SELECT task_id FROM agent_tasks WHERE trace_id = $1 LIMIT 1",
        trace_id
    )
    .fetch_optional(&**pool)
    .await?;

    Ok(row.map(|r| r.task_id))
}

pub async fn fetch_execution_step_summary(
    pool: &Arc<PgPool>,
    trace_id: &str,
) -> Result<ExecutionStepSummary> {
    let row = sqlx::query!(
        r#"
        SELECT
            COUNT(*)::bigint as step_count,
            COUNT(*) FILTER (WHERE s.status = 'completed')::bigint as completed_count,
            COUNT(*) FILTER (WHERE s.status = 'failed')::bigint as failed_count,
            COUNT(*) FILTER (WHERE s.status = 'pending' OR s.status = 'in_progress')::bigint as pending_count
        FROM task_execution_steps s
        JOIN agent_tasks t ON s.task_id = t.task_id
        WHERE t.trace_id = $1
        "#,
        trace_id
    )
    .fetch_one(&**pool)
    .await?;

    Ok(ExecutionStepSummary {
        total: row.step_count.unwrap_or(0),
        completed: row.completed_count.unwrap_or(0),
        failed: row.failed_count.unwrap_or(0),
        pending: row.pending_count.unwrap_or(0),
    })
}

pub async fn fetch_execution_step_events(
    pool: &Arc<PgPool>,
    trace_id: &str,
) -> Result<Vec<TraceEvent>> {
    let rows = sqlx::query!(
        r#"
        SELECT
            s.started_at as timestamp,
            s.content,
            s.status,
            s.duration_ms,
            t.user_id,
            t.session_id,
            t.task_id,
            t.context_id
        FROM task_execution_steps s
        JOIN agent_tasks t ON s.task_id = t.task_id
        WHERE t.trace_id = $1
        ORDER BY s.started_at ASC
        "#,
        trace_id
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let content = row.content.clone();
            let step_type = content
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let tool_name = content.get("tool_name").and_then(|v| v.as_str());
            let skill_name = content.get("skill_name").and_then(|v| v.as_str());

            let details = match step_type {
                "understanding" => format!("[{}] Analyzing request... - {}", step_type, row.status),
                "planning" => format!("[{}] Planning response... - {}", step_type, row.status),
                "skill_usage" => {
                    let name = skill_name.unwrap_or("unknown");
                    format!("[{}] Using {} skill... - {}", step_type, name, row.status)
                },
                "tool_execution" => {
                    let name = tool_name.unwrap_or("unknown");
                    format!("[{}] Running {}... - {}", step_type, name, row.status)
                },
                "completion" => format!("[{}] Complete - {}", step_type, row.status),
                _ => format!("[{}] - {}", step_type, row.status),
            };

            let metadata = json!({
                "step_type": step_type,
                "status": row.status,
                "duration_ms": row.duration_ms,
                "tool_name": tool_name,
                "skill_name": skill_name
            });

            TraceEvent {
                event_type: "STEP".to_string(),
                timestamp: row.timestamp,
                details,
                user_id: row.user_id,
                session_id: row.session_id,
                task_id: Some(row.task_id),
                context_id: Some(row.context_id),
                metadata: Some(metadata.to_string()),
            }
        })
        .collect())
}
