//! Unit tests for tool-call models and the tool-result formatter.
//!
//! Covers [`ToolCall`] serde, plus the AI/synthesis/display/fallback
//! formatting helpers on [`ToolResultFormatter`].

use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::json;
use systemprompt_identifiers::AiToolCallId;
use systemprompt_models::ai::tool_result_formatter::ToolResultFormatter;
use systemprompt_models::ai::tools::ToolCall;

fn sample_call(name: &str) -> ToolCall {
    ToolCall {
        ai_tool_call_id: AiToolCallId::new("call-1"),
        name: name.to_owned(),
        arguments: json!({"q": "value"}),
    }
}

fn ok_result(text: &str) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(text.to_owned())])
}

fn err_result(text: &str) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(text.to_owned())])
}

// ---------- ToolCall serde ----------

#[test]
fn tool_call_serde_roundtrip() {
    let call = sample_call("search");
    let v = serde_json::to_value(&call).unwrap();
    assert_eq!(v["name"], "search");
    assert_eq!(v["ai_tool_call_id"], "call-1");
    assert_eq!(v["arguments"]["q"], "value");
    let back: ToolCall = serde_json::from_value(v).unwrap();
    assert_eq!(back.name, "search");
    assert_eq!(back.ai_tool_call_id, AiToolCallId::new("call-1"));
}

// ---------- ToolResultFormatter ----------

#[test]
fn format_single_for_ai_success() {
    let out = ToolResultFormatter::format_single_for_ai(&sample_call("search"), &ok_result("hi"));
    assert!(out.contains("Tool 'search'"));
    assert!(out.contains("[SUCCESS]"));
    assert!(out.contains("hi"));
}

#[test]
fn format_single_for_ai_failure() {
    let out =
        ToolResultFormatter::format_single_for_ai(&sample_call("search"), &err_result("boom"));
    assert!(out.contains("[FAILED]"));
}

#[test]
fn format_for_ai_joins_multiple() {
    let calls = vec![sample_call("a"), sample_call("b")];
    let results = vec![ok_result("one"), ok_result("two")];
    let out = ToolResultFormatter::format_for_ai(&calls, &results);
    assert!(out.contains("Tool 'a'"));
    assert!(out.contains("Tool 'b'"));
    assert_eq!(out.lines().count(), 2);
}

#[test]
fn format_single_for_synthesis_success_has_completion_note() {
    let out = ToolResultFormatter::format_single_for_synthesis(
        &sample_call("search"),
        &ok_result("First line\nsecond"),
    );
    assert!(out.contains("### Tool: search [SUCCESS]"));
    assert!(out.contains("**Summary**: First line"));
    assert!(out.contains("completed successfully"));
}

#[test]
fn format_single_for_synthesis_failure_no_completion_note() {
    let out = ToolResultFormatter::format_single_for_synthesis(
        &sample_call("search"),
        &err_result("error detail"),
    );
    assert!(out.contains("[FAILED]"));
    assert!(!out.contains("completed successfully"));
}

#[test]
fn format_for_synthesis_uses_separator() {
    let calls = vec![sample_call("a"), sample_call("b")];
    let results = vec![ok_result("x"), ok_result("y")];
    let out = ToolResultFormatter::format_for_synthesis(&calls, &results);
    assert!(out.contains("\n---\n\n"));
}

#[test]
fn format_for_display_numbers_entries() {
    let calls = vec![sample_call("a"), sample_call("b")];
    let results = vec![ok_result("x"), ok_result("y")];
    let out = ToolResultFormatter::format_for_display(&calls, &results);
    assert!(out.contains("1. a [SUCCESS]: x"));
    assert!(out.contains("2. b [SUCCESS]: y"));
}

#[test]
fn format_fallback_summary_skips_errors() {
    let calls = vec![sample_call("ok"), sample_call("bad")];
    let results = vec![ok_result("good output"), err_result("ignored")];
    let out = ToolResultFormatter::format_fallback_summary(&calls, &results);
    assert!(out.contains("**ok**"));
    assert!(out.contains("good output"));
    assert!(!out.contains("ignored"));
}

#[test]
fn format_fallback_summary_empty_when_all_errors() {
    let calls = vec![sample_call("bad")];
    let results = vec![err_result("nope")];
    let out = ToolResultFormatter::format_fallback_summary(&calls, &results);
    assert_eq!(out, "Tool execution completed.");
}
