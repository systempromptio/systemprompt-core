use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;

use super::models::{
    AiRequestInfo, ConversationMessage, ExecutionStep, McpToolExecution, TaskArtifact, TaskInfo,
    ToolLogEntry,
};

pub async fn resolve_task_id(pool: &Arc<PgPool>, partial_id: &str) -> Result<Option<String>> {
    let pattern = format!("{}%", partial_id);
    let row = sqlx::query!(
        "SELECT task_id FROM agent_tasks WHERE task_id LIKE $1 ORDER BY created_at DESC LIMIT 1",
        pattern
    )
    .fetch_optional(&**pool)
    .await?;

    Ok(row.map(|r| r.task_id))
}

pub async fn fetch_task_info(pool: &Arc<PgPool>, task_id: &str) -> Result<TaskInfo> {
    let row = sqlx::query!(
        r#"SELECT task_id, context_id, agent_name, status, created_at, started_at, completed_at, execution_time_ms
           FROM agent_tasks WHERE task_id = $1"#,
        task_id
    )
    .fetch_one(&**pool)
    .await?;

    Ok(TaskInfo {
        task_id: row.task_id,
        context_id: row.context_id,
        agent_name: row.agent_name,
        status: row.status,
        created_at: row.created_at,
        started_at: row.started_at,
        completed_at: row.completed_at,
        execution_time_ms: row.execution_time_ms,
    })
}

pub async fn fetch_user_input(pool: &Arc<PgPool>, task_id: &str) -> Result<Option<String>> {
    let row = sqlx::query!(
        r#"SELECT mp.text_content
           FROM task_messages tm
           JOIN message_parts mp ON mp.message_id = tm.message_id AND mp.task_id = tm.task_id
           WHERE tm.task_id = $1 AND tm.role = 'user' AND mp.part_kind = 'text'
           ORDER BY tm.sequence_number DESC LIMIT 1"#,
        task_id
    )
    .fetch_optional(&**pool)
    .await?;

    Ok(row.and_then(|r| r.text_content))
}

pub async fn fetch_agent_response(pool: &Arc<PgPool>, task_id: &str) -> Result<Option<String>> {
    let row = sqlx::query!(
        r#"SELECT mp.text_content
           FROM task_messages tm
           JOIN message_parts mp ON mp.message_id = tm.message_id AND mp.task_id = tm.task_id
           WHERE tm.task_id = $1 AND tm.role = 'agent' AND mp.part_kind = 'text'
           ORDER BY tm.sequence_number DESC LIMIT 1"#,
        task_id
    )
    .fetch_optional(&**pool)
    .await?;

    Ok(row.and_then(|r| r.text_content))
}

pub async fn fetch_execution_steps(
    pool: &Arc<PgPool>,
    task_id: &str,
) -> Result<Vec<ExecutionStep>> {
    let rows = sqlx::query!(
        r#"SELECT
               step_id as id,
               content->>'type' as step_type,
               COALESCE(content->>'title', content->>'type') as title,
               status,
               duration_ms,
               error_message
           FROM task_execution_steps
           WHERE task_id = $1
           ORDER BY started_at"#,
        task_id
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ExecutionStep {
            step_id: r.id,
            step_type: r.step_type,
            title: r.title,
            status: r.status,
            duration_ms: r.duration_ms,
            error_message: r.error_message,
        })
        .collect())
}

pub async fn fetch_ai_requests(pool: &Arc<PgPool>, task_id: &str) -> Result<Vec<AiRequestInfo>> {
    let rows = sqlx::query!(
        r#"SELECT id, model, provider, max_tokens, input_tokens, output_tokens, cost_cents, latency_ms
           FROM ai_requests
           WHERE task_id = $1
           ORDER BY created_at"#,
        task_id
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

pub async fn fetch_system_prompt(pool: &Arc<PgPool>, request_id: &str) -> Result<Option<String>> {
    let row = sqlx::query!(
        r#"SELECT content
           FROM ai_request_messages
           WHERE request_id = $1 AND role = 'system' AND sequence_number = 0
           LIMIT 1"#,
        request_id
    )
    .fetch_optional(&**pool)
    .await?;

    Ok(row.map(|r| r.content))
}

pub async fn fetch_conversation_messages(
    pool: &Arc<PgPool>,
    request_id: &str,
) -> Result<Vec<ConversationMessage>> {
    let rows = sqlx::query!(
        r#"SELECT role, content, sequence_number
           FROM ai_request_messages
           WHERE request_id = $1
           ORDER BY sequence_number"#,
        request_id
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ConversationMessage {
            role: r.role,
            content: r.content,
            sequence_number: r.sequence_number,
        })
        .collect())
}

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

pub async fn fetch_ai_request_message_previews(
    pool: &Arc<PgPool>,
    request_id: &str,
) -> Result<Vec<ConversationMessage>> {
    let rows = sqlx::query!(
        r#"SELECT role, LEFT(content, 500) as content_preview, sequence_number
           FROM ai_request_messages
           WHERE request_id = $1
           ORDER BY sequence_number"#,
        request_id
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ConversationMessage {
            role: r.role,
            content: r.content_preview.unwrap_or_default(),
            sequence_number: r.sequence_number,
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
