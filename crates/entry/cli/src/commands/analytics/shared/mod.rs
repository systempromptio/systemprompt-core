//! Cross-domain helpers shared by every analytics command.
//!
//! Re-exports CSV export utilities ([`export`]), reusable output/formatting
//! types and number formatters ([`output`]), and time-range parsing and
//! bucketing ([`time`]) so the per-domain command modules draw from one
//! surface.

pub mod export;
pub mod output;
pub mod time;

pub use export::{
    CsvBuilder, ensure_export_dir, export_single_to_csv, export_to_csv, resolve_export_path,
};
pub use output::{
    BreakdownData, BreakdownItem, MetricCard, StatsSummary, TrendData, TrendPoint, format_change,
    format_cost, format_number, format_percent, format_tokens,
};
pub use systemprompt_models::time_format::format_date_range;
pub use time::{
    format_duration_ms, format_period_label, format_timestamp, parse_duration, parse_since,
    parse_time_range, parse_until, truncate_to_period,
};
