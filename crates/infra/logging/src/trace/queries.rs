use anyhow::Result;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;

use super::models::{AiRequestSummary, ExecutionStepSummary, McpExecutionSummary, TraceEvent};

pub async fn fetch_log_events(pool: &Arc<PgPool>, trace_id: &str) -> Result<Vec<TraceEvent>> {
    let rows = sqlx::query!(
        r#"
        SELECT
            timestamp,
            level as type,
            CONCAT(module, ': ', message) as details,
            user_id,
            session_id,
            task_id,
            context_id,
            metadata
        FROM logs
        WHERE trace_id = $1
        ORDER BY timestamp ASC
        "#,
        trace_id
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| TraceEvent {
            event_type: row.r#type,
            timestamp: row.timestamp,
            details: row.details.unwrap_or_default(),
            user_id: row.user_id,
            session_id: row.session_id,
            task_id: row.task_id,
            context_id: row.context_id,
            metadata: row.metadata,
        })
        .collect())
}

pub async fn fetch_ai_request_summary(
    pool: &Arc<PgPool>,
    trace_id: &str,
) -> Result<AiRequestSummary> {
    let row = sqlx::query!(
        r#"
        SELECT
            COALESCE(SUM(cost_cents), 0)::bigint as total_cost_cents,
            COALESCE(SUM(COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)), 0)::bigint as total_tokens,
            COALESCE(SUM(input_tokens), 0)::bigint as total_input_tokens,
            COALESCE(SUM(output_tokens), 0)::bigint as total_output_tokens,
            COUNT(*)::bigint as request_count,
            COALESCE(SUM(latency_ms), 0)::bigint as total_latency_ms
        FROM ai_requests
        WHERE trace_id = $1
        "#,
        trace_id
    )
    .fetch_one(&**pool)
    .await?;

    Ok(AiRequestSummary {
        total_cost_cents: row.total_cost_cents.unwrap_or(0),
        total_tokens: row.total_tokens.unwrap_or(0),
        total_input_tokens: row.total_input_tokens.unwrap_or(0),
        total_output_tokens: row.total_output_tokens.unwrap_or(0),
        request_count: row.request_count.unwrap_or(0),
        total_latency_ms: row.total_latency_ms.unwrap_or(0),
    })
}

pub async fn fetch_ai_request_events(
    pool: &Arc<PgPool>,
    trace_id: &str,
) -> Result<Vec<TraceEvent>> {
    let rows = sqlx::query!(
        r#"
        SELECT
            created_at as timestamp,
            provider,
            model,
            input_tokens,
            output_tokens,
            cost_cents,
            latency_ms,
            status,
            user_id,
            session_id,
            task_id,
            context_id
        FROM ai_requests
        WHERE trace_id = $1
        ORDER BY created_at ASC
        "#,
        trace_id
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let details = format!(
                "{}/{}: {} (in:{}, out:{}, {}ms)",
                row.provider,
                row.model,
                row.status,
                row.input_tokens.unwrap_or(0),
                row.output_tokens.unwrap_or(0),
                row.latency_ms.unwrap_or(0)
            );

            let metadata = json!({
                "cost_cents": row.cost_cents,
                "latency_ms": row.latency_ms,
                "input_tokens": row.input_tokens,
                "output_tokens": row.output_tokens,
                "tokens_used": row.input_tokens.unwrap_or(0) + row.output_tokens.unwrap_or(0),
                "provider": row.provider,
                "model": row.model
            });

            TraceEvent {
                event_type: "AI".to_string(),
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
            let details = format!(
                "{}/{}: {} ({}ms)",
                row.server_name,
                row.tool_name,
                row.status,
                row.execution_time_ms.unwrap_or(0)
            );

            let metadata = json!({
                "execution_time_ms": row.execution_time_ms,
                "tool_name": row.tool_name,
                "server_name": row.server_name
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
