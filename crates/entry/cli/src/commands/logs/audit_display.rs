use systemprompt_core_logging::CliService;

use super::audit::{AuditOutput, AuditToolCall};
use super::types::MessageRow;

pub fn render_text_output(output: &AuditOutput, full: bool) {
    CliService::section(&format!("AI Request Audit: {}", output.request_id));
    render_metadata(output);
    render_messages(&output.messages, full);
    render_tool_calls(&output.tool_calls, full);
}

fn render_metadata(output: &AuditOutput) {
    CliService::key_value("Provider", &output.provider);
    CliService::key_value("Model", &output.model);
    CliService::key_value("Input Tokens", &output.input_tokens.to_string());
    CliService::key_value("Output Tokens", &output.output_tokens.to_string());
    CliService::key_value("Cost", &format!("${:.6}", output.cost_dollars));
    CliService::key_value("Latency", &format!("{}ms", output.latency_ms));

    if let Some(task_id) = &output.task_id {
        CliService::key_value("Task ID", task_id);
    }
    if let Some(trace_id) = &output.trace_id {
        CliService::key_value("Trace ID", trace_id);
    }
}

fn render_messages(messages: &[MessageRow], full: bool) {
    if messages.is_empty() {
        return;
    }

    CliService::section("Messages");
    for msg in messages {
        CliService::info(&format!(
            "\n[{}] #{} ────────────────────────────────────────",
            msg.role.to_uppercase(),
            msg.sequence
        ));
        render_content(&msg.content, full, 500);
    }
}

fn render_tool_calls(tool_calls: &[AuditToolCall], full: bool) {
    if tool_calls.is_empty() {
        return;
    }

    CliService::section("Tool Calls");
    for tc in tool_calls {
        CliService::info(&format!(
            "\n[TOOL #{}] {} ────────────────────────────────────────",
            tc.sequence, tc.tool_name
        ));
        render_tool_input(&tc.tool_input, full);
    }
}

fn render_content(content: &str, full: bool, max_len: usize) {
    if full {
        CliService::info(content);
    } else if content.len() > max_len {
        CliService::info(&format!(
            "{}...\n[truncated, use --full to see all]",
            &content[..max_len]
        ));
    } else {
        CliService::info(content);
    }
}

fn render_tool_input(input: &str, full: bool) {
    if full {
        let formatted = serde_json::from_str::<serde_json::Value>(input)
            .ok()
            .and_then(|v| serde_json::to_string_pretty(&v).ok())
            .unwrap_or_else(|| input.to_string());
        CliService::info(&formatted);
    } else {
        render_content(input, false, 300);
    }
}
