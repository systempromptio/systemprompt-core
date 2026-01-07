use systemprompt_core_logging::{
    AiRequestInfo, AiTraceService, CliService, McpToolExecution, ToolLogEntry,
};

use super::ai_display::{print_content_block, print_section, truncate, ToolCallRow};
use tabled::settings::Style;
use tabled::Table;

pub async fn print_mcp_executions(
    service: &AiTraceService,
    executions: &[McpToolExecution],
    task_id: &str,
    context_id: &str,
    show_full: bool,
) {
    if executions.is_empty() {
        print_tool_errors_from_logs(service, task_id, context_id).await;
        return;
    }

    let tool_rows: Vec<ToolCallRow> = executions
        .iter()
        .map(|e| ToolCallRow {
            tool_name: e.tool_name.clone(),
            server: e.server_name.clone(),
            status: e.status.clone(),
            duration: e.execution_time_ms.map_or_else(|| "-".to_string(), |ms| format!("{}ms", ms)),
        })
        .collect();

    print_section("MCP TOOL EXECUTIONS");
    let table = Table::new(tool_rows).with(Style::rounded()).to_string();
    CliService::info(&table);

    for exec in executions {
        if exec.status == "failed" {
            if let Some(ref error) = exec.error_message {
                CliService::error(&format!("  {} failed:", exec.tool_name));
                print_content_block(error);
            }
        }

        print_tool_io(exec, show_full);

        if let Ok(linked_requests) = service
            .get_mcp_linked_ai_requests(&exec.mcp_execution_id)
            .await
        {
            if !linked_requests.is_empty() {
                print_mcp_linked_ai_requests(service, &linked_requests, &exec.tool_name).await;
            }
        }
    }
}

fn print_tool_io(exec: &McpToolExecution, show_full: bool) {
    let has_input = !exec.input.is_empty();
    let has_output = exec.output.as_ref().is_some_and(|s| !s.is_empty());

    if has_input || has_output {
        CliService::info(&format!("  → {}:", exec.tool_name));

        if has_input {
            CliService::info("    input:");
            if show_full || exec.input.len() <= 200 {
                print_tool_content(&exec.input);
            } else {
                print_tool_content(&truncate(&exec.input, 200));
            }
        }

        if let Some(ref output_str) = exec.output {
            if !output_str.is_empty() {
                CliService::info("    result:");
                if show_full || output_str.len() <= 500 {
                    print_tool_content(output_str);
                } else {
                    print_tool_content(&truncate(output_str, 500));
                    CliService::info("    [Truncated - use --tool-results for full output]");
                }
            }
        }
    }
}

fn print_tool_content(content: &str) {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
        if let Ok(pretty) = serde_json::to_string_pretty(&json) {
            for line in pretty.lines() {
                CliService::info(&format!("      {line}"));
            }
            return;
        }
    }
    for line in content.lines() {
        CliService::info(&format!("      {line}"));
    }
}

async fn print_mcp_linked_ai_requests(
    service: &AiTraceService,
    requests: &[AiRequestInfo],
    tool_name: &str,
) {
    CliService::info(&format!("  → AI requests made by {tool_name}:"));

    for req in requests {
        let tokens = req.input_tokens.unwrap_or(0) + req.output_tokens.unwrap_or(0);
        let latency_str = req.latency_ms.map_or_else(|| "-".to_string(), |ms| format!("{ms}ms"));

        CliService::info(&format!(
            "    {} {}/{} | {tokens} tokens | {latency_str}",
            truncate(&req.id, 8),
            req.provider,
            req.model
        ));

        if let Ok(previews) = service.get_ai_request_message_previews(&req.id).await {
            for msg in previews {
                let preview = if msg.content.len() >= 500 {
                    format!("{}...", truncate(&msg.content, 200))
                } else if msg.role == "system" && msg.content.len() > 100 {
                    format!("[System: {} chars]", msg.content.len())
                } else {
                    truncate(&msg.content, 200)
                };

                CliService::info(&format!(
                    "      #{} [{}] {preview}",
                    msg.sequence_number,
                    msg.role.to_uppercase()
                ));
            }
        }
    }
}

async fn print_tool_errors_from_logs(service: &AiTraceService, task_id: &str, context_id: &str) {
    let logs = match service.get_tool_logs(task_id, context_id).await {
        Ok(logs) => logs,
        Err(_) => return,
    };

    if logs.is_empty() {
        return;
    }

    print_section("TOOL EXECUTION LOGS");
    CliService::info("  (MCP execution records not found - showing logs)");

    let mut has_errors = false;
    for log in &logs {
        let time_str = log.timestamp.format("%H:%M:%S%.3f").to_string();

        let level_symbol = match log.level.as_str() {
            "ERROR" => {
                has_errors = true;
                "✗"
            },
            "WARN" => "⚠",
            "DEBUG" => "·",
            _ => "•",
        };

        let log_line = format!(
            "  {time_str} {level_symbol} [{}] {}",
            log.module,
            truncate(&log.message, 100)
        );

        match log.level.as_str() {
            "ERROR" => CliService::error(&log_line),
            "WARN" => CliService::warning(&log_line),
            _ => CliService::info(&log_line),
        }
    }

    if has_errors {
        print_error_details(&logs);
    }
}

fn print_error_details(logs: &[ToolLogEntry]) {
    CliService::error("  Tool Errors:");
    for log in logs {
        if log.level == "ERROR" {
            CliService::error(&format!("    {}: error:", log.module));
            print_content_block(&format!("      {}", log.message));
        }
    }
}
