use systemprompt_logging::CliService;
use systemprompt_models::text::truncate_with_ellipsis;

use super::LogEntryRow;

pub use systemprompt_models::time_format::{format_optional_duration_ms, format_timestamp};

pub fn cost_microdollars_to_dollars(microdollars: i64) -> f64 {
    microdollars as f64 / 1_000_000.0
}

pub fn display_log_row(log: &LogEntryRow) {
    let time_part = if log.timestamp.len() >= 23 {
        &log.timestamp[11..23]
    } else {
        &log.timestamp
    };

    let trace_short = truncate_with_ellipsis(&log.trace_id, 8);

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
