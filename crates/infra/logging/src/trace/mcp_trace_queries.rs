use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;

use super::models::{AiRequestInfo, McpToolExecution, TaskArtifact, ToolLogEntry};

pub async fn fetch_mcp_executions(
    pool: &Arc<PgPool>,
    task_id: &str,
    context_id: &str,
) -> Result<Vec<McpToolExecution>> {
    let rows = sqlx::query!(
        r#"SELECT mcp_execution_id, tool_name, server_name, status, execution_time_ms,
                  error_message, input, output
           FROM mcp_tool_executions
           WHERE task_id = $1 OR context_id = $2
           ORDER BY started_at"#,
        task_id,
        context_id
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| McpToolExecution {
            mcp_execution_id: r.mcp_execution_id,
            tool_name: r.tool_name,
            server_name: r.server_name,
            status: r.status,
            execution_time_ms: r.execution_time_ms,
            error_message: r.error_message,
            input: r.input,
            output: r.output,
        })
        .collect())
}

pub async fn fetch_mcp_linked_ai_requests(
    pool: &Arc<PgPool>,
    mcp_execution_id: &str,
) -> Result<Vec<AiRequestInfo>> {
    let rows = sqlx::query!(
        r#"SELECT id, model, provider, max_tokens, input_tokens, output_tokens, cost_cents, latency_ms
           FROM ai_requests
           WHERE mcp_execution_id = $1
           ORDER BY created_at"#,
        mcp_execution_id
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| AiRequestInfo {
            id: r.id,
            provider: r.provider,
            model: r.model,
            max_tokens: r.max_tokens,
            input_tokens: r.input_tokens,
            output_tokens: r.output_tokens,
            cost_cents: r.cost_cents,
            latency_ms: r.latency_ms,
        })
        .collect())
}

pub async fn fetch_tool_logs(
    pool: &Arc<PgPool>,
    task_id: &str,
    context_id: &str,
) -> Result<Vec<ToolLogEntry>> {
    let rows = sqlx::query!(
        r#"SELECT timestamp, level, module, message
           FROM logs
           WHERE (task_id = $1 OR context_id = $2)
             AND (
                 (module LIKE '%_tools' OR module LIKE '%_manager' OR module LIKE 'create_%' OR module LIKE 'update_%' OR module LIKE 'research_%')
                 OR (level = 'ERROR' AND message LIKE '%tool%')
                 OR message LIKE 'Tool executed%'
                 OR message LIKE 'Tool failed%'
                 OR message LIKE 'MCP execution%'
             )
           ORDER BY timestamp"#,
        task_id,
        context_id
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ToolLogEntry {
            timestamp: r.timestamp,
            level: r.level,
            module: r.module,
            message: r.message,
        })
        .collect())
}

pub async fn fetch_task_artifacts(
    pool: &Arc<PgPool>,
    task_id: &str,
    context_id: &str,
) -> Result<Vec<TaskArtifact>> {
    let rows = sqlx::query!(
        r#"SELECT ta.artifact_id, ta.artifact_type, ta.name, ta.source, ta.tool_name,
                  ap.part_kind as "part_kind?", ap.text_content as "text_content?",
                  ap.data_content as "data_content?"
           FROM task_artifacts ta
           LEFT JOIN artifact_parts ap ON ta.artifact_id = ap.artifact_id AND ta.context_id = ap.context_id
           WHERE ta.task_id = $1 OR ta.context_id = $2
           ORDER BY ta.created_at, ap.sequence_number"#,
        task_id,
        context_id
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| TaskArtifact {
            artifact_id: r.artifact_id,
            artifact_type: r.artifact_type,
            name: r.name,
            source: r.source,
            tool_name: r.tool_name,
            part_kind: r.part_kind,
            text_content: r.text_content,
            data_content: r.data_content,
        })
        .collect())
}
