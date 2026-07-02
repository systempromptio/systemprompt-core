//! `infra logs request show`: render one AI request with optional messages and
//! linked MCP tool calls.

use std::sync::Arc;

use anyhow::Result;
use clap::Args;
use systemprompt_identifiers::AiRequestId;
use systemprompt_logging::{AiTraceService, TraceQueryService};

use super::{
    MessageRow, RequestShowOutput, ToolCallRow, build_request_show, request_show_not_found,
};
use crate::shared::CommandOutput;

#[derive(Debug, Args)]
pub struct ShowArgs {
    #[arg(help = "AI request ID (can be partial)")]
    pub request_id: String,

    #[arg(long, short = 'm', help = "Show conversation messages")]
    pub messages: bool,

    #[arg(long, short = 't', help = "Show linked MCP tool calls")]
    pub tools: bool,
}

crate::define_pool_command!(ShowArgs => CommandOutput, no_config);

async fn execute_with_pool_inner(
    args: ShowArgs,
    pool: &Arc<sqlx::PgPool>,
) -> Result<CommandOutput> {
    let service = TraceQueryService::new(Arc::clone(pool));
    let Some(row) = service.find_ai_request_detail(&args.request_id).await? else {
        return Ok(request_show_not_found(&args.request_id));
    };

    let request_id = row.id;
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
        request_id: request_id.as_str().to_owned(),
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

    Ok(build_request_show(&output))
}

pub(super) async fn fetch_messages(
    pool: &Arc<sqlx::PgPool>,
    request_id: &AiRequestId,
) -> Vec<MessageRow> {
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
