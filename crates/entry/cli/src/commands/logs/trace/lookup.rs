use anyhow::Result;
use clap::Args;
use systemprompt_core_logging::{AiTraceService, CliService};
use systemprompt_runtime::AppContext;

use super::{AiLookupOutput, MessageRow, ToolCallRow};
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct LookupArgs {
    #[arg(help = "AI request ID to lookup")]
    pub request_id: String,

    #[arg(long, help = "Show conversation messages")]
    pub show_messages: bool,

    #[arg(long, help = "Show linked MCP tool calls")]
    pub show_linked: bool,
}

pub async fn execute(args: LookupArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    // Query the AI request details
    let query = r"
        SELECT
            id,
            provider,
            model,
            input_tokens,
            output_tokens,
            cost_cents,
            latency_ms
        FROM ai_requests
        WHERE id = $1 OR id LIKE $2
        LIMIT 1
    ";

    let partial_match = format!("{}%", args.request_id);
    let row = match sqlx::query_as::<_, (String, String, String, Option<i32>, Option<i32>, i32, Option<i64>)>(query)
        .bind(&args.request_id)
        .bind(&partial_match)
        .fetch_optional(pool.as_ref())
        .await?
    {
        Some(r) => r,
        None => {
            CliService::warning(&format!("AI request not found: {}", args.request_id));
            CliService::info("Tip: Use 'systemprompt logs trace list' to see available traces");
            return Ok(());
        }
    };

    let request_id = row.0.clone();
    let cost_dollars = f64::from(row.5) / 1_000_000.0;

    let mut messages = Vec::new();
    if args.show_messages {
        let service = AiTraceService::new(pool.clone());
        if let Ok(msgs) = service.get_conversation_messages(&request_id).await {
            messages = msgs
                .into_iter()
                .map(|m| MessageRow {
                    sequence: m.sequence_number,
                    role: m.role,
                    content: m.content,
                })
                .collect();
        }
    }

    let mut linked_mcp_calls = Vec::new();
    if args.show_linked {
        let linked_query = r"
            SELECT
                mte.tool_name,
                mte.server_name,
                mte.status,
                mte.execution_time_ms
            FROM mcp_tool_executions mte
            JOIN ai_request_mcp_links arml ON arml.mcp_execution_id = mte.id
            WHERE arml.ai_request_id = $1
        ";

        let linked_rows = sqlx::query_as::<_, (String, String, String, Option<i64>)>(linked_query)
            .bind(&request_id)
            .fetch_all(pool.as_ref())
            .await?;

        linked_mcp_calls = linked_rows
            .into_iter()
            .map(|r| ToolCallRow {
                tool_name: r.0,
                server: r.1,
                status: r.2,
                duration_ms: r.3,
            })
            .collect();
    }

    let output = AiLookupOutput {
        request_id,
        provider: row.1,
        model: row.2,
        input_tokens: row.3.unwrap_or(0),
        output_tokens: row.4.unwrap_or(0),
        cost_dollars,
        latency_ms: row.6.unwrap_or(0),
        messages,
        linked_mcp_calls,
    };

    if !config.is_json_output() {
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
                CliService::info(&format!("[{}] #{}: {}", msg.role.to_uppercase(), msg.sequence,
                    if msg.content.len() > 200 {
                        format!("{}...", &msg.content[..200])
                    } else {
                        msg.content.clone()
                    }
                ));
            }
        }

        if !output.linked_mcp_calls.is_empty() {
            CliService::section("Linked MCP Calls");
            for call in &output.linked_mcp_calls {
                CliService::info(&format!(
                    "{} ({}) - {} {}",
                    call.tool_name,
                    call.server,
                    call.status,
                    call.duration_ms.map_or_else(String::new, |ms| format!("({}ms)", ms))
                ));
            }
        }
    } else {
        let result = CommandResult::card(output)
            .with_title("AI Request Details");
        render_result(&result);
    }

    Ok(())
}
