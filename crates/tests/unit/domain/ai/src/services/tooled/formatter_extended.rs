use rmcp::model::{Annotated, Content, RawContent, RawTextContent};
use serde_json::json;
use systemprompt_ai::models::tools::{CallToolResult, ToolCall};
use systemprompt_ai::services::tooled::ToolResultFormatter;
use systemprompt_identifiers::AiToolCallId;

fn create_tool_call(name: &str) -> ToolCall {
    ToolCall {
        ai_tool_call_id: AiToolCallId::new(format!("call-{}", name)),
        name: name.to_string(),
        arguments: json!({}),
    }
}

fn create_text_content(text: &str) -> Content {
    Annotated {
        raw: RawContent::Text(RawTextContent {
            text: text.to_string(),
            meta: None,
        }),
        annotations: None,
    }
}

fn create_success_result(text: &str) -> CallToolResult {
    CallToolResult::success(vec![create_text_content(text)])
}

fn create_error_result(text: &str) -> CallToolResult {
    CallToolResult::error(vec![create_text_content(text)])
}

fn create_empty_success_result() -> CallToolResult {
    CallToolResult::success(vec![])
}

fn create_multi_content_result(texts: &[&str]) -> CallToolResult {
    let contents = texts.iter().map(|t| create_text_content(t)).collect();
    CallToolResult::success(contents)
}

mod format_single_for_ai_tests {
    use super::*;

    #[test]
    fn formats_with_tool_name_in_quotes() {
        let call = create_tool_call("my_search");
        let result = create_success_result("data");

        let formatted = ToolResultFormatter::format_single_for_ai(&call, &result);
        assert!(formatted.contains("'my_search'"));
    }

    #[test]
    fn formats_empty_content_result() {
        let call = create_tool_call("action");
        let result = create_empty_success_result();

        let formatted = ToolResultFormatter::format_single_for_ai(&call, &result);
        assert!(formatted.contains("action"));
        assert!(formatted.contains("SUCCESS"));
    }

    #[test]
    fn formats_special_characters_in_name() {
        let call = create_tool_call("mcp-server:tool.name");
        let result = create_success_result("ok");

        let formatted = ToolResultFormatter::format_single_for_ai(&call, &result);
        assert!(formatted.contains("mcp-server:tool.name"));
    }

    #[test]
    fn formats_multi_content_result() {
        let call = create_tool_call("multi");
        let result = create_multi_content_result(&["first part", "second part"]);

        let formatted = ToolResultFormatter::format_single_for_ai(&call, &result);
        assert!(formatted.contains("first part"));
        assert!(formatted.contains("second part"));
    }

    #[test]
    fn truncation_boundary_at_exactly_500() {
        let text = "a".repeat(500);
        let call = create_tool_call("exact");
        let result = create_success_result(&text);

        let formatted = ToolResultFormatter::format_single_for_ai(&call, &result);
        assert!(!formatted.contains("..."));
    }

    #[test]
    fn truncation_at_501_chars() {
        let text = "a".repeat(501);
        let call = create_tool_call("over");
        let result = create_success_result(&text);

        let formatted = ToolResultFormatter::format_single_for_ai(&call, &result);
        assert!(formatted.contains("..."));
    }
}

mod format_single_for_synthesis_tests {
    use super::*;

    #[test]
    fn summary_takes_first_non_empty_line() {
        let call = create_tool_call("test");
        let result = create_success_result("\n\nActual summary line\nMore details");

        let formatted = ToolResultFormatter::format_single_for_synthesis(&call, &result);
        assert!(formatted.contains("Actual summary line"));
    }

    #[test]
    fn summary_fallback_for_empty_content() {
        let call = create_tool_call("test");
        let result = create_empty_success_result();

        let formatted = ToolResultFormatter::format_single_for_synthesis(&call, &result);
        assert!(formatted.contains("No summary available"));
    }

    #[test]
    fn summary_truncated_to_200_chars() {
        let long_line = "x".repeat(300);
        let call = create_tool_call("test");
        let result = create_success_result(&long_line);

        let formatted = ToolResultFormatter::format_single_for_synthesis(&call, &result);
        assert!(formatted.contains("**Summary**"));
    }

    #[test]
    fn contains_markdown_headers() {
        let call = create_tool_call("my_tool");
        let result = create_success_result("content");

        let formatted = ToolResultFormatter::format_single_for_synthesis(&call, &result);
        assert!(formatted.contains("### Tool:"));
        assert!(formatted.contains("**Summary**"));
        assert!(formatted.contains("**Details**"));
    }

    #[test]
    fn error_result_no_completion_note() {
        let call = create_tool_call("failing");
        let result = create_error_result("error occurred");

        let formatted = ToolResultFormatter::format_single_for_synthesis(&call, &result);
        assert!(formatted.contains("FAILED"));
        assert!(!formatted.contains("IMPORTANT"));
    }

    #[test]
    fn success_result_has_completion_note() {
        let call = create_tool_call("succeeding");
        let result = create_success_result("done");

        let formatted = ToolResultFormatter::format_single_for_synthesis(&call, &result);
        assert!(formatted.contains("Do NOT call this tool again"));
    }
}

mod format_single_for_display_tests {
    use super::*;

    #[test]
    fn index_number_prefixes_output() {
        let call = create_tool_call("test");
        let result = create_success_result("data");

        let formatted = ToolResultFormatter::format_single_for_display(5, &call, &result);
        assert!(formatted.starts_with("5."));
    }

    #[test]
    fn preview_truncated_to_200() {
        let long_text = "y".repeat(300);
        let call = create_tool_call("test");
        let result = create_success_result(&long_text);

        let formatted = ToolResultFormatter::format_single_for_display(1, &call, &result);
        assert!(formatted.len() < 300);
        assert!(formatted.contains("..."));
    }

    #[test]
    fn shows_status_in_brackets() {
        let call = create_tool_call("test");
        let result = create_success_result("ok");

        let formatted = ToolResultFormatter::format_single_for_display(1, &call, &result);
        assert!(formatted.contains("[SUCCESS]"));
    }

    #[test]
    fn error_status_in_brackets() {
        let call = create_tool_call("test");
        let result = create_error_result("fail");

        let formatted = ToolResultFormatter::format_single_for_display(1, &call, &result);
        assert!(formatted.contains("[FAILED]"));
    }
}

mod format_for_ai_empty_tests {
    use super::*;

    #[test]
    fn empty_input_returns_empty_string() {
        let calls: Vec<ToolCall> = vec![];
        let results: Vec<CallToolResult> = vec![];

        let formatted = ToolResultFormatter::format_for_ai(&calls, &results);
        assert!(formatted.is_empty());
    }
}

mod format_for_synthesis_empty_tests {
    use super::*;

    #[test]
    fn empty_input_returns_empty_string() {
        let calls: Vec<ToolCall> = vec![];
        let results: Vec<CallToolResult> = vec![];

        let formatted = ToolResultFormatter::format_for_synthesis(&calls, &results);
        assert!(formatted.is_empty());
    }
}

mod format_for_display_empty_tests {
    use super::*;

    #[test]
    fn empty_input_returns_empty_string() {
        let calls: Vec<ToolCall> = vec![];
        let results: Vec<CallToolResult> = vec![];

        let formatted = ToolResultFormatter::format_for_display(&calls, &results);
        assert!(formatted.is_empty());
    }
}

mod fallback_summary_edge_cases {
    use super::*;

    #[test]
    fn skips_success_with_empty_content() {
        let calls = vec![create_tool_call("empty")];
        let results = vec![create_empty_success_result()];

        let formatted = ToolResultFormatter::format_fallback_summary(&calls, &results);
        assert_eq!(formatted, "Tool execution completed.");
    }

    #[test]
    fn success_result_formats_with_tool_name() {
        let calls = vec![create_tool_call("unknown")];
        let results = vec![create_success_result("data")];

        let formatted = ToolResultFormatter::format_fallback_summary(&calls, &results);
        assert!(formatted.contains("unknown"));
        assert!(formatted.contains("data"));
    }

    #[test]
    fn multiple_tools_separated_by_double_newline() {
        let calls = vec![create_tool_call("tool_a"), create_tool_call("tool_b")];
        let results = vec![
            create_success_result("Result A"),
            create_success_result("Result B"),
        ];

        let formatted = ToolResultFormatter::format_fallback_summary(&calls, &results);
        assert!(formatted.contains("\n\n"));
    }
}
