//! The structured trace view built from a trace's events and summaries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{TaskId, TraceId};
use systemprompt_runtime::TraceEvent;

use super::{AiSummaryRow, McpSummaryRow, StepSummaryRow, TraceEventRow, TraceViewOutput};

#[derive(Debug)]
pub struct TraceSummaries<'a> {
    pub ai: &'a systemprompt_runtime::AiRequestSummary,
    pub mcp: &'a systemprompt_runtime::McpExecutionSummary,
    pub step: &'a systemprompt_runtime::ExecutionStepSummary,
}

pub fn build_trace_output(
    trace_id: &TraceId,
    events: &[TraceEvent],
    summaries: &TraceSummaries<'_>,
    task_id: Option<&TaskId>,
    duration_ms: Option<i64>,
) -> TraceViewOutput {
    let first_timestamp = events.first().map(|e| e.timestamp);

    let event_rows: Vec<TraceEventRow> = events
        .iter()
        .map(|e| {
            let delta_ms =
                first_timestamp.map_or(0, |first| (e.timestamp - first).num_milliseconds());
            TraceEventRow {
                timestamp: e.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                delta_ms,
                event_type: e.event_type.clone(),
                details: e.details.clone(),
                latency_ms: None,
            }
        })
        .collect();

    let cost_dollars = f64::from(summaries.ai.total_cost_microdollars as i32) / 1_000_000.0;

    let status = if summaries.step.failed > 0 {
        "failed".to_owned()
    } else if summaries.step.pending > 0 {
        "in_progress".to_owned()
    } else {
        "completed".to_owned()
    };

    TraceViewOutput {
        trace_id: trace_id.clone(),
        events: event_rows,
        ai_summary: AiSummaryRow {
            request_count: summaries.ai.request_count,
            total_tokens: summaries.ai.total_input_tokens + summaries.ai.total_output_tokens,
            input_tokens: summaries.ai.total_input_tokens,
            output_tokens: summaries.ai.total_output_tokens,
            cost_dollars,
            total_latency_ms: summaries.ai.total_latency_ms,
        },
        mcp_summary: McpSummaryRow {
            execution_count: summaries.mcp.execution_count,
            total_execution_time_ms: summaries.mcp.total_execution_time_ms,
        },
        step_summary: StepSummaryRow {
            total: summaries.step.total,
            completed: summaries.step.completed,
            failed: summaries.step.failed,
            pending: summaries.step.pending,
        },
        task: task_id.map(|t| t.as_str().to_owned()),
        duration_ms,
        status,
    }
}
