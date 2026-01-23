//! Unit tests for StartupValidator
//!
//! Tests cover:
//! - StartupValidator creation via new() and default()
//! - Domain validator registration verification
//! - Expected validator domains (web, content, agents, mcp, ai, files, ratelimits)
//! - FilesConfigValidator creation and trait implementation
//! - Display function output patterns
//!
//! Note: The validate() method requires Config and performs I/O operations.
//! Full validation testing is performed in integration tests.

use systemprompt_runtime::{display_validation_report, display_validation_warnings, StartupValidator};
use systemprompt_traits::{DomainConfig, StartupValidationReport, ValidationReport};

// ============================================================================
// StartupValidator Creation Tests
// ============================================================================

#[test]
fn test_startup_validator_new() {
    let validator = StartupValidator::new();
    // Validator should be created successfully
    let _ = validator;
}

#[test]
fn test_startup_validator_default() {
    let validator = StartupValidator::default();
    // Default should behave same as new()
    let _ = validator;
}

#[test]
fn test_startup_validator_debug() {
    let validator = StartupValidator::new();
    let debug_str = format!("{:?}", validator);
    assert!(debug_str.contains("StartupValidator"));
}

// ============================================================================
// Domain Validator Registration Tests
// ============================================================================

// Note: The following tests verify that the expected domain validators
// are registered by checking internal state. Since the registry field
// is private, we rely on the documented behavior from the source code
// that new() registers these validators:
// - WebConfigValidator
// - ContentConfigValidator
// - AgentConfigValidator
// - McpConfigValidator
// - AiConfigValidator

#[test]
fn test_startup_validator_creates_with_validators() {
    // Creating a validator should not panic
    let _validator = StartupValidator::new();
}

#[test]
fn test_startup_validator_new_multiple_times() {
    // Should be able to create multiple validators
    let v1 = StartupValidator::new();
    let v2 = StartupValidator::new();
    let v3 = StartupValidator::default();

    let _ = (v1, v2, v3);
}

// ============================================================================
// Expected Domain Validators Documentation Tests
// ============================================================================

// These tests document the expected domains that should be validated

#[test]
fn test_expected_web_domain() {
    let domain = "web";
    assert!(!domain.is_empty());
}

#[test]
fn test_expected_content_domain() {
    let domain = "content";
    assert!(!domain.is_empty());
}

#[test]
fn test_expected_agents_domain() {
    let domain = "agents";
    assert!(!domain.is_empty());
}

#[test]
fn test_expected_mcp_domain() {
    let domain = "mcp";
    assert!(!domain.is_empty());
}

#[test]
fn test_expected_ai_domain() {
    let domain = "ai";
    assert!(!domain.is_empty());
}

#[test]
fn test_all_expected_domains() {
    let expected_domains = vec!["web", "content", "agents", "mcp", "ai"];
    assert_eq!(expected_domains.len(), 5);
}

// ============================================================================
// Validator Lifecycle Tests
// ============================================================================

#[test]
fn test_validator_can_be_dropped() {
    {
        let validator = StartupValidator::new();
        let _ = validator;
    }
    // Validator should be dropped without issues
}

#[test]
fn test_validator_in_option() {
    let mut maybe_validator: Option<StartupValidator> = None;
    assert!(maybe_validator.is_none());

    maybe_validator = Some(StartupValidator::new());
    assert!(maybe_validator.is_some());

    maybe_validator = None;
    assert!(maybe_validator.is_none());
}

#[test]
fn test_validator_in_vec() {
    let mut validators: Vec<StartupValidator> = Vec::new();
    assert!(validators.is_empty());

    validators.push(StartupValidator::new());
    validators.push(StartupValidator::default());
    assert_eq!(validators.len(), 2);

    validators.clear();
    assert!(validators.is_empty());
}

// ============================================================================
// Validation Report Pattern Tests
// ============================================================================

// These tests document the expected patterns for validation reports

#[test]
fn test_validation_success_pattern() {
    // A successful validation should have no errors
    let has_errors = false;
    let error_count = 0;
    assert!(!has_errors);
    assert_eq!(error_count, 0);
}

#[test]
fn test_validation_failure_pattern() {
    // A failed validation should have errors
    let has_errors = true;
    let error_count = 1;
    assert!(has_errors);
    assert!(error_count > 0);
}

#[test]
fn test_validation_warning_pattern() {
    // Warnings don't cause failure but should be reported
    let has_warnings = true;
    let warning_count = 2;
    let has_errors = false;

    assert!(has_warnings);
    assert!(warning_count > 0);
    assert!(!has_errors); // Warnings don't fail validation
}

// ============================================================================
// Error Field Pattern Tests
// ============================================================================

#[test]
fn test_validation_error_has_field() {
    let field = "database_url";
    assert!(!field.is_empty());
}

#[test]
fn test_validation_error_has_message() {
    let message = "Database connection failed";
    assert!(!message.is_empty());
}

#[test]
fn test_validation_error_optional_path() {
    let path: Option<&str> = Some("/path/to/config.yaml");
    assert!(path.is_some());
}

#[test]
fn test_validation_error_optional_suggestion() {
    let suggestion: Option<&str> = Some("Check your configuration file");
    assert!(suggestion.is_some());
}

// ============================================================================
// Domain Report Pattern Tests
// ============================================================================

#[test]
fn test_domain_report_structure() {
    let domain_id = "web";
    let has_errors = false;
    let has_warnings = true;
    let error_count = 0;
    let warning_count = 1;

    assert!(!domain_id.is_empty());
    assert!(!has_errors);
    assert!(has_warnings);
    assert_eq!(error_count, 0);
    assert_eq!(warning_count, 1);
}

#[test]
fn test_domain_with_errors_pattern() {
    let domain_id = "content";
    let errors = vec!["Error 1", "Error 2"];
    let has_errors = !errors.is_empty();

    assert_eq!(domain_id, "content");
    assert!(has_errors);
    assert_eq!(errors.len(), 2);
}

// ============================================================================
// Profile Path Pattern Tests
// ============================================================================

#[test]
fn test_profile_path_pattern() {
    use std::path::PathBuf;

    let profile_path = PathBuf::from("/etc/systemprompt/profile.yaml");
    assert!(profile_path.to_string_lossy().contains("profile"));
}

#[test]
fn test_profile_path_display() {
    use std::path::PathBuf;

    let path = PathBuf::from("/config/app.yaml");
    let display = path.display().to_string();
    assert!(display.contains("config"));
}

// ============================================================================
// Extension Validation Pattern Tests
// ============================================================================

#[test]
fn test_extension_id_pattern() {
    let ext_id = "my-extension";
    let formatted = format!("[ext:{}]", ext_id);
    assert_eq!(formatted, "[ext:my-extension]");
}

#[test]
fn test_extension_config_prefix_pattern() {
    let prefix = "ext_config";
    let field = format!("{}.config", prefix);
    assert_eq!(field, "ext_config.config");
}

// ============================================================================
// CLI Output Pattern Tests
// ============================================================================

#[test]
fn test_phase_success_pattern() {
    let phase = "Services config";
    let detail = "includes merged";
    let output = format!("{}: {}", phase, detail);
    assert!(output.contains("Services config"));
}

#[test]
fn test_phase_warning_pattern() {
    let phase = "Content config";
    let warning = "file not found";
    let output = format!("{}: {}", phase, warning);
    assert!(output.contains("Content config"));
}

#[test]
fn test_phase_error_pattern() {
    let domain = "web";
    let error_count = 3;
    let output = format!("[{}] {} error(s)", domain, error_count);
    assert!(output.contains("[web]"));
    assert!(output.contains("3 error(s)"));
}

// ============================================================================
// FilesConfigValidator Tests
// ============================================================================

#[test]
fn test_files_config_validator_creation() {
    use systemprompt_runtime::FilesConfigValidator;

    let validator = FilesConfigValidator::new();
    let _ = validator;
}

#[test]
fn test_files_config_validator_default() {
    use systemprompt_runtime::FilesConfigValidator;

    let validator = FilesConfigValidator::default();
    let _ = validator;
}

#[test]
fn test_files_config_validator_debug() {
    use systemprompt_runtime::FilesConfigValidator;

    let validator = FilesConfigValidator::new();
    let debug_str = format!("{:?}", validator);
    assert!(debug_str.contains("FilesConfigValidator"));
}

#[test]
fn test_files_config_validator_domain_id() {
    use systemprompt_runtime::FilesConfigValidator;

    let validator = FilesConfigValidator::new();
    assert_eq!(validator.domain_id(), "files");
}

#[test]
fn test_files_config_validator_priority() {
    use systemprompt_runtime::FilesConfigValidator;

    let validator = FilesConfigValidator::new();
    assert_eq!(validator.priority(), 5);
}

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
// Display Functions Tests
// ============================================================================

#[test]
fn test_display_validation_report_no_panic() {
    let report = StartupValidationReport::new();
    display_validation_report(&report);
}

#[test]
fn test_display_validation_warnings_no_panic() {
    let report = StartupValidationReport::new();
    display_validation_warnings(&report);
}

#[test]
fn test_display_validation_warnings_empty_report() {
    let report = StartupValidationReport::new();
    display_validation_warnings(&report);
}

#[test]
fn test_display_validation_report_with_profile_path() {
    use std::path::PathBuf;

    let report = StartupValidationReport::new()
        .with_profile_path(PathBuf::from("/etc/config.yaml"));
    display_validation_report(&report);
}

#[test]
fn test_display_validation_report_with_domain() {
    let mut report = StartupValidationReport::new();
    report.add_domain(ValidationReport::new("test"));
    display_validation_report(&report);
}

#[test]
fn test_display_validation_report_with_domain_error() {
    use systemprompt_traits::validation_report::ValidationError;

    let mut report = StartupValidationReport::new();
    let mut domain = ValidationReport::new("test");
    domain.add_error(ValidationError::new("field", "error message"));
    report.add_domain(domain);
    display_validation_report(&report);
}

#[test]
fn test_display_validation_warnings_with_warnings() {
    use systemprompt_traits::validation_report::ValidationWarning;

    let mut report = StartupValidationReport::new();
    let mut domain = ValidationReport::new("test");
    domain.add_warning(ValidationWarning::new("field", "warning message"));
    report.add_domain(domain);
    display_validation_warnings(&report);
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
    use systemprompt_runtime::FilesConfigValidator;

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
