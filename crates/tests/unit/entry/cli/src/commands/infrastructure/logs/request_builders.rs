#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::infrastructure::logs::request::{
    RequestListRow, RequestShowOutput, RequestStatsOutput, build_request_list, build_request_show,
    build_request_stats, request_show_not_found,
};
use systemprompt_cli::infrastructure::logs::{MessageRow, ToolCallRow};
use systemprompt_models::artifacts::CliArtifact;

fn sample_row() -> RequestListRow {
    RequestListRow {
        request_id: "req_abc123".to_owned(),
        timestamp: "2026-06-03 10:00:00".to_owned(),
        provider: "anthropic".to_owned(),
        model: "claude".to_owned(),
        tokens: "10/20".to_owned(),
        cost: "$0.000100".to_owned(),
        latency_ms: Some(42),
        status: "success".to_owned(),
    }
}

#[test]
fn request_list_returns_table() {
    let output = build_request_list(&[sample_row()]);
    assert!(matches!(output.artifact(), CliArtifact::Table { .. }));
    assert!(!output.should_skip_render());
}

#[test]
fn request_list_empty_returns_message() {
    let output = build_request_list(&[]);
    assert!(matches!(output.artifact(), CliArtifact::Message { .. }));
}

#[test]
fn request_show_returns_card() {
    let detail = RequestShowOutput {
        request_id: "req_abc123".to_owned(),
        provider: "anthropic".to_owned(),
        model: "claude".to_owned(),
        input_tokens: 10,
        output_tokens: 20,
        cost_dollars: 0.0001,
        latency_ms: 42,
        status: "success".to_owned(),
        error_message: None,
        messages: vec![MessageRow {
            sequence: 1,
            role: "user".to_owned(),
            content: "hello".to_owned(),
        }],
        linked_mcp_calls: vec![ToolCallRow {
            tool_name: "systemprompt".to_owned(),
            server: "systemprompt".to_owned(),
            status: "success".to_owned(),
            duration_ms: Some(5),
        }],
    };
    let output = build_request_show(&detail);
    assert!(matches!(
        output.artifact(),
        CliArtifact::PresentationCard { .. }
    ));
}

#[test]
fn request_show_not_found_returns_message() {
    let output = request_show_not_found("missing");
    assert!(matches!(output.artifact(), CliArtifact::Message { .. }));
}

#[test]
fn request_stats_returns_card() {
    let stats: RequestStatsOutput = serde_json::from_value(serde_json::json!({
        "total_requests": 3,
        "total_tokens": { "input": 30, "output": 60, "total": 90 },
        "total_cost_dollars": 0.0003,
        "average_latency_ms": 40,
        "by_provider": [],
        "by_model": []
    }))
    .unwrap();
    let output = build_request_stats(&stats);
    assert!(matches!(
        output.artifact(),
        CliArtifact::PresentationCard { .. }
    ));
}
