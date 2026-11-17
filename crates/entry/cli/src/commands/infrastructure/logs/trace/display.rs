//! Trace event rendering helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde_json::Value;
use systemprompt_models::artifacts::NoticeLine;

use super::summary::line;
use systemprompt_runtime::TraceEvent;

use crate::presentation::tables::{format_metadata_value, trace_events_table};

pub(super) fn push_event(
    out: &mut Vec<NoticeLine>,
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
        "LOG" if event.details.starts_with("ERROR") => line(out, "error", event_line),
        "LOG" if event.details.starts_with("WARN") => line(out, "warning", event_line),
        _ => line(out, "info", event_line),
    }

    if verbose {
        push_event_context(out, event);
        push_event_metadata(out, event);
    }
}

fn push_event_context(out: &mut Vec<NoticeLine>, event: &TraceEvent) {
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
        line(
            out,
            "info",
            format!("           {}", context_parts.join(" | ")),
        );
    }
}

fn push_event_metadata(out: &mut Vec<NoticeLine>, event: &TraceEvent) {
    if let Some(ref metadata) = event.metadata
        && let Ok(parsed) = serde_json::from_str::<Value>(metadata)
        && let Some(obj) = parsed.as_object()
    {
        for (key, value) in obj {
            if !value.is_null() {
                let formatted_value = format_metadata_value(key, value);
                line(out, "info", format!("           {key}: {formatted_value}"));
            }
        }
    }
}

pub(super) fn push_table(out: &mut Vec<NoticeLine>, events: &[TraceEvent]) {
    let table = trace_events_table(events);
    if !table.is_empty() {
        line(out, "info", table);
    }
}
