//! Tests for db-command byte formatting and table-name suggestions.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::infrastructure::db::helpers::{
    extract_relation_name, format_bytes, suggest_table_name,
};

#[test]
fn format_bytes_picks_unit_by_magnitude() {
    assert_eq!(format_bytes(0), "0 bytes");
    assert_eq!(format_bytes(1023), "1023 bytes");
    assert_eq!(format_bytes(1024), "1.00 KB");
    assert_eq!(format_bytes(1536), "1.50 KB");
    assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
    assert_eq!(format_bytes(5 * 1024 * 1024 * 1024 / 2), "2.50 GB");
}

#[test]
fn extract_relation_name_reads_quoted_identifier() {
    assert_eq!(
        extract_relation_name("relation \"userz\" does not exist"),
        "userz"
    );
    assert_eq!(extract_relation_name("no quotes here"), "unknown");
    assert_eq!(extract_relation_name("dangling \"quote"), "unknown");
}

#[test]
fn suggest_table_name_matches_substrings_and_typos() {
    assert_eq!(suggest_table_name("log").as_deref(), Some("logs"));
    assert_eq!(suggest_table_name("userz").as_deref(), Some("users"));
    assert_eq!(
        suggest_table_name("mcp_tool_execution").as_deref(),
        Some("mcp_tool_executions")
    );
    assert_eq!(
        suggest_table_name("agent_execution").as_deref(),
        Some("agent_execution_steps")
    );
}

#[test]
fn suggest_table_name_returns_none_for_unrelated_input() {
    assert_eq!(suggest_table_name("zzzzzzzzzzzzzzzzzzz"), None);
}
