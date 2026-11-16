//! Trace summary rendering: event counts, AI and MCP rollups.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use systemprompt_identifiers::TaskId;
use systemprompt_models::artifacts::NoticeLine;
use systemprompt_runtime::{
    AiRequestSummary, ExecutionStepSummary, McpExecutionSummary, TraceEvent,
};

#[derive(Debug)]
pub struct SummaryContext<'a> {
    pub events: &'a [TraceEvent],
    pub first: Option<DateTime<Utc>>,
    pub last: Option<DateTime<Utc>>,
    pub task_id: Option<&'a TaskId>,
    pub ai_summary: &'a AiRequestSummary,
    pub mcp_summary: &'a McpExecutionSummary,
    pub step_summary: &'a ExecutionStepSummary,
}

pub fn push_summary(out: &mut Vec<NoticeLine>, ctx: &SummaryContext<'_>) {
    line(out, "info", "Summary");

    if let (Some(first), Some(last)) = (ctx.first, ctx.last) {
        let duration = last.signed_duration_since(first);
        kv(out, "  Duration", &format!("{}ms", duration.num_milliseconds()));
    }

    push_event_counts(out, ctx.events);
    push_ai_summary(out, ctx.ai_summary);
    push_mcp_summary(out, ctx.mcp_summary);
    push_step_summary(out, ctx.step_summary);
    push_trace_context(out, ctx.events, ctx.task_id);
    push_status(out, ctx.events);
}

fn push_event_counts(out: &mut Vec<NoticeLine>, events: &[TraceEvent]) {
    let mut event_counts: HashMap<String, usize> = HashMap::new();
    for event in events {
        *event_counts.entry(event.event_type.clone()).or_insert(0) += 1;
    }

    let mut count_vec: Vec<_> = event_counts.iter().collect();
    count_vec.sort_by_key(|&(k, _)| k);

    let event_parts: Vec<String> = count_vec
        .iter()
        .map(|(k, v)| format!("{} {}", v, k))
        .collect();
    kv(out, 
        "  Events",
        &format!("{} ({})", events.len(), event_parts.join(", ")),
    );
}

fn push_ai_summary(out: &mut Vec<NoticeLine>, ai_summary: &AiRequestSummary) {
    if ai_summary.request_count > 0 {
        line(out, "info", "  AI Requests:");
        kv(out, "     Requests", &ai_summary.request_count.to_string());
        kv(out, 
            "     Tokens",
            &format!(
                "{} (in: {}, out: {})",
                ai_summary.total_tokens,
                ai_summary.total_input_tokens,
                ai_summary.total_output_tokens
            ),
        );
        let dollars = ai_summary.total_cost_microdollars as f64 / 1_000_000.0;
        kv(out, "     Cost", &format!("${dollars:.6}"));
        kv(out, 
            "     Total Latency",
            &format!("{}ms", ai_summary.total_latency_ms),
        );
        if ai_summary.request_count > 0 {
            let avg_latency = ai_summary.total_latency_ms / ai_summary.request_count;
            kv(out, "     Avg Latency", &format!("{avg_latency}ms"));
        }
    }
}

fn push_mcp_summary(out: &mut Vec<NoticeLine>, mcp_summary: &McpExecutionSummary) {
    if mcp_summary.execution_count > 0 {
        line(out, "info", "  MCP Tool Executions:");
        kv(out, "     Executions", &mcp_summary.execution_count.to_string());
        kv(out, 
            "     Total Time",
            &format!("{}ms", mcp_summary.total_execution_time_ms),
        );
        if mcp_summary.execution_count > 0 {
            let avg_time = mcp_summary.total_execution_time_ms / mcp_summary.execution_count;
            kv(out, "     Avg Time", &format!("{avg_time}ms"));
        }
    }
}

fn push_step_summary(out: &mut Vec<NoticeLine>, step_summary: &ExecutionStepSummary) {
    if step_summary.total > 0 {
        line(out, "info", "  Execution Steps:");
        kv(out, 
            "     Steps",
            &format!(
                "{} ({} completed, {} failed, {} pending)",
                step_summary.total,
                step_summary.completed,
                step_summary.failed,
                step_summary.pending
            ),
        );
    }
}

fn push_trace_context(out: &mut Vec<NoticeLine>, events: &[TraceEvent], task_id: Option<&TaskId>) {
    if let Some(task_id) = task_id {
        kv(out, 
            "  Task",
            &format!(
                "{} (use: systemprompt infra logs trace show <task_id>)",
                task_id.as_str()
            ),
        );
    }

    if let Some(session_id) = events.first().and_then(|e| e.session_id.as_ref()) {
        kv(out, "  Session", session_id.as_str());
    }

    if let Some(user_id) = events.first().and_then(|e| e.user_id.as_ref()) {
        kv(out, "  User", user_id.as_str());
    }
}

fn push_status(out: &mut Vec<NoticeLine>, events: &[TraceEvent]) {
    let has_errors = events.iter().any(|e| {
        e.details.contains("ERROR")
            || e.details.contains("failed")
            || e.details.contains("(failed)")
    });

    if has_errors {
        line(out, "error", "  Status: [FAILED]");
    } else {
        line(out, "success", "  Status: [OK]");
    }
}

pub(super) fn line(out: &mut Vec<NoticeLine>, level: &str, text: impl AsRef<str>) {
    out.push(NoticeLine::new(level, text.as_ref()));
}

fn kv(out: &mut Vec<NoticeLine>, key: &str, value: impl AsRef<str>) {
    out.push(NoticeLine::new("info", format!("{key}: {}", value.as_ref())));
}
