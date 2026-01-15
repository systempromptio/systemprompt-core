use std::sync::Arc;

use anyhow::Result;
use clap::Args;
use systemprompt_core_logging::{AiTraceService, CliService};
use systemprompt_runtime::AppContext;

use super::{MessageRow, RequestShowOutput, ToolCallRow};
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "AI request ID (can be partial)")]
    pub request_id: String,

    #[arg(long, short = 'm', help = "Show conversation messages")]
    pub messages: bool,

    #[arg(long, short = 't', help = "Show linked MCP tool calls")]
    pub tools: bool,
}

struct AiRequestRow {
    id: String,
    provider: String,
    model: String,
    input_tokens: Option<i32>,
    output_tokens: Option<i32>,
    cost_cents: i32,
    latency_ms: Option<i32>,
}

struct LinkedMcpRow {
    tool_name: String,
    server_name: String,
    status: String,
    execution_time_ms: Option<i32>,
}

pub async fn execute(args: ShowArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let partial_match = format!("{}%", args.request_id);
    let Some(row) = sqlx::query_as!(
        AiRequestRow,
        r#"
        SELECT
            id as "id!",
            provider as "provider!",
            model as "model!",
            input_tokens,
            output_tokens,
            cost_cents as "cost_cents!",
            latency_ms
        FROM ai_requests
        WHERE id = $1 OR id LIKE $2
        LIMIT 1
        "#,
        args.request_id,
        partial_match
    )
    .fetch_optional(pool.as_ref())
    .await?
    else {
        CliService::warning(&format!("AI request not found: {}", args.request_id));
        CliService::info("Tip: Use 'systemprompt logs request list' to see recent requests");
        return Ok(());
    };

    let request_id = row.id.clone();
    let cost_dollars = f64::from(row.cost_cents) / 1_000_000.0;

    let messages = if args.messages {
        fetch_messages(&pool, &request_id).await
    } else {
        Vec::new()
    };

    let linked_mcp_calls = if args.tools {
        fetch_linked_mcp_calls(&pool, &request_id).await?
    } else {
        Vec::new()
    };

    let output = RequestShowOutput {
        request_id,
        provider: row.provider,
        model: row.model,
        input_tokens: row.input_tokens.unwrap_or(0),
        output_tokens: row.output_tokens.unwrap_or(0),
        cost_dollars,
        latency_ms: i64::from(row.latency_ms.unwrap_or(0)),
        messages,
        linked_mcp_calls,
    };

    if config.is_json_output() {
        let result = CommandResult::card(output).with_title("AI Request Details");
        render_result(&result);
    } else {
        render_text_output(&output);
    }

    Ok(())
}

async fn fetch_messages(pool: &Arc<sqlx::PgPool>, request_id: &str) -> Vec<MessageRow> {
    let service = AiTraceService::new(Arc::clone(pool));
    service
        .get_conversation_messages(request_id)
        .await
        .map(|msgs| {
            msgs.into_iter()
                .map(|m| MessageRow {
                    sequence: m.sequence_number,
                    role: m.role,
                    content: m.content,
                })
                .collect()
        })
        .unwrap_or_else(|e| {
            tracing::warn!(request_id = %request_id, error = %e, "Failed to fetch conversation messages");
            Vec::new()
        })
}

async fn fetch_linked_mcp_calls(
    pool: &Arc<sqlx::PgPool>,
    request_id: &str,
) -> Result<Vec<ToolCallRow>> {
    let rows = sqlx::query_as!(
        LinkedMcpRow,
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
    .fetch_all(pool.as_ref())
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ToolCallRow {
            tool_name: r.tool_name,
            server: r.server_name,
            status: r.status,
            duration_ms: r.execution_time_ms.map(i64::from),
        })
        .collect())
}

fn render_text_output(output: &RequestShowOutput) {
    CliService::section(&format!("AI Request: {}", output.request_id));
    CliService::key_value("Provider", &output.provider);
    CliService::key_value("Model", &output.model);
    CliService::key_value("Input Tokens", &output.input_tokens.to_string());
    CliService::key_value("Output Tokens", &output.output_tokens.to_string());
    CliService::key_value("Cost", &format!("${:.6}", output.cost_dollars));
    CliService::key_value("Latency", &format!("{}ms", output.latency_ms));

    if !output.messages.is_empty() {
        CliService::section("Messages");
        for msg in &output.messages {
            let content_preview = if msg.content.len() > 200 {
                format!("{}...", &msg.content[..200])
            } else {
                msg.content.clone()
            };
            CliService::info(&format!(
                "[{}] #{}: {}",
                msg.role.to_uppercase(),
                msg.sequence,
                content_preview
            ));
        }
    }

    if !output.linked_mcp_calls.is_empty() {
        CliService::section("Linked Tool Calls");
        for call in &output.linked_mcp_calls {
            let duration = call
                .duration_ms
                .map_or_else(String::new, |ms| format!("({}ms)", ms));
            CliService::info(&format!(
                "{} ({}) - {} {}",
                call.tool_name, call.server, call.status, duration
            ));
        }
    }
}
