use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;

use systemprompt_identifiers::AiRequestId;

use super::models::{
    AiRequestDetail, AiRequestListItem, AiRequestStats, AuditLookupResult, AuditToolCallRow,
    ConversationMessage, LinkedMcpCall, ModelStatsRow, ProviderStatsRow,
};

struct ListRow {
    id: String,
    created_at: DateTime<Utc>,
    trace_id: Option<String>,
    provider: String,
    model: String,
    input_tokens: Option<i32>,
    output_tokens: Option<i32>,
    cost_microdollars: i64,
    latency_ms: Option<i32>,
    status: String,
}

struct TotalRow {
    request_count: Option<i64>,
    total_input_tokens: Option<i64>,
    total_output_tokens: Option<i64>,
    total_cost_microdollars: Option<i64>,
    avg_latency_ms: Option<i64>,
}

struct ProviderRow {
    provider: String,
    request_count: Option<i64>,
    total_tokens: Option<i64>,
    total_cost_microdollars: Option<i64>,
    avg_latency_ms: Option<i64>,
}

struct ModelRow {
    model: String,
    provider: String,
    request_count: Option<i64>,
    total_tokens: Option<i64>,
    total_cost_microdollars: Option<i64>,
    avg_latency_ms: Option<i64>,
}

struct DetailRow {
    id: String,
    provider: String,
    model: String,
    input_tokens: Option<i32>,
    output_tokens: Option<i32>,
    cost_microdollars: i64,
    latency_ms: Option<i32>,
    status: String,
    error_message: Option<String>,
}

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

pub async fn list_ai_requests(
    pool: &Arc<PgPool>,
    since: Option<DateTime<Utc>>,
    model: Option<&str>,
    provider: Option<&str>,
    limit: i64,
) -> Result<Vec<AiRequestListItem>> {
    let rows = sqlx::query_as!(
        ListRow,
        r#"
        SELECT
            id as "id!",
            created_at as "created_at!",
            trace_id,
            provider as "provider!",
            model as "model!",
            input_tokens,
            output_tokens,
            cost_microdollars as "cost_microdollars!",
            latency_ms,
            status as "status!"
        FROM ai_requests
        WHERE ($1::timestamptz IS NULL OR created_at >= $1)
          AND ($2::text IS NULL OR model ILIKE $2)
          AND ($3::text IS NULL OR provider ILIKE $3)
        ORDER BY created_at DESC
        LIMIT $4
        "#,
        since,
        model,
        provider,
        limit
    )
    .fetch_all(&**pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| AiRequestListItem {
            id: AiRequestId::new(r.id),
            created_at: r.created_at,
            trace_id: r.trace_id.map(Into::into),
            provider: r.provider,
            model: r.model,
            input_tokens: r.input_tokens,
            output_tokens: r.output_tokens,
            cost_microdollars: r.cost_microdollars,
            latency_ms: r.latency_ms,
            status: r.status,
        })
        .collect())
}

pub async fn get_ai_request_stats(
    pool: &Arc<PgPool>,
    since: Option<DateTime<Utc>>,
) -> Result<AiRequestStats> {
    let totals = sqlx::query_as!(
        TotalRow,
        r#"
        SELECT
            COUNT(*) as "request_count",
            COALESCE(SUM(input_tokens), 0) as "total_input_tokens",
            COALESCE(SUM(output_tokens), 0) as "total_output_tokens",
            COALESCE(SUM(cost_microdollars), 0)::bigint as "total_cost_microdollars",
            COALESCE(AVG(latency_ms), 0)::bigint as "avg_latency_ms"
        FROM ai_requests
        WHERE ($1::timestamptz IS NULL OR created_at >= $1)
        "#,
        since
    )
    .fetch_one(&**pool)
    .await?;

    let provider_rows = sqlx::query_as!(
        ProviderRow,
        r#"
        SELECT
            provider as "provider!",
            COUNT(*) as "request_count",
            COALESCE(SUM(input_tokens), 0) + COALESCE(SUM(output_tokens), 0) as "total_tokens",
            COALESCE(SUM(cost_microdollars), 0)::bigint as "total_cost_microdollars",
            COALESCE(AVG(latency_ms), 0)::bigint as "avg_latency_ms"
        FROM ai_requests
        WHERE ($1::timestamptz IS NULL OR created_at >= $1)
        GROUP BY provider
        ORDER BY request_count DESC
        "#,
        since
    )
    .fetch_all(&**pool)
    .await?;

    let model_rows = sqlx::query_as!(
        ModelRow,
        r#"
        SELECT
            model as "model!",
            provider as "provider!",
            COUNT(*) as "request_count",
            COALESCE(SUM(input_tokens), 0) + COALESCE(SUM(output_tokens), 0) as "total_tokens",
            COALESCE(SUM(cost_microdollars), 0)::bigint as "total_cost_microdollars",
            COALESCE(AVG(latency_ms), 0)::bigint as "avg_latency_ms"
        FROM ai_requests
        WHERE ($1::timestamptz IS NULL OR created_at >= $1)
        GROUP BY model, provider
        ORDER BY request_count DESC
        LIMIT 10
        "#,
        since
    )
    .fetch_all(&**pool)
    .await?;

    Ok(AiRequestStats {
        total_requests: totals.request_count.unwrap_or(0),
        total_input_tokens: totals.total_input_tokens.unwrap_or(0),
        total_output_tokens: totals.total_output_tokens.unwrap_or(0),
        total_cost_microdollars: totals.total_cost_microdollars.unwrap_or(0),
        avg_latency_ms: totals.avg_latency_ms.unwrap_or(0),
        by_provider: provider_rows
            .into_iter()
            .map(|r| ProviderStatsRow {
                provider: r.provider,
                request_count: r.request_count.unwrap_or(0),
                total_tokens: r.total_tokens.unwrap_or(0),
                total_cost_microdollars: r.total_cost_microdollars.unwrap_or(0),
                avg_latency_ms: r.avg_latency_ms.unwrap_or(0),
            })
            .collect(),
        by_model: model_rows
            .into_iter()
            .map(|r| ModelStatsRow {
                model: r.model,
                provider: r.provider,
                request_count: r.request_count.unwrap_or(0),
                total_tokens: r.total_tokens.unwrap_or(0),
                total_cost_microdollars: r.total_cost_microdollars.unwrap_or(0),
                avg_latency_ms: r.avg_latency_ms.unwrap_or(0),
            })
            .collect(),
    })
}

pub async fn find_ai_request_detail(
    pool: &Arc<PgPool>,
    id: &str,
) -> Result<Option<AiRequestDetail>> {
    let partial = format!("{id}%");
    let row = sqlx::query_as!(
        DetailRow,
        r#"
        SELECT
            id as "id!",
            provider as "provider!",
            model as "model!",
            input_tokens,
            output_tokens,
            cost_microdollars as "cost_microdollars!",
            latency_ms,
            status as "status!",
            error_message
        FROM ai_requests
        WHERE id = $1 OR id LIKE $2
        LIMIT 1
        "#,
        id,
        partial
    )
    .fetch_optional(&**pool)
    .await?;

    Ok(row.map(|r| AiRequestDetail {
        id: AiRequestId::new(r.id),
        provider: r.provider,
        model: r.model,
        input_tokens: r.input_tokens,
        output_tokens: r.output_tokens,
        cost_microdollars: r.cost_microdollars,
        latency_ms: r.latency_ms,
        status: r.status,
        error_message: r.error_message,
    }))
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
        task_id: r.task_id.map(Into::into),
        trace_id: r.trace_id.map(Into::into),
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
