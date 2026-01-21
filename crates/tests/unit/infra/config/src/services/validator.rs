//! Unit tests for ConfigValidator and ValidationReport
//!
//! Tests cover:
//! - ValidationReport creation and error/warning tracking
//! - ValidationReport is_valid state management

use systemprompt_config::ValidationReport;

// ============================================================================
// ValidationReport Creation Tests
// ============================================================================

#[test]
fn test_validation_report_new() {
    let report = ValidationReport::new();
    assert!(report.errors.is_empty());
    assert!(report.warnings.is_empty());
}

#[test]
fn test_validation_report_default() {
    let report = ValidationReport::default();
    assert!(report.errors.is_empty());
    assert!(report.warnings.is_empty());
}

#[test]
fn test_validation_report_new_is_valid() {
    let report = ValidationReport::new();
    assert!(report.is_valid());
}

// ============================================================================
// ValidationReport Error Tests
// ============================================================================

#[test]
fn test_validation_report_add_error() {
    let mut report = ValidationReport::new();
    report.add_error("Test error".to_string());
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0], "Test error");
}

#[test]
fn test_validation_report_add_multiple_errors() {
    let mut report = ValidationReport::new();
    report.add_error("Error 1".to_string());
    report.add_error("Error 2".to_string());
    report.add_error("Error 3".to_string());
    assert_eq!(report.errors.len(), 3);
}

#[test]
fn test_validation_report_with_error_not_valid() {
    let mut report = ValidationReport::new();
    report.add_error("Error".to_string());
    assert!(!report.is_valid());
}

#[test]
fn test_validation_report_error_content() {
    let mut report = ValidationReport::new();
    report.add_error("Missing required variable: DATABASE_URL".to_string());
    assert!(report.errors[0].contains("DATABASE_URL"));
}

// ============================================================================
// ValidationReport Warning Tests
// ============================================================================

#[test]
fn test_validation_report_add_warning() {
    let mut report = ValidationReport::new();
    report.add_warning("Test warning".to_string());
    assert_eq!(report.warnings.len(), 1);
    assert_eq!(report.warnings[0], "Test warning");
}

#[test]
fn test_validation_report_add_multiple_warnings() {
    let mut report = ValidationReport::new();
    report.add_warning("Warning 1".to_string());
    report.add_warning("Warning 2".to_string());
    assert_eq!(report.warnings.len(), 2);
}

#[test]
fn test_validation_report_warnings_dont_affect_validity() {
    let mut report = ValidationReport::new();
    report.add_warning("Warning".to_string());
    assert!(report.is_valid());
}

#[test]
fn test_validation_report_warning_content() {
    let mut report = ValidationReport::new();
    report.add_warning("PORT not explicitly set".to_string());
    assert!(report.warnings[0].contains("PORT"));
}

// ============================================================================
// ValidationReport Mixed State Tests
// ============================================================================

#[test]
fn test_validation_report_errors_and_warnings() {
    let mut report = ValidationReport::new();
    report.add_error("Error".to_string());
    report.add_warning("Warning".to_string());
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.warnings.len(), 1);
    assert!(!report.is_valid());
}

#[test]
fn test_validation_report_multiple_errors_multiple_warnings() {
    let mut report = ValidationReport::new();
    report.add_error("Error 1".to_string());
    report.add_error("Error 2".to_string());
    report.add_warning("Warning 1".to_string());
    report.add_warning("Warning 2".to_string());
    report.add_warning("Warning 3".to_string());
    assert_eq!(report.errors.len(), 2);
    assert_eq!(report.warnings.len(), 3);
    assert!(!report.is_valid());
}

// ============================================================================
// ValidationReport Debug Tests
// ============================================================================

#[test]
fn test_validation_report_debug() {
    let report = ValidationReport::new();
    let debug_str = format!("{:?}", report);
    assert!(debug_str.contains("ValidationReport"));
}

#[test]
fn test_validation_report_debug_with_content() {
    let mut report = ValidationReport::new();
    report.add_error("Error message".to_string());
    report.add_warning("Warning message".to_string());
    let debug_str = format!("{:?}", report);
    assert!(debug_str.contains("Error message"));
    assert!(debug_str.contains("Warning message"));
}

// ============================================================================
// ValidationReport Edge Cases
// ============================================================================

#[test]
fn test_validation_report_empty_error() {
    let mut report = ValidationReport::new();
    report.add_error(String::new());
    assert_eq!(report.errors.len(), 1);
    assert!(!report.is_valid());
}

#[test]
fn test_validation_report_empty_warning() {
    let mut report = ValidationReport::new();
    report.add_warning(String::new());
    assert_eq!(report.warnings.len(), 1);
    assert!(report.is_valid());
}

#[test]
fn test_validation_report_duplicate_errors() {
    let mut report = ValidationReport::new();
    report.add_error("Same error".to_string());
    report.add_error("Same error".to_string());
    assert_eq!(report.errors.len(), 2);
}

#[test]
fn test_validation_report_long_error_message() {
    let mut report = ValidationReport::new();
    let long_message = "A".repeat(1000);
    report.add_error(long_message.clone());
    assert_eq!(report.errors[0], long_message);
}

// ============================================================================
// ValidationReport is_valid Comprehensive Tests
// ============================================================================

#[test]
fn test_is_valid_empty_report() {
    let report = ValidationReport::new();
    assert!(report.is_valid());
}

#[test]
fn test_is_valid_only_warnings() {
    let mut report = ValidationReport::new();
    report.add_warning("w1".to_string());
    report.add_warning("w2".to_string());
    assert!(report.is_valid());
}

#[test]
fn test_is_valid_only_errors() {
    let mut report = ValidationReport::new();
    report.add_error("e1".to_string());
    assert!(!report.is_valid());
}

#[test]
fn test_is_valid_errors_and_warnings() {
    let mut report = ValidationReport::new();
    report.add_error("error".to_string());
    report.add_warning("warning".to_string());
    assert!(!report.is_valid());
}

// ============================================================================
// ValidationReport Typical Usage Tests
// ============================================================================

#[test]
fn test_validation_report_typical_config_errors() {
    let mut report = ValidationReport::new();

    // Simulate typical validation errors
    report.add_error("Required variable missing: DATABASE_URL".to_string());
    report.add_error("Required variable missing: JWT_SECRET".to_string());
    report.add_warning("PORT not explicitly set".to_string());

    assert!(!report.is_valid());
    assert_eq!(report.errors.len(), 2);
    assert_eq!(report.warnings.len(), 1);
}

#[test]
fn test_validation_report_url_format_error() {
    let mut report = ValidationReport::new();
    report.add_error("Invalid URL format: API_SERVER_URL = not-a-url".to_string());

    assert!(!report.is_valid());
    assert!(report.errors[0].contains("URL format"));
}

#[test]
fn test_validation_report_port_validation_error() {
    let mut report = ValidationReport::new();
    report.add_error("Port is not a valid number: abc".to_string());

    assert!(!report.is_valid());
    assert!(report.errors[0].contains("Port"));
}

#[test]
fn test_validation_report_unresolved_variable_error() {
    let mut report = ValidationReport::new();
    report.add_error("Unresolved variable: HOST = ${UNDEFINED_VAR}".to_string());

    assert!(!report.is_valid());
    assert!(report.errors[0].contains("Unresolved"));
}

#[test]
fn test_validation_report_production_warning() {
    let mut report = ValidationReport::new();
    report.add_warning("Production should have USE_HTTPS=true".to_string());

    assert!(report.is_valid());
    assert!(report.warnings[0].contains("HTTPS"));
}
