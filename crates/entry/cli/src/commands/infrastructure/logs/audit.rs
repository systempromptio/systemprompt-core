//! `infra logs audit` command rendering the audit trail.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{AiRequestId, TaskId, TraceId};
use systemprompt_models::text::truncate_with_ellipsis;
use systemprompt_runtime::{AuditPage, TraceQueryService};

use super::types::MessageRow;
use crate::CliConfig;
use crate::shared::{CommandOutput, render_result};

#[derive(Debug, Args)]
pub struct AuditArgs {
    #[arg(help = "AI request ID, task ID, or trace ID")]
    pub id: String,

    #[arg(
        long,
        short = 'm',
        help = "Include the conversation messages (default: counts only)"
    )]
    pub messages: bool,

    #[arg(
        long,
        short = 't',
        help = "Include the tool calls (default: counts only)"
    )]
    pub tools: bool,

    #[arg(
        long,
        default_value = "0",
        allow_negative_numbers = true,
        value_parser = clap::value_parser!(i64).range(0..),
        help = "Skip this many messages / tool calls before the page"
    )]
    pub offset: i64,

    #[arg(
        long,
        short = 'n',
        default_value = "20",
        allow_negative_numbers = true,
        value_parser = clap::value_parser!(i64).range(0..),
        help = "Maximum messages / tool calls per page (0 = all)"
    )]
    pub limit: i64,

    #[arg(
        long,
        value_name = "CHARS",
        default_value = "0",
        help = "Truncate each message body and tool input to this many characters (0 = full)"
    )]
    pub max_content: usize,
}

impl AuditArgs {
    const fn page(&self) -> AuditPage {
        AuditPage {
            offset: self.offset,
            limit: self.limit,
        }
    }

    fn bounded(&self, text: String) -> String {
        if self.max_content == 0 {
            text
        } else {
            truncate_with_ellipsis(&text, self.max_content)
        }
    }
}

/// One audit page of a request.
///
/// `message_count` / `tool_call_count` are the request's totals whatever the
/// page; `messages` / `tool_calls` are the opted-in slice from `offset`, and
/// `has_more` says whether a later slice exists.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AuditOutput {
    pub request_id: AiRequestId,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub requested_model: Option<String>,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cache_read_tokens: i32,
    pub cache_creation_tokens: i32,
    #[serde(default)]
    pub reasoning_tokens: i32,
    pub cost_dollars: f64,
    pub latency_ms: i64,
    pub task_id: Option<TaskId>,
    pub trace_id: Option<TraceId>,
    pub message_count: i64,
    pub tool_call_count: i64,
    pub offset: i64,
    pub has_more: bool,
    pub messages: Vec<MessageRow>,
    pub tool_calls: Vec<AuditToolCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AuditToolCall {
    pub tool_name: String,
    pub tool_input: String,
    pub sequence: i32,
}

crate::define_pool_command!(AuditArgs => (), with_config);

async fn execute_with_pool_inner(
    args: AuditArgs,
    pool: &Arc<sqlx::PgPool>,
    config: &CliConfig,
) -> Result<()> {
    let service = TraceQueryService::new(Arc::clone(pool));

    let row = service.find_ai_request_for_audit(&args.id).await?;

    let Some(row) = row else {
        render_result(&not_found_output(&args.id), config);
        return Ok(());
    };

    let request_id = row.id;
    let page = args.page();
    let (message_count, tool_call_count) = tokio::try_join!(
        service.count_audit_messages(&request_id),
        service.count_audit_tool_calls(&request_id),
    )?;
    let messages = if args.messages {
        service.list_audit_messages(&request_id, page).await?
    } else {
        Vec::new()
    };
    let tool_calls = if args.tools {
        service.list_audit_tool_calls(&request_id, page).await?
    } else {
        Vec::new()
    };
    let page_end = page.offset.saturating_add(page.limit);
    let has_more = page.limit > 0
        && ((args.messages && page_end < message_count)
            || (args.tools && page_end < tool_call_count));

    let output = AuditOutput {
        request_id,
        status: row.status,
        finish_reason: row.finish_reason,
        error_message: row.error_message,
        provider: row.provider,
        model: row.model,
        requested_model: row.requested_model,
        input_tokens: row.input_tokens.unwrap_or(0),
        output_tokens: row.output_tokens.unwrap_or(0),
        cache_read_tokens: row.cache_read_tokens.unwrap_or(0),
        reasoning_tokens: row.reasoning_tokens.unwrap_or(0),
        cache_creation_tokens: row.cache_creation_tokens.unwrap_or(0),
        cost_dollars: row.cost_microdollars as f64 / 1_000_000.0,
        latency_ms: i64::from(row.latency_ms.unwrap_or(0)),
        task_id: row.task_id,
        trace_id: row.trace_id.map(TraceId::new),
        message_count,
        tool_call_count,
        offset: page.offset,
        has_more,
        messages: messages
            .into_iter()
            .map(|m| MessageRow {
                sequence: m.sequence_number,
                role: m.role,
                content: args.bounded(m.content),
            })
            .collect(),
        tool_calls: tool_calls
            .into_iter()
            .map(|t| AuditToolCall {
                tool_name: t.tool_name,
                tool_input: args.bounded(t.tool_input),
                sequence: t.sequence_number,
            })
            .collect(),
    };

    render_result(&build_audit(&output), config);

    Ok(())
}

#[must_use]
pub fn build_audit(output: &AuditOutput) -> CommandOutput {
    let title = if output.status == "completed" {
        "AI Request Audit".to_owned()
    } else {
        format!("AI Request Audit — {}", output.status.to_uppercase())
    };
    CommandOutput::card_value(title, output)
}

#[must_use]
pub fn not_found_output(id: &str) -> CommandOutput {
    use systemprompt_models::artifacts::NoticeLine;
    CommandOutput::message(vec![
        NoticeLine::new("warning", format!("No AI request found for: {id}")),
        NoticeLine::new(
            "info",
            "Tip: Use 'systemprompt infra logs request list' to see recent requests",
        ),
        NoticeLine::new(
            "info",
            "Use 'systemprompt infra logs trace list' to see recent traces",
        ),
    ])
}
