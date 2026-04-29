use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;

use systemprompt_identifiers::{AiRequestId, TaskId, TraceId};

use super::models::{AuditLookupResult, AuditToolCallRow, ConversationMessage, LinkedMcpCall};

struct AuditRow {
    id: String,
    provider: String,
    model: String,
    input_tokens: Option<i32>,
    output_tokens: Option<i32>,
    cost_microdollars: i64,
    latency_ms: Option<i32>,
    task_id: Option<String>,
    trace_id: Option<String>,
}

struct MsgRow {
    role: String,
    content: String,
    sequence_number: i32,
}

struct ToolCallDbRow {
    tool_name: String,
    tool_input: String,
    sequence_number: i32,
}

struct LinkedMcpDbRow {
    tool_name: String,
    server_name: String,
    status: String,
    execution_time_ms: Option<i32>,
}

pub async fn find_ai_request_for_audit(
    pool: &Arc<PgPool>,
    id: &str,
) -> Result<Option<AuditLookupResult>> {
    let partial = format!("{id}%");

    if let Some(row) = find_audit_by_request_id(pool, id, &partial).await? {
        return Ok(Some(row));
    }
    if let Some(row) = find_audit_by_task_id(pool, id, &partial).await? {
        return Ok(Some(row));
    }
    find_audit_by_trace_id(pool, id, &partial).await
}

async fn find_audit_by_request_id(
    pool: &Arc<PgPool>,
    id: &str,
    partial: &str,
) -> Result<Option<AuditLookupResult>> {
    let row = sqlx::query_as!(
        AuditRow,
        r#"
        SELECT id as "id!", provider as "provider!", model as "model!",
            input_tokens, output_tokens, cost_microdollars as "cost_microdollars!",
            latency_ms, task_id, trace_id
        FROM ai_requests WHERE id = $1 OR id LIKE $2 LIMIT 1
        "#,
        id,
        partial
    )
    .fetch_optional(&**pool)
    .await?;

    Ok(row.map(audit_row_to_result))
}

async fn find_audit_by_task_id(
    pool: &Arc<PgPool>,
    id: &str,
    partial: &str,
) -> Result<Option<AuditLookupResult>> {
    let row = sqlx::query_as!(
        AuditRow,
        r#"
        SELECT id as "id!", provider as "provider!", model as "model!",
            input_tokens, output_tokens, cost_microdollars as "cost_microdollars!",
            latency_ms, task_id, trace_id
        FROM ai_requests WHERE task_id = $1 OR task_id LIKE $2
        ORDER BY created_at DESC LIMIT 1
        "#,
        id,
        partial
    )
    .fetch_optional(&**pool)
    .await?;

    Ok(row.map(audit_row_to_result))
}

async fn find_audit_by_trace_id(
    pool: &Arc<PgPool>,
    id: &str,
    partial: &str,
) -> Result<Option<AuditLookupResult>> {
    let row = sqlx::query_as!(
        AuditRow,
        r#"
        SELECT id as "id!", provider as "provider!", model as "model!",
            input_tokens, output_tokens, cost_microdollars as "cost_microdollars!",
            latency_ms, task_id, trace_id
        FROM ai_requests WHERE trace_id = $1 OR trace_id LIKE $2
        ORDER BY created_at DESC LIMIT 1
        "#,
        id,
        partial
    )
    .fetch_optional(&**pool)
    .await?;

    Ok(row.map(audit_row_to_result))
}

fn audit_row_to_result(r: AuditRow) -> AuditLookupResult {
    AuditLookupResult {
        id: AiRequestId::new(r.id),
        provider: r.provider,
        model: r.model,
        input_tokens: r.input_tokens,
        output_tokens: r.output_tokens,
        cost_microdollars: r.cost_microdollars,
        latency_ms: r.latency_ms,
        task_id: r.task_id.map(TaskId::new),
        trace_id: r.trace_id.map(TraceId::new),
    }
}

pub async fn list_audit_messages(
    pool: &Arc<PgPool>,
    request_id: &str,
) -> Result<Vec<ConversationMessage>> {
    let rows = sqlx::query_as!(
        MsgRow,
        r#"
        SELECT role as "role!", content as "content!", sequence_number as "sequence_number!"
        FROM ai_request_messages WHERE request_id = $1 ORDER BY sequence_number
        "#,
        request_id
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|m| ConversationMessage {
            role: m.role,
            content: m.content,
            sequence_number: m.sequence_number,
        })
        .collect())
}

pub async fn list_audit_tool_calls(
    pool: &Arc<PgPool>,
    request_id: &str,
) -> Result<Vec<AuditToolCallRow>> {
    let rows = sqlx::query_as!(
        ToolCallDbRow,
        r#"
        SELECT tool_name as "tool_name!", tool_input as "tool_input!",
            sequence_number as "sequence_number!"
        FROM ai_request_tool_calls WHERE request_id = $1 ORDER BY sequence_number
        "#,
        request_id
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|t| AuditToolCallRow {
            tool_name: t.tool_name,
            tool_input: t.tool_input,
            sequence_number: t.sequence_number,
        })
        .collect())
}

pub async fn list_linked_mcp_calls(
    pool: &Arc<PgPool>,
    request_id: &str,
) -> Result<Vec<LinkedMcpCall>> {
    let rows = sqlx::query_as!(
        LinkedMcpDbRow,
        r#"
        SELECT
            mte.tool_name as "tool_name!",
            mte.server_name as "server_name!",
            mte.status as "status!",
            mte.execution_time_ms
        FROM mcp_tool_executions mte
        JOIN ai_request_tool_calls artc ON artc.mcp_execution_id = mte.mcp_execution_id
        WHERE artc.request_id = $1
        "#,
        request_id
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| LinkedMcpCall {
            tool_name: r.tool_name,
            server_name: r.server_name,
            status: r.status,
            execution_time_ms: r.execution_time_ms,
        })
        .collect())
}
