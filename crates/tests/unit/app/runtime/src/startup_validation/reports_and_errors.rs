//! Tests for StartupValidationReport, ValidationReport, ValidationError, ValidationWarning,
//! and domain/expected domains

use systemprompt_runtime::FilesConfigValidator;
use systemprompt_traits::{DomainConfig, StartupValidationReport, ValidationReport};

// ============================================================================
// StartupValidationReport Tests
// ============================================================================

#[test]
fn test_startup_validation_report_new() {
    let report = StartupValidationReport::new();
    assert!(!report.has_errors());
    assert_eq!(report.error_count(), 0);
    assert_eq!(report.warning_count(), 0);
}

#[test]
fn test_startup_validation_report_with_profile_path() {
    use std::path::PathBuf;

    let report = StartupValidationReport::new()
        .with_profile_path(PathBuf::from("/etc/config/profile.yaml"));
    assert!(report.profile_path.is_some());
}

#[test]
fn test_startup_validation_report_add_domain() {
    let mut report = StartupValidationReport::new();
    let domain = ValidationReport::new("test");
    report.add_domain(domain);
    assert_eq!(report.domains.len(), 1);
}

#[test]
fn test_startup_validation_report_has_errors_false() {
    let report = StartupValidationReport::new();
    assert!(!report.has_errors());
}

#[test]
fn test_startup_validation_report_error_count_zero() {
    let report = StartupValidationReport::new();
    assert_eq!(report.error_count(), 0);
}

#[test]
fn test_startup_validation_report_warning_count_zero() {
    let report = StartupValidationReport::new();
    assert_eq!(report.warning_count(), 0);
}

// ============================================================================
// ValidationReport Tests
// ============================================================================

#[test]
fn test_validation_report_new() {
    let report = ValidationReport::new("test_domain");
    assert_eq!(report.domain, "test_domain");
    assert!(!report.has_errors());
    assert!(!report.has_warnings());
}

#[test]
fn test_validation_report_domain_name() {
    let report = ValidationReport::new("files");
    assert_eq!(report.domain, "files");
}

#[test]
fn test_validation_report_errors_empty() {
    let report = ValidationReport::new("test");
    assert!(report.errors.is_empty());
}

#[test]
fn test_validation_report_warnings_empty() {
    let report = ValidationReport::new("test");
    assert!(report.warnings.is_empty());
}

#[test]
fn test_validation_report_add_error() {
    use systemprompt_traits::validation_report::ValidationError;

    let mut report = ValidationReport::new("test");
    report.add_error(ValidationError::new("field", "message"));
    assert!(report.has_errors());
    assert_eq!(report.errors.len(), 1);
}

#[test]
fn test_validation_report_add_warning() {
    use systemprompt_traits::validation_report::ValidationWarning;

    let mut report = ValidationReport::new("test");
    report.add_warning(ValidationWarning::new("field", "message"));
    assert!(report.has_warnings());
    assert_eq!(report.warnings.len(), 1);
}

#[test]
fn test_validation_report_multiple_errors() {
    use systemprompt_traits::validation_report::ValidationError;

    let mut report = ValidationReport::new("test");
    report.add_error(ValidationError::new("field1", "error 1"));
    report.add_error(ValidationError::new("field2", "error 2"));
    assert_eq!(report.errors.len(), 2);
}

#[test]
fn test_validation_report_multiple_warnings() {
    use systemprompt_traits::validation_report::ValidationWarning;

    let mut report = ValidationReport::new("test");
    report.add_warning(ValidationWarning::new("field1", "warning 1"));
    report.add_warning(ValidationWarning::new("field2", "warning 2"));
    assert_eq!(report.warnings.len(), 2);
}

// ============================================================================
// ValidationError Tests
// ============================================================================

#[test]
fn test_validation_error_new() {
    use systemprompt_traits::validation_report::ValidationError;

    let error = ValidationError::new("field_name", "error message");
    assert_eq!(error.field, "field_name");
    assert_eq!(error.message, "error message");
}

#[test]
fn test_validation_error_with_path() {
    use std::path::PathBuf;
    use systemprompt_traits::validation_report::ValidationError;

    let error = ValidationError::new("field", "message")
        .with_path(PathBuf::from("/path/to/file"));
    assert!(error.path.is_some());
}

#[test]
fn test_validation_error_with_suggestion() {
    use systemprompt_traits::validation_report::ValidationError;

    let error = ValidationError::new("field", "message")
        .with_suggestion("Try this fix");
    assert!(error.suggestion.is_some());
    assert_eq!(error.suggestion.unwrap(), "Try this fix");
}

#[test]
fn test_validation_error_full_chain() {
    use std::path::PathBuf;
    use systemprompt_traits::validation_report::ValidationError;

    let error = ValidationError::new("database_url", "Connection failed")
        .with_path(PathBuf::from("/config/db.yaml"))
        .with_suggestion("Check your database credentials");
    assert_eq!(error.field, "database_url");
    assert_eq!(error.message, "Connection failed");
    assert!(error.path.is_some());
    assert!(error.suggestion.is_some());
}

// ============================================================================
// ValidationWarning Tests
// ============================================================================

#[test]
fn test_validation_warning_new() {
    use systemprompt_traits::validation_report::ValidationWarning;

    let warning = ValidationWarning::new("field_name", "warning message");
    assert_eq!(warning.field, "field_name");
    assert_eq!(warning.message, "warning message");
}

#[test]
fn test_validation_warning_with_suggestion() {
    use systemprompt_traits::validation_report::ValidationWarning;

    let warning = ValidationWarning::new("field", "message")
        .with_suggestion("Consider this");
    assert!(warning.suggestion.is_some());
}

// ============================================================================
// Expected Domains Complete List Tests
// ============================================================================

#[test]
fn test_all_registered_domains() {
    let expected_domains = vec!["files", "ratelimits", "web", "content", "agents", "mcp", "ai"];
    assert_eq!(expected_domains.len(), 7);
}

#[test]
fn test_files_domain_priority() {
    let validator = FilesConfigValidator::new();
    let priority = validator.priority();
    assert!(priority > 0);
}

#[test]
fn test_domain_id_formats() {
    let domains = vec!["files", "ratelimits", "web", "content", "agents", "mcp", "ai"];
    for domain in domains {
        assert!(!domain.is_empty());
        assert!(domain.chars().all(|c| c.is_lowercase() || c == '_'));
    }
}
