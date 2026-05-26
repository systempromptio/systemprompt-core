//! Unit tests for cli::table rendering helpers.

use std::time::Duration;

use systemprompt_logging::services::cli::table::{
    ServiceTableEntry, render_service_table, render_startup_complete, render_table,
    truncate_to_width,
};
use systemprompt_logging::services::cli::theme::ServiceStatus;

#[test]
fn truncate_to_width_returns_input_when_short_enough() {
    assert_eq!(truncate_to_width("hello", 10), "hello");
    assert_eq!(truncate_to_width("hello", 5), "hello");
}

#[test]
fn truncate_to_width_truncates_and_appends_ellipsis() {
    let out = truncate_to_width("abcdefghij", 6);
    assert_eq!(out, "abc...");
    assert!(out.chars().count() <= 6);
}

#[test]
fn truncate_to_width_zero_width_yields_ellipsis_only() {
    let out = truncate_to_width("abcdef", 3);
    assert_eq!(out, "...");
}

#[test]
fn truncate_to_width_handles_unicode() {
    let s = "日本語テストひらがな";
    let out = truncate_to_width(s, 5);
    assert!(out.ends_with("..."));
}

#[test]
fn render_table_empty_rows_no_panic() {
    render_table(&["a", "b"], &[]);
}

#[test]
fn render_table_single_row() {
    render_table(
        &["Name", "Type"],
        &[vec!["alpha".to_owned(), "service".to_owned()]],
    );
}

#[test]
fn render_table_multiple_rows_unequal_widths() {
    let rows = vec![
        vec!["x".to_owned(), "y-long-cell".to_owned()],
        vec!["very-long-name".to_owned(), "y".to_owned()],
    ];
    render_table(&["Col1", "Col2"], &rows);
}

#[test]
fn render_table_handles_extra_cells() {
    let rows = vec![vec![
        "a".to_owned(),
        "b".to_owned(),
        "extra".to_owned(),
    ]];
    render_table(&["X", "Y"], &rows);
}

#[test]
fn service_table_entry_builder() {
    let e = ServiceTableEntry::new("svc", "type", Some(1234), ServiceStatus::Running);
    assert_eq!(e.name, "svc");
    assert_eq!(e.service_type, "type");
    assert_eq!(e.port, Some(1234));
    assert!(matches!(e.status, ServiceStatus::Running));
}

#[test]
fn render_service_table_empty_no_panic() {
    render_service_table("Title", &[]);
}

#[test]
fn render_service_table_all_status_variants() {
    let services = vec![
        ServiceTableEntry::new("r", "t", Some(80), ServiceStatus::Running),
        ServiceTableEntry::new("s", "t", Some(81), ServiceStatus::Starting),
        ServiceTableEntry::new("o", "t", None, ServiceStatus::Stopped),
        ServiceTableEntry::new("f", "t", Some(83), ServiceStatus::Failed),
        ServiceTableEntry::new("u", "t", Some(84), ServiceStatus::Unknown),
    ];
    render_service_table("All Services", &services);
}

#[test]
fn render_startup_complete_smoke() {
    render_startup_complete(Duration::from_millis(1234), "http://localhost:8080");
    render_startup_complete(Duration::from_secs(0), "");
}
