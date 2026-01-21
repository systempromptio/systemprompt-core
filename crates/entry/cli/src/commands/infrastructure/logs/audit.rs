use std::sync::Arc;

use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::audit_display::render_text_output;
use super::types::MessageRow;
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct AuditArgs {
    #[arg(help = "AI request ID, task ID, or trace ID")]
    pub id: String,

    #[arg(long, help = "Show full message content without truncation")]
    pub full: bool,

    #[arg(long, help = "Output as JSON")]
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AuditOutput {
    pub request_id: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cost_dollars: f64,
    pub latency_ms: i64,
    pub task_id: Option<String>,
    pub trace_id: Option<String>,
    pub messages: Vec<MessageRow>,
    pub tool_calls: Vec<AuditToolCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AuditToolCall {
    pub tool_name: String,
    pub tool_input: String,
    pub sequence: i32,
}

struct AiRequestRow {
    id: String,
    provider: String,
    model: String,
    input_tokens: Option<i32>,
    output_tokens: Option<i32>,
    cost_cents: i32,
    latency_ms: Option<i32>,
    task_id: Option<String>,
    trace_id: Option<String>,
}

struct DbMessageRow {
    role: String,
    content: String,
    sequence_number: i32,
}

struct ToolCallRow {
    tool_name: String,
    tool_input: String,
    sequence_number: i32,
}

pub async fn execute(args: AuditArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;
    execute_with_pool_inner(args, &pool, config).await
}

pub async fn execute_with_pool(
    args: AuditArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let pool = db_ctx.db_pool().pool_arc()?;
    execute_with_pool_inner(args, &pool, config).await
}

async fn execute_with_pool_inner(
    args: AuditArgs,
    pool: &Arc<sqlx::PgPool>,
    config: &CliConfig,
) -> Result<()> {
    let row = find_request(pool, &args.id).await?;

    let Some(row) = row else {
        render_not_found(&args.id);
        return Ok(());
    };

    let output = build_output(pool, row).await?;

    if config.is_json_output() || args.json {
        let result = CommandResult::card(output).with_title("AI Request Audit");
        render_result(&result);
    } else {
        render_text_output(&output, args.full);
    }

    Ok(())
}

async fn find_request(pool: &Arc<sqlx::PgPool>, id: &str) -> Result<Option<AiRequestRow>> {
    let partial_match = format!("{id}%");

    let request = find_by_request_id(pool, id, &partial_match).await?;
    if request.is_some() {
        return Ok(request);
    }

    let request = find_by_task_id(pool, id, &partial_match).await?;
    if request.is_some() {
        return Ok(request);
    }

    find_by_trace_id(pool, id, &partial_match).await
}

async fn find_by_request_id(
    pool: &Arc<sqlx::PgPool>,
    id: &str,
    partial: &str,
) -> Result<Option<AiRequestRow>> {
    Ok(sqlx::query_as!(
        AiRequestRow,
        r#"
        SELECT id as "id!", provider as "provider!", model as "model!",
            input_tokens, output_tokens, cost_cents as "cost_cents!",
            latency_ms, task_id, trace_id
        FROM ai_requests WHERE id = $1 OR id LIKE $2 LIMIT 1
        "#,
        id,
        partial
    )
    .fetch_optional(pool.as_ref())
    .await?)
}

async fn find_by_task_id(
    pool: &Arc<sqlx::PgPool>,
    id: &str,
    partial: &str,
) -> Result<Option<AiRequestRow>> {
    Ok(sqlx::query_as!(
        AiRequestRow,
        r#"
        SELECT id as "id!", provider as "provider!", model as "model!",
            input_tokens, output_tokens, cost_cents as "cost_cents!",
            latency_ms, task_id, trace_id
        FROM ai_requests WHERE task_id = $1 OR task_id LIKE $2
        ORDER BY created_at DESC LIMIT 1
        "#,
        id,
        partial
    )
    .fetch_optional(pool.as_ref())
    .await?)
}

async fn find_by_trace_id(
    pool: &Arc<sqlx::PgPool>,
    id: &str,
    partial: &str,
) -> Result<Option<AiRequestRow>> {
    Ok(sqlx::query_as!(
        AiRequestRow,
        r#"
        SELECT id as "id!", provider as "provider!", model as "model!",
            input_tokens, output_tokens, cost_cents as "cost_cents!",
            latency_ms, task_id, trace_id
        FROM ai_requests WHERE trace_id = $1 OR trace_id LIKE $2
        ORDER BY created_at DESC LIMIT 1
        "#,
        id,
        partial
    )
    .fetch_optional(pool.as_ref())
    .await?)
}

fn render_not_found(id: &str) {
    use systemprompt_logging::CliService;
    CliService::warning(&format!("No AI request found for: {id}"));
    CliService::info("Tip: Use 'systemprompt infra logs request list' to see recent requests");
    CliService::info("     Use 'systemprompt infra logs trace list' to see recent traces");
}

async fn build_output(pool: &Arc<sqlx::PgPool>, row: AiRequestRow) -> Result<AuditOutput> {
    let messages = fetch_messages(pool, &row.id).await?;
    let tool_calls = fetch_tool_calls(pool, &row.id).await?;

    Ok(AuditOutput {
        request_id: row.id,
        provider: row.provider,
        model: row.model,
        input_tokens: row.input_tokens.unwrap_or(0),
        output_tokens: row.output_tokens.unwrap_or(0),
        cost_dollars: f64::from(row.cost_cents) / 100.0,
        latency_ms: i64::from(row.latency_ms.unwrap_or(0)),
        task_id: row.task_id,
        trace_id: row.trace_id,
        messages,
        tool_calls,
    })
}

async fn fetch_messages(pool: &Arc<sqlx::PgPool>, request_id: &str) -> Result<Vec<MessageRow>> {
    let rows = sqlx::query_as!(
        DbMessageRow,
        r#"
        SELECT role as "role!", content as "content!", sequence_number as "sequence_number!"
        FROM ai_request_messages WHERE request_id = $1 ORDER BY sequence_number
        "#,
        request_id
    )
    .fetch_all(pool.as_ref())
    .await?;

    Ok(rows
        .into_iter()
        .map(|m| MessageRow {
            sequence: m.sequence_number,
            role: m.role,
            content: m.content,
        })
        .collect())
}

async fn fetch_tool_calls(
    pool: &Arc<sqlx::PgPool>,
    request_id: &str,
) -> Result<Vec<AuditToolCall>> {
    let rows = sqlx::query_as!(
        ToolCallRow,
        r#"
        SELECT tool_name as "tool_name!", tool_input as "tool_input!",
            sequence_number as "sequence_number!"
        FROM ai_request_tool_calls WHERE request_id = $1 ORDER BY sequence_number
        "#,
        request_id
    )
    .fetch_all(pool.as_ref())
    .await?;

    Ok(rows
        .into_iter()
        .map(|t| AuditToolCall {
            tool_name: t.tool_name,
            tool_input: t.tool_input,
            sequence: t.sequence_number,
        })
        .collect())
}
