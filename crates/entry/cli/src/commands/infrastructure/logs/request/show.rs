use std::sync::Arc;

use anyhow::Result;
use clap::Args;
use systemprompt_logging::{AiTraceService, CliService, TraceQueryService};

use super::{MessageRow, RequestShowOutput, ToolCallRow};
use crate::CliConfig;
use crate::shared::CommandResult;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "AI request ID (can be partial)")]
    pub request_id: String,

    #[arg(long, short = 'm', help = "Show conversation messages")]
    pub messages: bool,

    #[arg(long, short = 't', help = "Show linked MCP tool calls")]
    pub tools: bool,

    #[arg(long, help = "Show full message content without truncation")]
    pub full: bool,
}

crate::define_pool_command!(ShowArgs => CommandResult<RequestShowOutput>, with_config);

async fn execute_with_pool_inner(
    args: ShowArgs,
    pool: &Arc<sqlx::PgPool>,
    config: &CliConfig,
) -> Result<CommandResult<RequestShowOutput>> {
    let service = TraceQueryService::new(Arc::clone(pool));
    let Some(row) = service.find_ai_request_detail(&args.request_id).await? else {
        if !config.is_json_output() {
            CliService::warning(&format!("AI request not found: {}", args.request_id));
            CliService::info(
                "Tip: Use 'systemprompt infra logs request list' to see recent requests",
            );
        }
        let empty_output = RequestShowOutput {
            request_id: args.request_id,
            provider: String::new(),
            model: String::new(),
            input_tokens: 0,
            output_tokens: 0,
            cost_dollars: 0.0,
            latency_ms: 0,
            status: "not_found".to_string(),
            error_message: Some("Request not found".to_string()),
            messages: Vec::new(),
            linked_mcp_calls: Vec::new(),
        };
        return Ok(CommandResult::card(empty_output)
            .with_title("AI Request Details")
            .with_skip_render());
    };

    let request_id = row.id.to_string();
    let cost_dollars = row.cost_microdollars as f64 / 1_000_000.0;

    let messages = if args.messages {
        fetch_messages(pool, &request_id).await
    } else {
        Vec::new()
    };

    let linked_mcp_calls = if args.tools {
        service
            .list_linked_mcp_calls(&request_id)
            .await?
            .into_iter()
            .map(|r| ToolCallRow {
                tool_name: r.tool_name,
                server: r.server_name,
                status: r.status,
                duration_ms: r.execution_time_ms.map(i64::from),
            })
            .collect()
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
        status: row.status,
        error_message: row.error_message,
        messages,
        linked_mcp_calls,
    };

    let result = CommandResult::card(output).with_title("AI Request Details");

    if config.is_json_output() {
        return Ok(result);
    }

    render_text_output(&result.data, args.full);
    Ok(result.with_skip_render())
}

async fn fetch_messages(pool: &Arc<sqlx::PgPool>, request_id: &str) -> Vec<MessageRow> {
    let service = AiTraceService::new(Arc::clone(pool));
    service
        .get_conversation_messages(request_id)
        .await
        .map_or_else(
            |e| {
                tracing::warn!(request_id = %request_id, error = %e, "Failed to fetch conversation messages");
                Vec::new()
            },
            |msgs| {
                msgs.into_iter()
                    .map(|m| MessageRow {
                        sequence: m.sequence_number,
                        role: m.role,
                        content: m.content,
                    })
                    .collect()
            },
        )
}


fn render_text_output(output: &RequestShowOutput, full: bool) {
    CliService::section(&format!("AI Request: {}", output.request_id));
    CliService::key_value("Provider", &output.provider);
    CliService::key_value("Model", &output.model);
    CliService::key_value("Input Tokens", &output.input_tokens.to_string());
    CliService::key_value("Output Tokens", &output.output_tokens.to_string());
    CliService::key_value("Cost", &format!("${:.6}", output.cost_dollars));
    CliService::key_value("Latency", &format!("{}ms", output.latency_ms));

    if output.status == "failed" {
        CliService::key_value("Status", "FAILED");
        if let Some(err) = &output.error_message {
            CliService::key_value("Error", err);
        }
    } else {
        CliService::key_value("Status", &output.status);
    }

    if !output.messages.is_empty() {
        CliService::section("Messages");
        for msg in &output.messages {
            let content_display = if full {
                msg.content.clone()
            } else if msg.content.len() > 200 {
                format!("{}...", &msg.content[..200])
            } else {
                msg.content.clone()
            };
            CliService::info(&format!(
                "[{}] #{}: {}",
                msg.role.to_uppercase(),
                msg.sequence,
                content_display
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
