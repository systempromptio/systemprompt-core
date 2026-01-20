use chrono::{DateTime, Utc};
use systemprompt_core_logging::CliService;

use super::LogEntryRow;

pub fn truncate_id(id: &str, max_len: usize) -> String {
    if id.len() > max_len {
        format!("{}...", &id[..max_len])
    } else {
        id.to_string()
    }
}

pub fn format_timestamp(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn cost_cents_to_dollars(cents: i32) -> f64 {
    f64::from(cents) / 1_000_000.0
}

pub fn format_duration_ms(ms: Option<i64>) -> String {
    ms.map_or_else(String::new, |d| format!(" ({}ms)", d))
}

pub fn display_log_row(log: &LogEntryRow) {
    let time_part = if log.timestamp.len() >= 23 {
        &log.timestamp[11..23]
    } else {
        &log.timestamp
    };

    let trace_short = truncate_id(&log.trace_id, 8);

    let line = format!(
        "{} {} [{}] {}  [{}]",
        time_part, log.level, log.module, log.message, trace_short
    );

    match log.level.as_str() {
        "ERROR" => CliService::error(&line),
        "WARN" => CliService::warning(&line),
        _ => CliService::info(&line),
    }
}
