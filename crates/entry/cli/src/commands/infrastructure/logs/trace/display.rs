//! Trace event rendering helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde_json::Value;
use systemprompt_logging::{CliService, TraceEvent};

use crate::presentation::tables::{format_metadata_value, trace_events_table};

pub(super) fn print_event(
    event: &TraceEvent,
    verbose: bool,
    prev_timestamp: Option<DateTime<Utc>>,
) {
    let timestamp = event.timestamp.format("%H:%M:%S%.3f").to_string();

    let delta = prev_timestamp.map_or_else(
        || "(+0ms)".to_owned(),
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
        let s = session_id.as_str();
        let len = s.len().min(12);
        context_parts.push(format!("session: {}", &s[..len]));
    }
    if let Some(ref user_id) = event.user_id {
        let s = user_id.as_str();
        let len = s.len().min(12);
        context_parts.push(format!("user: {}", &s[..len]));
    }
    if let Some(ref task_id) = event.task_id {
        let s = task_id.as_str();
        let len = s.len().min(12);
        context_parts.push(format!("task: {}", &s[..len]));
    }
    if let Some(ref context_id) = event.context_id {
        let s = context_id.as_str();
        let len = s.len().min(12);
        context_parts.push(format!("context: {}", &s[..len]));
    }

    if !context_parts.is_empty() {
        CliService::info(&format!("           {}", context_parts.join(" | ")));
    }
}

fn print_event_metadata(event: &TraceEvent) {
    if let Some(ref metadata) = event.metadata
        && let Ok(parsed) = serde_json::from_str::<Value>(metadata)
        && let Some(obj) = parsed.as_object()
    {
        for (key, value) in obj {
            if !value.is_null() {
                let formatted_value = format_metadata_value(key, value);
                CliService::info(&format!("           {key}: {formatted_value}"));
            }
        }
    }
}

pub(super) fn print_table(events: &[TraceEvent]) {
    let table = trace_events_table(events);
    if !table.is_empty() {
        CliService::info(&table);
    }
}
