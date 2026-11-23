//! Tests for `presentation::tables` — the shared table widgets that shape
//! command records into rendered `tabled` output.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use chrono::{TimeZone, Utc};
use systemprompt_cli::presentation::tables::{
    ai_requests_table, execution_steps_table,
    extract_latency_from_metadata, format_metadata_value, mcp_tool_calls_table,
    task_artifacts_table, task_info_table, trace_events_table, truncate_cell,
};
use systemprompt_identifiers::{
    AgentName, AiRequestId, ArtifactId, ContextId, ExecutionStepId, McpExecutionId, McpServerId,
    McpToolName, TaskId,
};
use systemprompt_runtime::{
    AiRequestInfo, ExecutionStep, McpToolExecution, TaskArtifact, TaskInfo, TraceEvent,
};

fn ts(secs: i64) -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(1_700_000_000 + secs, 0).unwrap()
}

#[test]
fn truncate_cell_passes_short_strings_through() {
    assert_eq!(truncate_cell("short", 10), "short");
}

#[test]
fn truncate_cell_appends_ellipsis_when_cut() {
    assert_eq!(truncate_cell("abcdefghij", 8), "abcde...");
}

#[test]
fn truncate_cell_flattens_newlines() {
    assert_eq!(truncate_cell("a\nb\r\nc", 20), "a b c");
}

#[test]
fn task_info_table_shows_duration_and_truncated_id() {
    let info = TaskInfo {
        task_id: TaskId::new("task-1234567890"),
        context_id: ContextId::generate(),
        agent_name: Some(AgentName::new("agent")),
        status: "completed".to_owned(),
        created_at: ts(0),
        started_at: Some(ts(1)),
        completed_at: Some(ts(2)),
        execution_time_ms: Some(1500),
        error_message: None,
    };
    let table = task_info_table(&info);
    assert!(table.contains("task-123"));
    assert!(!table.contains("task-1234567890"));
    assert!(table.contains("1500ms"));
    assert!(table.contains("completed"));
}

#[test]
fn task_info_table_dashes_missing_fields() {
    let info = TaskInfo {
        task_id: TaskId::new("t1"),
        context_id: ContextId::generate(),
        agent_name: None,
        status: "pending".to_owned(),
        created_at: ts(0),
        started_at: None,
        completed_at: None,
        execution_time_ms: None,
        error_message: None,
    };
    let table = task_info_table(&info);
    assert!(table.contains('-'));
    assert!(table.contains("pending"));
}

#[test]
fn execution_steps_table_numbers_rows_and_defaults_type() {
    let steps = vec![
        ExecutionStep {
            step_id: ExecutionStepId::new("s1"),
            step_type: None,
            title: Some("first step".to_owned()),
            status: "completed".to_owned(),
            duration_ms: Some(10),
            error_message: None,
        },
        ExecutionStep {
            step_id: ExecutionStepId::new("s2"),
            step_type: Some("tool_call".to_owned()),
            title: None,
            status: "failed".to_owned(),
            duration_ms: None,
            error_message: Some("boom".to_owned()),
        },
    ];
    let table = execution_steps_table(&steps);
    assert!(table.contains("unknown"));
    assert!(table.contains("tool_call"));
    assert!(table.contains("first step"));
    assert!(table.contains("10ms"));
    assert!(table.contains('1'));
    assert!(table.contains('2'));
}

#[test]
fn ai_requests_table_sums_tokens_and_formats_cost() {
    let requests = vec![AiRequestInfo {
        id: AiRequestId::new("req-1"),
        provider: Some("anthropic".to_owned()),
        model: Some("claude".to_owned()),
        max_tokens: Some(1024),
        input_tokens: Some(100),
        output_tokens: Some(50),
        cost_microdollars: 2_500_000,
        latency_ms: Some(321),
    }];
    let table = ai_requests_table(&requests);
    assert!(table.contains("anthropic/claude"));
    assert!(table.contains("150 (in:100, out:50)"));
    assert!(table.contains("$2.5000"));
    assert!(table.contains("321ms"));
    assert!(table.contains("1024"));
}

#[test]
fn ai_requests_table_defaults_missing_numbers() {
    let requests = vec![AiRequestInfo {
        id: AiRequestId::new("req-2"),
        provider: Some("openai".to_owned()),
        model: Some("gpt".to_owned()),
        max_tokens: None,
        input_tokens: None,
        output_tokens: None,
        cost_microdollars: 0,
        latency_ms: None,
    }];
    let table = ai_requests_table(&requests);
    assert!(table.contains("0 (in:0, out:0)"));
    assert!(table.contains("$0.0000"));
}

#[test]
fn mcp_tool_calls_table_renders_status_and_duration() {
    let executions = vec![McpToolExecution {
        mcp_execution_id: McpExecutionId::new("m1"),
        tool_name: McpToolName::new("search"),
        server_name: McpServerId::new("content"),
        status: "completed".to_owned(),
        execution_time_ms: Some(88),
        error_message: None,
        input: String::new(),
        output: None,
    }];
    let table = mcp_tool_calls_table(&executions);
    assert!(table.contains("search"));
    assert!(table.contains("content"));
    assert!(table.contains("88ms"));
}

fn artifact(id: &str, name: Option<&str>) -> TaskArtifact {
    TaskArtifact {
        artifact_id: ArtifactId::new(id),
        artifact_type: "document".to_owned(),
        name: name.map(str::to_owned),
        source: None,
        tool_name: None,
        part_kind: None,
        text_content: None,
        data_content: None,
    }
}

#[test]
fn task_artifacts_table_deduplicates_by_id() {
    let artifacts = vec![
        artifact("a1", Some("one")),
        artifact("a1", Some("one")),
        artifact("a2", Some("two")),
    ];
    let table = task_artifacts_table(&artifacts);
    assert_eq!(table.matches("one").count(), 1);
    assert!(table.contains("two"));
}

#[test]
fn trace_events_table_computes_deltas() {
    let events = vec![
        TraceEvent {
            event_type: "LOG".to_owned(),
            timestamp: ts(0),
            details: "first".to_owned(),
            user_id: None,
            session_id: None,
            task_id: None,
            context_id: None,
            metadata: None,
        },
        TraceEvent {
            event_type: "AI".to_owned(),
            timestamp: ts(2),
            details: "second".to_owned(),
            user_id: None,
            session_id: None,
            task_id: None,
            context_id: None,
            metadata: Some(r#"{"latency_ms": 45}"#.to_owned()),
        },
    ];
    let table = trace_events_table(&events);
    assert!(table.contains("+0ms"));
    assert!(table.contains("+2000ms"));
    assert!(table.contains("45ms"));
    assert!(table.contains("first"));
}

#[test]
fn trace_events_table_is_empty_for_no_events() {
    assert_eq!(trace_events_table(&[]), "");
}

#[test]
fn extract_latency_reads_ai_and_mcp_keys() {
    assert_eq!(
        extract_latency_from_metadata(Some(r#"{"latency_ms": 12}"#), "AI"),
        "12ms"
    );
    assert_eq!(
        extract_latency_from_metadata(Some(r#"{"execution_time_ms": 30}"#), "MCP"),
        "30ms"
    );
    assert_eq!(
        extract_latency_from_metadata(Some(r#"{"latency_ms": 12}"#), "MCP"),
        "-"
    );
    assert_eq!(extract_latency_from_metadata(None, "AI"), "-");
    assert_eq!(extract_latency_from_metadata(Some("not json"), "AI"), "-");
}

#[test]
fn format_metadata_value_applies_unit_formatting() {
    use serde_json::json;
    assert_eq!(
        format_metadata_value("cost_microdollars", &json!(1_500_000)),
        "$1.500000"
    );
    assert_eq!(format_metadata_value("latency_ms", &json!(25)), "25ms");
    assert_eq!(
        format_metadata_value("execution_time_ms", &json!(30)),
        "30ms"
    );
    assert_eq!(format_metadata_value("tokens_used", &json!(99)), "99");
    assert_eq!(format_metadata_value("other_key", &json!("plain")), "plain");
    assert_eq!(
        format_metadata_value("latency_ms", &json!("weird")),
        "weird"
    );
}
