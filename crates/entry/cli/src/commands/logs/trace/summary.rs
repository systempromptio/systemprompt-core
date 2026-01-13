use chrono::{DateTime, Utc};
use std::collections::HashMap;
use systemprompt_core_logging::{
    AiRequestSummary, CliService, ExecutionStepSummary, McpExecutionSummary, TraceEvent,
};

pub struct SummaryContext<'a> {
    pub events: &'a [TraceEvent],
    pub first: Option<DateTime<Utc>>,
    pub last: Option<DateTime<Utc>>,
    pub task_id: Option<&'a str>,
    pub ai_summary: &'a AiRequestSummary,
    pub mcp_summary: &'a McpExecutionSummary,
    pub step_summary: &'a ExecutionStepSummary,
}

pub fn print_summary(ctx: &SummaryContext<'_>) {
    CliService::section("Summary");

    if let (Some(first), Some(last)) = (ctx.first, ctx.last) {
        let duration = last.signed_duration_since(first);
        CliService::key_value("  Duration", &format!("{}ms", duration.num_milliseconds()));
    }

    print_event_counts(ctx.events);
    print_ai_summary(ctx.ai_summary);
    print_mcp_summary(ctx.mcp_summary);
    print_step_summary(ctx.step_summary);
    print_trace_context(ctx.events, ctx.task_id);
    print_status(ctx.events);
}

fn print_event_counts(events: &[TraceEvent]) {
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
    CliService::key_value(
        "  Events",
        &format!("{} ({})", events.len(), event_parts.join(", ")),
    );
}

fn print_ai_summary(ai_summary: &AiRequestSummary) {
    if ai_summary.request_count > 0 {
        CliService::info("  AI Requests:");
        CliService::key_value("     Requests", &ai_summary.request_count.to_string());
        CliService::key_value(
            "     Tokens",
            &format!(
                "{} (in: {}, out: {})",
                ai_summary.total_tokens,
                ai_summary.total_input_tokens,
                ai_summary.total_output_tokens
            ),
        );
        let dollars = ai_summary.total_cost_cents as f64 / 1_000_000.0;
        CliService::key_value("     Cost", &format!("${dollars:.6}"));
        CliService::key_value(
            "     Total Latency",
            &format!("{}ms", ai_summary.total_latency_ms),
        );
        if ai_summary.request_count > 0 {
            let avg_latency = ai_summary.total_latency_ms / ai_summary.request_count;
            CliService::key_value("     Avg Latency", &format!("{avg_latency}ms"));
        }
    }
}

fn print_mcp_summary(mcp_summary: &McpExecutionSummary) {
    if mcp_summary.execution_count > 0 {
        CliService::info("  MCP Tool Executions:");
        CliService::key_value("     Executions", &mcp_summary.execution_count.to_string());
        CliService::key_value(
            "     Total Time",
            &format!("{}ms", mcp_summary.total_execution_time_ms),
        );
        if mcp_summary.execution_count > 0 {
            let avg_time = mcp_summary.total_execution_time_ms / mcp_summary.execution_count;
            CliService::key_value("     Avg Time", &format!("{avg_time}ms"));
        }
    }
}

fn print_step_summary(step_summary: &ExecutionStepSummary) {
    if step_summary.total > 0 {
        CliService::info("  Execution Steps:");
        CliService::key_value(
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

fn print_trace_context(events: &[TraceEvent], task_id: Option<&str>) {
    if let Some(task_id) = task_id {
        CliService::key_value(
            "  Task",
            &format!("{task_id} (use: just ai-trace <task_id>)"),
        );
    }

    if let Some(session_id) = events.first().and_then(|e| e.session_id.as_ref()) {
        CliService::key_value("  Session", session_id);
    }

    if let Some(user_id) = events.first().and_then(|e| e.user_id.as_ref()) {
        CliService::key_value("  User", user_id);
    }
}

fn print_status(events: &[TraceEvent]) {
    let has_errors = events.iter().any(|e| {
        e.details.contains("ERROR")
            || e.details.contains("failed")
            || e.details.contains("(failed)")
    });

    if has_errors {
        CliService::error("  Status: [FAILED]");
    } else {
        CliService::success("  Status: [OK]");
    }
}
