use std::sync::Arc;

use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_logging::TraceQueryService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::audit_display::render_text_output;
use super::types::MessageRow;
use crate::CliConfig;
use crate::shared::{CommandResult, render_result};

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
    let service = TraceQueryService::new(Arc::clone(pool));

    let row = service.find_ai_request_for_audit(&args.id).await?;

    let Some(row) = row else {
        render_not_found(&args.id);
        return Ok(());
    };

    let (messages, tool_calls) = tokio::try_join!(
        service.list_audit_messages(&row.id),
        service.list_audit_tool_calls(&row.id),
    )?;

    let output = AuditOutput {
        request_id: row.id,
        provider: row.provider,
        model: row.model,
        input_tokens: row.input_tokens.unwrap_or(0),
        output_tokens: row.output_tokens.unwrap_or(0),
        cost_dollars: row.cost_microdollars as f64 / 1_000_000.0,
        latency_ms: i64::from(row.latency_ms.unwrap_or(0)),
        task_id: row.task_id.map(|id| id.to_string()),
        trace_id: row.trace_id.map(|id| id.to_string()),
        messages: messages
            .into_iter()
            .map(|m| MessageRow {
                sequence: m.sequence_number,
                role: m.role,
                content: m.content,
            })
            .collect(),
        tool_calls: tool_calls
            .into_iter()
            .map(|t| AuditToolCall {
                tool_name: t.tool_name,
                tool_input: t.tool_input,
                sequence: t.sequence_number,
            })
            .collect(),
    };

    if config.is_json_output() || args.json {
        let result = CommandResult::card(output).with_title("AI Request Audit");
        render_result(&result);
    } else {
        render_text_output(&output, args.full);
    }

    Ok(())
}

fn render_not_found(id: &str) {
    use systemprompt_logging::CliService;
    CliService::warning(&format!("No AI request found for: {id}"));
    CliService::info("Tip: Use 'systemprompt infra logs request list' to see recent requests");
    CliService::info("     Use 'systemprompt infra logs trace list' to see recent traces");
}
