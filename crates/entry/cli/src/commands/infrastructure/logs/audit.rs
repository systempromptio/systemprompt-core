use std::sync::Arc;

use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{AiRequestId, TaskId, TraceId};
use systemprompt_logging::TraceQueryService;

use super::types::MessageRow;
use crate::CliConfig;
use crate::shared::{CommandOutput, render_result};

#[derive(Debug, Args)]
pub struct AuditArgs {
    #[arg(help = "AI request ID, task ID, or trace ID")]
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AuditOutput {
    pub request_id: AiRequestId,
    pub provider: String,
    pub model: String,
    pub requested_model: Option<String>,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cost_dollars: f64,
    pub latency_ms: i64,
    pub task_id: Option<TaskId>,
    pub trace_id: Option<TraceId>,
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
    let (messages, tool_calls) = tokio::try_join!(
        service.list_audit_messages(&request_id),
        service.list_audit_tool_calls(&request_id),
    )?;

    let output = AuditOutput {
        request_id,
        provider: row.provider,
        model: row.model,
        requested_model: row.requested_model,
        input_tokens: row.input_tokens.unwrap_or(0),
        output_tokens: row.output_tokens.unwrap_or(0),
        cost_dollars: row.cost_microdollars as f64 / 1_000_000.0,
        latency_ms: i64::from(row.latency_ms.unwrap_or(0)),
        task_id: row.task_id,
        trace_id: row.trace_id.map(TraceId::new),
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

    render_result(&build_audit(&output), config);

    Ok(())
}

#[must_use]
pub fn build_audit(output: &AuditOutput) -> CommandOutput {
    CommandOutput::card_value("AI Request Audit", output)
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
