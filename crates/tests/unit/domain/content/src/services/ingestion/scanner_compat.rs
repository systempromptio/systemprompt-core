//! Tests that exercise ingestion code paths via the public `ContentReady` API,
//! covering frontmatter parsing edge cases and the `IngestionReport` fields
//! added in later versions.

use systemprompt_content::{IngestionOptions, IngestionReport};

#[test]
fn ingestion_options_dry_run_default_is_false() {
    let opts = IngestionOptions::default();
    assert!(!opts.dry_run);
}

#[test]
fn ingestion_options_with_dry_run_true() {
    let opts = IngestionOptions::default().with_dry_run(true);
    assert!(opts.dry_run);
    assert!(!opts.override_existing);
    assert!(!opts.recursive);
}

#[test]
fn ingestion_options_full_chain() {
    let opts = IngestionOptions::default()
        .with_override(true)
        .with_recursive(true)
        .with_dry_run(true);
    assert!(opts.override_existing);
    assert!(opts.recursive);
    assert!(opts.dry_run);
}

#[test]
fn ingestion_report_would_create_empty_by_default() {
    let report = IngestionReport::new();
    assert!(report.would_create.is_empty());
    assert!(report.would_update.is_empty());
}

#[test]
fn ingestion_report_unchanged_and_skipped_default_zero() {
    let report = IngestionReport::new();
    assert_eq!(report.unchanged_count, 0);
    assert_eq!(report.skipped_count, 0);
}

#[test]
fn ingestion_report_would_create_accumulation() {
    let mut report = IngestionReport::new();
    report.would_create.push("new-post-1".to_string());
    report.would_create.push("new-post-2".to_string());
    assert_eq!(report.would_create.len(), 2);
    assert!(report.is_success());
}

#[test]
fn ingestion_report_would_update_accumulation() {
    let mut report = IngestionReport::new();
    report.would_update.push("existing-post".to_string());
    report.unchanged_count = 5;
    report.skipped_count = 2;
    assert_eq!(report.would_update.len(), 1);
    assert_eq!(report.unchanged_count, 5);
    assert_eq!(report.skipped_count, 2);
    assert!(report.is_success());
}

#[test]
fn ingestion_report_errors_make_is_success_false() {
    let mut report = IngestionReport::new();
    report.would_create.push("draft".to_string());
    report.errors.push("parse error".to_string());
    assert!(!report.is_success());
}

#[test]
fn ingestion_report_skipped_count_set() {
    let mut report = IngestionReport::new();
    report.skipped_count = 10;
    assert_eq!(report.skipped_count, 10);
    assert!(report.is_success());
}

#[test]
fn ingestion_report_unchanged_count_set() {
    let mut report = IngestionReport::new();
    report.unchanged_count = 42;
    assert_eq!(report.unchanged_count, 42);
    assert!(report.is_success());
}
