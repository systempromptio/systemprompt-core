//! Tests for ToolResultFormatter.

use systemprompt_ai::services::tooled::ToolResultFormatter;
use systemprompt_ai::models::tools::{CallToolResult, ToolCall};
use systemprompt_identifiers::AiToolCallId;
use rmcp::model::{Annotated, Content, RawContent, RawTextContent};
use serde_json::json;

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
    CallToolResult {
        content: vec![create_text_content(text)],
        structured_content: None,
        is_error: Some(false),
        meta: None,
    }
}

fn create_error_result(text: &str) -> CallToolResult {
    CallToolResult {
        content: vec![create_text_content(text)],
        structured_content: None,
        is_error: Some(true),
        meta: None,
    }
}

mod format_for_ai_tests {
    use super::*;

    #[test]
    fn formats_single_success() {
        let calls = vec![create_tool_call("search")];
        let results = vec![create_success_result("Found 10 results")];

        let formatted = ToolResultFormatter::format_for_ai(&calls, &results);

        assert!(formatted.contains("search"));
        assert!(formatted.contains("SUCCESS"));
        assert!(formatted.contains("Found 10 results"));
    }

    #[test]
    fn formats_single_failure() {
        let calls = vec![create_tool_call("fetch")];
        let results = vec![create_error_result("Connection timeout")];

        let formatted = ToolResultFormatter::format_for_ai(&calls, &results);

        assert!(formatted.contains("fetch"));
        assert!(formatted.contains("FAILED"));
        assert!(formatted.contains("Connection timeout"));
    }

    #[test]
    fn formats_multiple_results() {
        let calls = vec![
            create_tool_call("tool1"),
            create_tool_call("tool2"),
        ];
        let results = vec![
            create_success_result("Result 1"),
            create_success_result("Result 2"),
        ];

        let formatted = ToolResultFormatter::format_for_ai(&calls, &results);

        assert!(formatted.contains("tool1"));
        assert!(formatted.contains("tool2"));
        assert!(formatted.contains("Result 1"));
        assert!(formatted.contains("Result 2"));
    }

    #[test]
    fn truncates_long_content() {
        let long_text = "x".repeat(1000);
        let calls = vec![create_tool_call("long")];
        let results = vec![create_success_result(&long_text)];

        let formatted = ToolResultFormatter::format_for_ai(&calls, &results);

        // Should be truncated (500 chars + "...")
        assert!(formatted.len() < 1000);
        assert!(formatted.contains("..."));
    }
}

mod format_for_synthesis_tests {
    use super::*;

    #[test]
    fn includes_tool_name() {
        let calls = vec![create_tool_call("calculator")];
        let results = vec![create_success_result("42")];

        let formatted = ToolResultFormatter::format_for_synthesis(&calls, &results);

        assert!(formatted.contains("calculator"));
    }

    #[test]
    fn includes_status() {
        let calls = vec![create_tool_call("test")];
        let results = vec![create_success_result("ok")];

        let formatted = ToolResultFormatter::format_for_synthesis(&calls, &results);

        assert!(formatted.contains("SUCCESS"));
    }

    #[test]
    fn includes_summary() {
        let calls = vec![create_tool_call("test")];
        let results = vec![create_success_result("This is the summary line\nMore details here")];

        let formatted = ToolResultFormatter::format_for_synthesis(&calls, &results);

        assert!(formatted.contains("Summary"));
    }

    #[test]
    fn adds_completion_note_for_success() {
        let calls = vec![create_tool_call("action")];
        let results = vec![create_success_result("Done")];

        let formatted = ToolResultFormatter::format_for_synthesis(&calls, &results);

        assert!(formatted.contains("IMPORTANT"));
        assert!(formatted.contains("completed successfully"));
    }

    #[test]
    fn no_completion_note_for_failure() {
        let calls = vec![create_tool_call("action")];
        let results = vec![create_error_result("Failed")];

        let formatted = ToolResultFormatter::format_for_synthesis(&calls, &results);

        assert!(!formatted.contains("completed successfully"));
    }

    #[test]
    fn separates_multiple_results() {
        let calls = vec![
            create_tool_call("a"),
            create_tool_call("b"),
        ];
        let results = vec![
            create_success_result("A result"),
            create_success_result("B result"),
        ];

        let formatted = ToolResultFormatter::format_for_synthesis(&calls, &results);

        assert!(formatted.contains("---"));
    }
}

mod format_for_display_tests {
    use super::*;

    #[test]
    fn includes_index() {
        let calls = vec![
            create_tool_call("first"),
            create_tool_call("second"),
        ];
        let results = vec![
            create_success_result("1"),
            create_success_result("2"),
        ];

        let formatted = ToolResultFormatter::format_for_display(&calls, &results);

        assert!(formatted.contains("1."));
        assert!(formatted.contains("2."));
    }

    #[test]
    fn shows_preview() {
        let calls = vec![create_tool_call("test")];
        let results = vec![create_success_result("Short preview")];

        let formatted = ToolResultFormatter::format_for_display(&calls, &results);

        assert!(formatted.contains("Short preview"));
    }

    #[test]
    fn truncates_preview() {
        let long_text = "x".repeat(500);
        let calls = vec![create_tool_call("long")];
        let results = vec![create_success_result(&long_text)];

        let formatted = ToolResultFormatter::format_for_display(&calls, &results);

        // Should be truncated (200 chars)
        assert!(formatted.len() < 500);
    }
}

mod format_fallback_summary_tests {
    use super::*;

    #[test]
    fn includes_successful_results() {
        let calls = vec![create_tool_call("good")];
        let results = vec![create_success_result("Important data")];

        let formatted = ToolResultFormatter::format_fallback_summary(&calls, &results);

        assert!(formatted.contains("good"));
        assert!(formatted.contains("Important data"));
    }

    #[test]
    fn excludes_failed_results() {
        let calls = vec![create_tool_call("bad")];
        let results = vec![create_error_result("Error message")];

        let formatted = ToolResultFormatter::format_fallback_summary(&calls, &results);

        // Should not include failed result content
        assert!(!formatted.contains("Error message"));
    }

    #[test]
    fn returns_default_for_all_failures() {
        let calls = vec![create_tool_call("fail")];
        let results = vec![create_error_result("Error")];

        let formatted = ToolResultFormatter::format_fallback_summary(&calls, &results);

        assert!(formatted.contains("completed"));
    }

    #[test]
    fn handles_empty_results() {
        let calls: Vec<ToolCall> = vec![];
        let results: Vec<CallToolResult> = vec![];

        let formatted = ToolResultFormatter::format_fallback_summary(&calls, &results);

        assert!(formatted.contains("completed"));
    }

    #[test]
    fn combines_multiple_successes() {
        let calls = vec![
            create_tool_call("first"),
            create_tool_call("second"),
        ];
        let results = vec![
            create_success_result("First result"),
            create_success_result("Second result"),
        ];

        let formatted = ToolResultFormatter::format_fallback_summary(&calls, &results);

        assert!(formatted.contains("first"));
        assert!(formatted.contains("second"));
        assert!(formatted.contains("First result"));
        assert!(formatted.contains("Second result"));
    }

    #[test]
    fn mixed_success_and_failure() {
        let calls = vec![
            create_tool_call("success"),
            create_tool_call("failure"),
        ];
        let results = vec![
            create_success_result("Good result"),
            create_error_result("Bad result"),
        ];

        let formatted = ToolResultFormatter::format_fallback_summary(&calls, &results);

        assert!(formatted.contains("success"));
        assert!(formatted.contains("Good result"));
        assert!(!formatted.contains("Bad result"));
    }
}
