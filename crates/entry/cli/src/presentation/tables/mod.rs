//! Reusable `tabled` table widgets for command output.
//!
//! Pure shaping and rendering: each function turns domain records into a
//! rendered table string. Callers decide where the string is printed, so the
//! row shaping stays testable without a terminal.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod entities;
mod trace;

pub use self::entities::{artifact_list_table, context_list_table, db_tables_table};
pub use self::trace::{
    ai_requests_table, execution_steps_table, extract_latency_from_metadata, format_metadata_value,
    mcp_tool_calls_table, task_artifacts_table, task_info_table, trace_events_table,
};

use crate::shared::truncate_with_ellipsis;

#[must_use]
pub fn truncate_cell(s: &str, max_len: usize) -> String {
    let flattened = s.replace('\n', " ").replace('\r', "");
    truncate_with_ellipsis(&flattened, max_len)
}

pub(super) fn dash() -> String {
    "-".to_owned()
}

pub(super) fn millis(value: Option<impl std::fmt::Display>) -> String {
    value.map_or_else(dash, |ms| format!("{ms}ms"))
}
