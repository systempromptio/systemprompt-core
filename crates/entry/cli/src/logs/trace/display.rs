use chrono::{DateTime, Utc};
use serde_json::Value;
use systemprompt_core_logging::{
    AiRequestSummary, CliService, ExecutionStepSummary, McpExecutionSummary, TraceEvent,
};
use tabled::settings::Style;
use tabled::{Table, Tabled};

#[derive(Tabled)]
pub struct TraceRow {
    #[tabled(rename = "Time")]
    pub time: String,
    #[tabled(rename = "Delta")]
    pub delta: String,
    #[tabled(rename = "Type")]
    pub event_type: String,
    #[tabled(rename = "Details")]
    pub details: String,
    #[tabled(rename = "Latency")]
    pub latency: String,
}

pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}

pub fn format_metadata_value(key: &str, value: &Value) -> String {
    match key {
        "cost_cents" => value.as_i64().map_or_else(
            || format!("{}", value).trim_matches('"').to_string(),
            |microdollars| {
                let dollars = microdollars as f64 / 1_000_000.0;
                format!("${:.6}", dollars)
            },
        ),
        "latency_ms" | "execution_time_ms" => value
            .as_i64()
            .map_or_else(|| format!("{}", value).trim_matches('"').to_string(), |ms| format!("{}ms", ms)),
        "tokens_used" => value.as_i64().map_or_else(
            || format!("{}", value).trim_matches('"').to_string(),
            |tokens| format!("{}", tokens),
        ),
        _ => format!("{}", value).trim_matches('"').to_string(),
    }
}

pub fn extract_latency_from_metadata(metadata: Option<&str>, event_type: &str) -> String {
    if let Some(meta) = metadata {
        if let Ok(parsed) = serde_json::from_str::<Value>(meta) {
            match event_type {
                "AI" => {
                    if let Some(latency) = parsed.get("latency_ms").and_then(Value::as_i64) {
                        return format!("{}ms", latency);
                    }
                },
                "MCP" => {
                    if let Some(exec_time) =
                        parsed.get("execution_time_ms").and_then(Value::as_i64)
                    {
                        return format!("{}ms", exec_time);
                    }
                },
                _ => {},
            }
        }
    }
    "-".to_string()
}

pub fn print_event(event: &TraceEvent, verbose: bool, prev_timestamp: Option<DateTime<Utc>>) {
    let timestamp = event.timestamp.format("%H:%M:%S%.3f").to_string();

    let delta = prev_timestamp.map_or_else(
        || "(+0ms)".to_string(),
        |prev| {
            let delta_ms = event
                .timestamp
                .signed_duration_since(prev)
                .num_milliseconds();
            format!("(+{delta_ms}ms)")
        },
    );

    let type_label = match event.event_type.as_str() {
        "LOG" => "[LOG]   ",
        "AI" => "[AI]    ",
        "STEP" => "[STEP]  ",
        "TASK" => "[TASK]  ",
        "MESSAGE" => "[MSG]   ",
        "MCP" => "[MCP]   ",
        _ => "[UNKNOWN]",
    };

    let event_line = format!("{timestamp} {delta} {type_label} {}", event.details);

    match event.event_type.as_str() {
        "LOG" if event.details.starts_with("ERROR") => CliService::error(&event_line),
        "LOG" if event.details.starts_with("WARN") => CliService::warning(&event_line),
        _ => CliService::info(&event_line),
    }

    if verbose {
        print_event_context(event);
        print_event_metadata(event);
    }
}

fn print_event_context(event: &TraceEvent) {
    let mut context_parts = Vec::new();

    if let Some(ref session_id) = event.session_id {
        let len = session_id.len().min(12);
        context_parts.push(format!("session: {}", &session_id[..len]));
    }
    if let Some(ref user_id) = event.user_id {
        let len = user_id.len().min(12);
        context_parts.push(format!("user: {}", &user_id[..len]));
    }
    if let Some(ref task_id) = event.task_id {
        let len = task_id.len().min(12);
        context_parts.push(format!("task: {}", &task_id[..len]));
    }
    if let Some(ref context_id) = event.context_id {
        let len = context_id.len().min(12);
        context_parts.push(format!("context: {}", &context_id[..len]));
    }

    if !context_parts.is_empty() {
        CliService::info(&format!("           {}", context_parts.join(" | ")));
    }
}

fn print_event_metadata(event: &TraceEvent) {
    if let Some(ref metadata) = event.metadata {
        if let Ok(parsed) = serde_json::from_str::<Value>(metadata) {
            if let Some(obj) = parsed.as_object() {
                for (key, value) in obj {
                    if !value.is_null() {
                        let formatted_value = format_metadata_value(key, value);
                        CliService::info(&format!("           {key}: {formatted_value}"));
                    }
                }
            }
        }
    }
}

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
    let mut event_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
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

pub fn print_table(events: &[TraceEvent]) {
    let mut prev_timestamp: Option<DateTime<Utc>> = None;
    let rows: Vec<TraceRow> = events
        .iter()
        .map(|e| {
            let time = e.timestamp.format("%H:%M:%S%.3f").to_string();
            let delta = prev_timestamp.map_or_else(
                || "+0ms".to_string(),
                |prev| {
                    let delta_ms = e.timestamp.signed_duration_since(prev).num_milliseconds();
                    format!("+{}ms", delta_ms)
                },
            );
            prev_timestamp = Some(e.timestamp);

            let latency = extract_latency_from_metadata(e.metadata.as_deref(), &e.event_type);

            TraceRow {
                time,
                delta,
                event_type: e.event_type.clone(),
                details: truncate_string(&e.details, 100),
                latency,
            }
        })
        .collect();

    if !rows.is_empty() {
        let table = Table::new(rows).with(Style::modern()).to_string();
        CliService::info(&table);
    }
}
