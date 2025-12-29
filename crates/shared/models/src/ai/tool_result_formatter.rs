use rmcp::model::{CallToolResult, RawContent};

use super::tools::ToolCall;

#[derive(Debug, Clone, Copy, Default)]
pub struct ToolResultFormatter;

impl ToolResultFormatter {
    pub fn format_for_ai(tool_calls: &[ToolCall], tool_results: &[CallToolResult]) -> String {
        tool_calls
            .iter()
            .zip(tool_results.iter())
            .map(|(call, result)| Self::format_single_for_ai(call, result))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn format_single_for_ai(call: &ToolCall, result: &CallToolResult) -> String {
        let status = Self::status_string(result);
        let content_text = Self::extract_text_content(result);

        format!(
            "Tool '{}' [{}]: {}",
            call.name,
            status,
            Self::truncate(&content_text, 500)
        )
    }

    pub fn format_for_synthesis(
        tool_calls: &[ToolCall],
        tool_results: &[CallToolResult],
    ) -> String {
        tool_calls
            .iter()
            .zip(tool_results.iter())
            .map(|(call, result)| Self::format_single_for_synthesis(call, result))
            .collect::<Vec<_>>()
            .join("\n---\n\n")
    }

    pub fn format_single_for_synthesis(call: &ToolCall, result: &CallToolResult) -> String {
        let status = Self::status_string(result);
        let is_success = !result.is_error.unwrap_or(false);
        let content_text = Self::extract_text_content(result);

        let summary = content_text
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("No summary available")
            .chars()
            .take(200)
            .collect::<String>();

        let truncated = Self::truncate(&content_text, 1000);

        let completion_note = if is_success {
            "\n\n**IMPORTANT**: This tool completed successfully. The action has been performed. \
             Do NOT call this tool again with the same parameters."
        } else {
            ""
        };

        format!(
            "### Tool: {} [{}]\n\n**Summary**: {}\n**Details** (truncated):\n{}{}",
            call.name, status, summary, truncated, completion_note
        )
    }

    pub fn format_for_display(tool_calls: &[ToolCall], tool_results: &[CallToolResult]) -> String {
        tool_calls
            .iter()
            .zip(tool_results.iter())
            .enumerate()
            .map(|(i, (call, result))| Self::format_single_for_display(i + 1, call, result))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn format_single_for_display(
        index: usize,
        call: &ToolCall,
        result: &CallToolResult,
    ) -> String {
        let status = Self::status_string(result);
        let content_text = Self::extract_text_content(result);
        let preview = Self::truncate(&content_text, 200);

        format!("{}. {} [{}]: {}", index, call.name, status, preview)
    }

    pub fn format_fallback_summary(
        tool_calls: &[ToolCall],
        tool_results: &[CallToolResult],
    ) -> String {
        let mut texts = Vec::new();

        for (call, result) in tool_calls.iter().zip(tool_results.iter()) {
            let is_error = result.is_error.unwrap_or(true);
            if is_error {
                continue;
            }

            let content_text = Self::extract_text_content(result);
            if !content_text.is_empty() {
                texts.push(format!("**{}**:\n{}", call.name, content_text));
            }
        }

        if texts.is_empty() {
            "Tool execution completed.".to_string()
        } else {
            texts.join("\n\n")
        }
    }

    fn status_string(result: &CallToolResult) -> &'static str {
        if result.is_error.unwrap_or(false) {
            "FAILED"
        } else {
            "SUCCESS"
        }
    }

    fn extract_text_content(result: &CallToolResult) -> String {
        result
            .content
            .iter()
            .filter_map(|c| match &c.raw {
                RawContent::Text(text_content) => Some(text_content.text.as_str()),
                RawContent::Image(_)
                | RawContent::Resource(_)
                | RawContent::Audio(_)
                | RawContent::ResourceLink(_) => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn truncate(s: &str, max_len: usize) -> String {
        if s.len() > max_len {
            format!("{}...", &s[..max_len])
        } else {
            s.to_string()
        }
    }
}
