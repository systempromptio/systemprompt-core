//! Unit tests for tool-call models and the tool-result formatter.
//!
//! Covers [`ToolCall`]/[`ToolExecution`] serde and `from_json_row` parsing
//! (including the missing/out-of-range error paths), plus the AI/synthesis/
//! display/fallback formatting helpers on [`ToolResultFormatter`].

use rmcp::model::{CallToolResult, Content};
use serde_json::{Value as JsonValue, json};
use std::collections::HashMap;
use systemprompt_identifiers::AiToolCallId;
use systemprompt_models::ai::tool_result_formatter::ToolResultFormatter;
use systemprompt_models::ai::tools::{ToolCall, ToolExecution};
use systemprompt_models::errors::RowParseError;

fn sample_call(name: &str) -> ToolCall {
    ToolCall {
        ai_tool_call_id: AiToolCallId::new("call-1"),
        name: name.to_owned(),
        arguments: json!({"q": "value"}),
    }
}

fn ok_result(text: &str) -> CallToolResult {
    CallToolResult::success(vec![Content::text(text.to_owned())])
}

fn err_result(text: &str) -> CallToolResult {
    CallToolResult::error(vec![Content::text(text.to_owned())])
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

// ---------- ToolExecution::from_json_row ----------

fn full_row() -> HashMap<String, JsonValue> {
    let mut row = HashMap::new();
    row.insert("id".to_owned(), json!("exec-1"));
    row.insert("request_id".to_owned(), json!("req-1"));
    row.insert("sequence".to_owned(), json!(3));
    row.insert("tool_name".to_owned(), json!("search"));
    row.insert("service_id".to_owned(), json!("svc-1"));
    row.insert("input".to_owned(), json!("{\"a\":1}"));
    row.insert("output".to_owned(), json!("{\"b\":2}"));
    row.insert("status".to_owned(), json!("completed"));
    row.insert("execution_time_ms".to_owned(), json!(42));
    row.insert("error_message".to_owned(), json!("none"));
    row.insert("created_at".to_owned(), json!("2026-06-22T10:00:00Z"));
    row
}

#[test]
fn from_json_row_parses_all_fields() {
    let exec = ToolExecution::from_json_row(&full_row()).unwrap();
    assert_eq!(exec.id.as_str(), "exec-1");
    assert_eq!(exec.request_id.as_str(), "req-1");
    assert_eq!(exec.sequence, 3);
    assert_eq!(exec.tool_name, "search");
    assert_eq!(exec.service_id.as_str(), "svc-1");
    assert_eq!(exec.input, json!({"a": 1}));
    assert_eq!(exec.output, Some(json!({"b": 2})));
    assert_eq!(exec.status, "completed");
    assert_eq!(exec.execution_time_ms, Some(42));
    assert_eq!(exec.error_message.as_deref(), Some("none"));
}

#[test]
fn from_json_row_missing_id() {
    let mut row = full_row();
    row.remove("id");
    let err = ToolExecution::from_json_row(&row).unwrap_err();
    assert!(matches!(err, RowParseError::Missing("id")));
}

#[test]
fn from_json_row_missing_status() {
    let mut row = full_row();
    row.remove("status");
    let err = ToolExecution::from_json_row(&row).unwrap_err();
    assert!(matches!(err, RowParseError::Missing("status")));
}

#[test]
fn from_json_row_missing_created_at() {
    let mut row = full_row();
    row.remove("created_at");
    let err = ToolExecution::from_json_row(&row).unwrap_err();
    assert!(matches!(err, RowParseError::Missing("created_at")));
}

#[test]
fn from_json_row_sequence_out_of_range() {
    let mut row = full_row();
    row.insert("sequence".to_owned(), json!(i64::from(i32::MAX) + 1));
    let err = ToolExecution::from_json_row(&row).unwrap_err();
    assert!(matches!(err, RowParseError::OutOfRange("sequence")));
}

#[test]
fn from_json_row_invalid_json_input_falls_back_to_null() {
    let mut row = full_row();
    row.insert("input".to_owned(), json!("not valid json {"));
    row.remove("output");
    let exec = ToolExecution::from_json_row(&row).unwrap();
    assert_eq!(exec.input, JsonValue::Null);
    assert_eq!(exec.output, None);
}

#[test]
fn from_json_row_optional_fields_absent() {
    let mut row = full_row();
    row.remove("execution_time_ms");
    row.remove("error_message");
    let exec = ToolExecution::from_json_row(&row).unwrap();
    assert_eq!(exec.execution_time_ms, None);
    assert_eq!(exec.error_message, None);
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
