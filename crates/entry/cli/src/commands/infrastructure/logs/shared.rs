//! Formatting helpers shared across the `infra logs` subcommands.
//!
//! Re-exports the timestamp/duration formatters from `systemprompt_models` and
//! provides [`level_notice`], `filter_notes` and
//! [`cost_microdollars_to_dollars`] used by the view, search, stream and trace
//! outputs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_logging::LogLevel;
use systemprompt_models::artifacts::NoticeLine;

use super::LogFilters;

pub use systemprompt_extension::cli::time_format::{format_optional_duration_ms, format_timestamp};

pub fn cost_microdollars_to_dollars(microdollars: i64) -> f64 {
    microdollars as f64 / 1_000_000.0
}

pub fn level_notice(level: LogLevel, text: impl Into<String>) -> NoticeLine {
    let notice = match level {
        LogLevel::Error => "error",
        LogLevel::Warn => "warning",
        LogLevel::Info | LogLevel::Debug | LogLevel::Trace => "info",
    };
    NoticeLine::new(notice, text)
}

pub(crate) fn filter_notes(filters: &LogFilters) -> Vec<NoticeLine> {
    let mut notes = Vec::new();
    if let Some(level) = filters.level {
        notes.push(NoticeLine::new(
            "info",
            format!("Level: {}", level.as_str()),
        ));
    }
    if let Some(ref module) = filters.module {
        notes.push(NoticeLine::new("info", format!("Module: {module}")));
    }
    if let Some(ref since) = filters.since {
        notes.push(NoticeLine::new("info", format!("Since: {since}")));
    }
    notes
}
