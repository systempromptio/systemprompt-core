#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::infrastructure::logs::{
    AuditOutput, LogsSummaryOutput, audit_not_found, build_audit, build_logs_summary,
};
use systemprompt_models::artifacts::CliArtifact;

#[test]
fn logs_summary_returns_card() {
    let summary: LogsSummaryOutput = serde_json::from_value(serde_json::json!({
        "total_logs": 5,
        "by_level": { "error": 1, "warn": 1, "info": 2, "debug": 1, "trace": 0 },
        "top_modules": [{ "module": "agent", "count": 3 }],
        "time_range": {},
        "database_info": { "logs_table_rows": 5 }
    }))
    .unwrap();
    let output = build_logs_summary(&summary);
    assert!(matches!(
        output.artifact(),
        CliArtifact::PresentationCard { .. }
    ));
}

#[test]
fn audit_returns_card() {
    let audit: AuditOutput = serde_json::from_value(serde_json::json!({
        "request_id": "req_abc123",
        "provider": "anthropic",
        "model": "claude",
        "input_tokens": 10,
        "output_tokens": 20,
        "cost_dollars": 0.0001,
        "latency_ms": 42,
        "task_id": null,
        "trace_id": null,
        "messages": [],
        "tool_calls": []
    }))
    .unwrap();
    let output = build_audit(&audit);
    assert!(matches!(
        output.artifact(),
        CliArtifact::PresentationCard { .. }
    ));
}

#[test]
fn audit_not_found_returns_message() {
    let output = audit_not_found("missing");
    assert!(matches!(output.artifact(), CliArtifact::Message { .. }));
}
