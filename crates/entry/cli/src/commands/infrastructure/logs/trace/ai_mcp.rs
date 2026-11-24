//! Linked MCP tool-execution rendering for AI traces.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ContextId, McpToolName, TaskId};
use systemprompt_logging::LogLevel;
use systemprompt_models::ai::StoredMessageRole;
use systemprompt_models::mcp::McpToolExecutionStatus;
use systemprompt_runtime::{AiRequestInfo, AiTraceService, McpToolExecution, ToolLogEntry};

use super::ai_display::{Transcript, truncate};
use crate::presentation::tables::mcp_tool_calls_table;

pub(super) async fn mcp_executions(
    out: &mut Transcript,
    service: &AiTraceService,
    executions: &[McpToolExecution],
    show_full: bool,
) {
    out.section("MCP TOOL EXECUTIONS");
    out.info(mcp_tool_calls_table(executions));

    for exec in executions {
        if exec.status == McpToolExecutionStatus::Failed
            && let Some(ref error) = exec.error_message
        {
            out.error(format!("  {} failed:", exec.tool_name));
            out.block(error);
        }

        tool_io(out, exec, show_full);

        match service
            .get_mcp_linked_ai_requests(&exec.mcp_execution_id)
            .await
        {
            Ok(linked_requests) => {
                if !linked_requests.is_empty() {
                    mcp_linked_ai_requests(out, service, &linked_requests, &exec.tool_name).await;
                }
            },
            Err(e) => tracing::warn!(
                mcp_execution_id = %exec.mcp_execution_id,
                error = %e,
                "Failed to fetch AI requests linked to MCP execution"
            ),
        }
    }
}

fn tool_io(out: &mut Transcript, exec: &McpToolExecution, show_full: bool) {
    let has_input = !exec.input.is_empty();
    let has_output = exec.output.as_ref().is_some_and(|s| !s.is_empty());

    if has_input || has_output {
        out.info(format!("  → {}:", exec.tool_name));

        if has_input {
            out.info("    input:");
            if show_full || exec.input.len() <= 200 {
                tool_content(out, &exec.input);
            } else {
                tool_content(out, &truncate(&exec.input, 200));
            }
        }

        if let Some(ref output_str) = exec.output
            && !output_str.is_empty()
        {
            out.info("    result:");
            if show_full || output_str.len() <= 500 {
                tool_content(out, output_str);
            } else {
                tool_content(out, &truncate(output_str, 500));
                out.info("    [Truncated - use --tool-results for full output]");
            }
        }
    }
}

fn tool_content(out: &mut Transcript, content: &str) {
    let pretty = serde_json::from_str::<serde_json::Value>(content)
        .ok()
        .and_then(|json| serde_json::to_string_pretty(&json).ok());
    for line in pretty.as_deref().unwrap_or(content).lines() {
        out.info(format!("      {line}"));
    }
}

async fn mcp_linked_ai_requests(
    out: &mut Transcript,
    service: &AiTraceService,
    requests: &[AiRequestInfo],
    tool_name: &McpToolName,
) {
    out.info(format!("  → AI requests made by {tool_name}:"));

    for req in requests {
        let tokens = req.input_tokens.unwrap_or(0) + req.output_tokens.unwrap_or(0);
        let latency_str = req
            .latency_ms
            .map_or_else(|| "-".to_owned(), |ms| format!("{ms}ms"));

        out.info(format!(
            "    {} {}/{} | {tokens} tokens | {latency_str}",
            truncate(req.id.as_str(), 8),
            req.provider.as_deref().unwrap_or("-"),
            req.model.as_deref().unwrap_or("-")
        ));

        let previews = match service.get_ai_request_message_previews(&req.id).await {
            Ok(previews) => previews,
            Err(e) => {
                tracing::warn!(request_id = %req.id, error = %e, "Failed to fetch message previews");
                continue;
            },
        };

        for msg in previews {
            let preview = if msg.content.len() >= 500 {
                format!("{}...", truncate(&msg.content, 200))
            } else if msg.role == StoredMessageRole::System && msg.content.len() > 100 {
                format!("[System: {} chars]", msg.content.len())
            } else {
                truncate(&msg.content, 200)
            };

            out.info(format!(
                "      #{} [{}] {preview}",
                msg.sequence_number,
                msg.role.as_str().to_uppercase()
            ));
        }
    }
}

pub(super) async fn tool_errors_from_logs(
    out: &mut Transcript,
    service: &AiTraceService,
    task_id: &TaskId,
    context_id: &ContextId,
) {
    let logs = match service.get_tool_logs(task_id, context_id).await {
        Ok(logs) => logs,
        Err(e) => {
            tracing::warn!(task_id = %task_id, error = %e, "Failed to fetch tool logs");
            return;
        },
    };

    if logs.is_empty() {
        return;
    }

    out.section("TOOL EXECUTION LOGS");
    out.info("  (MCP execution records not found - showing logs)");

    let mut has_errors = false;
    for log in &logs {
        let time_str = log.timestamp.format("%H:%M:%S%.3f").to_string();

        let level_symbol = match log.level {
            LogLevel::Error => {
                has_errors = true;
                "✗"
            },
            LogLevel::Warn => "⚠",
            LogLevel::Debug => "·",
            LogLevel::Info | LogLevel::Trace => "•",
        };

        let log_line = format!(
            "  {time_str} {level_symbol} [{}] {}",
            log.module,
            truncate(&log.message, 100)
        );

        out.level(log.level, log_line);
    }

    if has_errors {
        error_details(out, &logs);
    }
}

fn error_details(out: &mut Transcript, logs: &[ToolLogEntry]) {
    out.error("  Tool Errors:");
    for log in logs {
        if log.level == LogLevel::Error {
            out.error(format!("    {}: error:", log.module));
            out.block(&format!("      {}", log.message));
        }
    }
}
