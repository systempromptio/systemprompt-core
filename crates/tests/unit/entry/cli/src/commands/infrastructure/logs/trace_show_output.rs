//! Tests for `infra logs trace show` output assembly: event delta timing,
//! cost conversion, and step-derived status.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use chrono::{Duration, TimeZone, Utc};
use systemprompt_cli::infrastructure::logs::trace::show::{TraceSummaries, build_trace_output};
use systemprompt_identifiers::TaskId;
use systemprompt_logging::{
    AiRequestSummary, ExecutionStepSummary, McpExecutionSummary, TraceEvent,
};

fn event(offset_ms: i64, event_type: &str) -> TraceEvent {
    TraceEvent {
        event_type: event_type.to_string(),
        timestamp: Utc.with_ymd_and_hms(2026, 7, 22, 10, 0, 0).unwrap()
            + Duration::milliseconds(offset_ms),
        details: format!("{event_type} details"),
        user_id: None,
        session_id: None,
        task_id: None,
        context_id: None,
        metadata: None,
    }
}

fn summaries<'a>(
    ai: &'a AiRequestSummary,
    mcp: &'a McpExecutionSummary,
    step: &'a ExecutionStepSummary,
) -> TraceSummaries<'a> {
    TraceSummaries { ai, mcp, step }
}

#[test]
fn build_trace_output_computes_deltas_from_first_event() {
    let ai = AiRequestSummary::default();
    let mcp = McpExecutionSummary::default();
    let step = ExecutionStepSummary::default();
    let events = vec![event(0, "log"), event(250, "ai_request"), event(900, "log")];

    let output = build_trace_output("trace-1", &events, &summaries(&ai, &mcp, &step), None, None);

    assert_eq!(output.trace_id.as_str(), "trace-1");
    let deltas: Vec<i64> = output.events.iter().map(|e| e.delta_ms).collect();
    assert_eq!(deltas, vec![0, 250, 900]);
    assert!(output.events[0].timestamp.starts_with("2026-07-22 10:00:00"));
    assert!(output.task.is_none());
}

#[test]
fn build_trace_output_converts_microdollars_and_sums_tokens() {
    let ai = AiRequestSummary {
        total_cost_microdollars: 1_500_000,
        total_tokens: 0,
        total_input_tokens: 120,
        total_output_tokens: 30,
        request_count: 2,
        total_latency_ms: 640,
    };
    let mcp = McpExecutionSummary {
        execution_count: 3,
        total_execution_time_ms: 75,
    };
    let step = ExecutionStepSummary::default();
    let task_id = TaskId::generate();

    let output = build_trace_output(
        "trace-2",
        &[],
        &summaries(&ai, &mcp, &step),
        Some(&task_id),
        Some(1234),
    );

    assert_eq!(output.ai_summary.total_tokens, 150);
    assert_eq!(output.ai_summary.cost_dollars, 1.5);
    assert_eq!(output.ai_summary.request_count, 2);
    assert_eq!(output.mcp_summary.execution_count, 3);
    assert_eq!(output.task.as_deref(), Some(task_id.as_str()));
    assert_eq!(output.duration_ms, Some(1234));
}

#[test]
fn build_trace_output_derives_status_from_step_counts() {
    let ai = AiRequestSummary::default();
    let mcp = McpExecutionSummary::default();

    let failed = ExecutionStepSummary {
        total: 3,
        completed: 1,
        failed: 1,
        pending: 1,
    };
    let out = build_trace_output("t", &[], &summaries(&ai, &mcp, &failed), None, None);
    assert_eq!(out.status, "failed");

    let pending = ExecutionStepSummary {
        total: 2,
        completed: 1,
        failed: 0,
        pending: 1,
    };
    let out = build_trace_output("t", &[], &summaries(&ai, &mcp, &pending), None, None);
    assert_eq!(out.status, "in_progress");

    let done = ExecutionStepSummary::default();
    let out = build_trace_output("t", &[], &summaries(&ai, &mcp, &done), None, None);
    assert_eq!(out.status, "completed");
}
