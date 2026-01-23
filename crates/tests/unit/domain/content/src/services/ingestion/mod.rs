//! Unit tests for ingestion services
//!
//! Tests cover:
//! - IngestionService functionality
//! - IngestionOptions builder pattern
//! - IngestionReport structure

use systemprompt_content::{IngestionOptions, IngestionReport};

// ============================================================================
// IngestionOptions Tests
// ============================================================================

#[test]
fn test_ingestion_options_default() {
    let options = IngestionOptions::default();
    assert!(!options.override_existing);
    assert!(!options.recursive);
}

#[test]
fn test_ingestion_options_builder_override() {
    let options = IngestionOptions::default().with_override(true);
    assert!(options.override_existing);
    assert!(!options.recursive);
}

#[test]
fn test_ingestion_options_builder_recursive() {
    let options = IngestionOptions::default().with_recursive(true);
    assert!(!options.override_existing);
    assert!(options.recursive);
}

#[test]
fn test_ingestion_options_builder_chain() {
    let options = IngestionOptions::default()
        .with_override(true)
        .with_recursive(true);
    assert!(options.override_existing);
    assert!(options.recursive);
}

#[test]
fn test_ingestion_options_builder_toggle() {
    let options = IngestionOptions::default()
        .with_override(true)
        .with_override(false);
    assert!(!options.override_existing);
}

// ============================================================================
// IngestionReport Tests
// ============================================================================

#[test]
fn test_ingestion_report_new() {
    let report = IngestionReport::new();
    assert_eq!(report.files_found, 0);
    assert_eq!(report.files_processed, 0);
    assert!(report.errors.is_empty());
    assert!(report.warnings.is_empty());
}

#[test]
fn test_ingestion_report_default() {
    let report = IngestionReport::default();
    assert_eq!(report.files_found, 0);
    assert_eq!(report.files_processed, 0);
}

#[test]
fn test_ingestion_report_is_success_empty() {
    let report = IngestionReport::new();
    assert!(report.is_success());
}

#[test]
fn test_ingestion_report_is_success_with_errors() {
    let mut report = IngestionReport::new();
    report.errors.push("Error 1".to_string());
    assert!(!report.is_success());
}

#[test]
fn test_ingestion_report_is_success_with_warnings_only() {
    let mut report = IngestionReport::new();
    report.warnings.push("Warning 1".to_string());
    assert!(report.is_success());
}

#[test]
fn test_ingestion_report_accumulation() {
    let mut report = IngestionReport::new();
    report.files_found = 10;
    report.files_processed = 8;
    report.errors.push("File not found".to_string());
    report.errors.push("Parse error".to_string());
    report.warnings.push("Skipped duplicate".to_string());

    assert_eq!(report.files_found, 10);
    assert_eq!(report.files_processed, 8);
    assert_eq!(report.errors.len(), 2);
    assert_eq!(report.warnings.len(), 1);
    assert!(!report.is_success());
}

#[test]
fn test_ingestion_report_debug() {
    let report = IngestionReport::new();
    let debug = format!("{:?}", report);
    assert!(debug.contains("IngestionReport"));
    assert!(debug.contains("files_found"));
}
