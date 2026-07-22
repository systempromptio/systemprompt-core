//! Tests for `infra logs search` row mapping: module filtering, timestamp
//! formatting, metadata parsing, and tool-execution projection.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use chrono::{TimeZone, Utc};
use systemprompt_cli::infrastructure::logs::search::{map_log_rows, map_tool_rows};
use systemprompt_identifiers::{LogId, TraceId};
use systemprompt_logging::{LogSearchItem, ToolExecutionItem};

fn log_item(module: &str, metadata: Option<&str>) -> LogSearchItem {
    LogSearchItem {
        id: LogId::generate(),
        trace_id: TraceId::generate(),
        timestamp: Utc.with_ymd_and_hms(2026, 7, 22, 9, 30, 5).unwrap(),
        level: "warn".to_string(),
        module: module.to_string(),
        message: "boom".to_string(),
        metadata: metadata.map(str::to_string),
    }
}

fn tool_item(server: Option<&str>, duration: Option<i32>) -> ToolExecutionItem {
    ToolExecutionItem {
        timestamp: Utc.with_ymd_and_hms(2026, 7, 22, 9, 30, 5).unwrap(),
        trace_id: TraceId::generate(),
        tool_name: "list_files".to_string(),
        server_name: server.map(str::to_string),
        status: "success".to_string(),
        execution_time_ms: duration,
    }
}

#[test]
fn map_log_rows_uppercases_level_and_formats_timestamp_with_millis() {
    let rows = map_log_rows(vec![log_item("mcp::server", None)], None);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].level, "WARN");
    assert_eq!(rows[0].timestamp, "2026-07-22 09:30:05.000");
    assert_eq!(rows[0].message, "boom");
    assert!(rows[0].metadata.is_none());
}

#[test]
fn map_log_rows_filters_by_module_substring() {
    let rows = map_log_rows(
        vec![log_item("mcp::server", None), log_item("agent::task", None)],
        Some("agent"),
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].module, "agent::task");
}

#[test]
fn map_log_rows_parses_valid_metadata_and_drops_invalid() {
    let rows = map_log_rows(
        vec![
            log_item("m", Some(r#"{"key":"value"}"#)),
            log_item("m", Some("not-json")),
        ],
        None,
    );
    assert_eq!(rows[0].metadata.as_ref().unwrap()["key"], "value");
    assert!(rows[1].metadata.is_none());
}

#[test]
fn map_tool_rows_defaults_missing_server_and_widens_duration() {
    let rows = map_tool_rows(vec![tool_item(None, Some(42)), tool_item(Some("fs"), None)]);
    assert_eq!(rows[0].server, "unknown");
    assert_eq!(rows[0].duration_ms, Some(42));
    assert_eq!(rows[0].timestamp, "2026-07-22 09:30:05");
    assert_eq!(rows[1].server, "fs");
    assert!(rows[1].duration_ms.is_none());
    assert_eq!(rows[1].status, "success");
}
