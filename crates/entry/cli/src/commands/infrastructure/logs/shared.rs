//! Formatting helpers shared across the `infra logs` subcommands.
//!
//! Re-exports the timestamp/duration formatters from `systemprompt_models` and
//! provides [`display_log_row`], [`print_level_line`] and
//! [`cost_microdollars_to_dollars`] used by the view, search, and trace
//! renderers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_logging::{CliService, LogLevel};
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

    let trace_short = truncate_with_ellipsis(log.trace_id.as_str(), 8);

    let line = format!(
        "{} {} [{}] {}  [{}]",
        time_part, log.level, log.module, log.message, trace_short
    );

    print_level_line(log.level, &line);
}

pub fn print_level_line(level: LogLevel, line: &str) {
    match level {
        LogLevel::Error => CliService::error(line),
        LogLevel::Warn => CliService::warning(line),
        LogLevel::Info | LogLevel::Debug | LogLevel::Trace => CliService::info(line),
    }
}
