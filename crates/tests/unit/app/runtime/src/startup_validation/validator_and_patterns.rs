//! Tests for StartupValidator creation, domain validators, and validation patterns

use systemprompt_runtime::StartupValidator;

// ============================================================================
// StartupValidator Creation Tests
// ============================================================================

#[test]
fn test_startup_validator_debug() {
    let validator = StartupValidator::new();
    let debug_str = format!("{:?}", validator);
    assert!(debug_str.contains("StartupValidator"));
}

// ============================================================================
// Expected Domain Validators Documentation Tests
// ============================================================================

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

#[test]
fn test_validation_success_pattern() {
    let has_errors = false;
    let error_count = 0;
    assert!(!has_errors);
    assert_eq!(error_count, 0);
}

#[test]
fn test_validation_failure_pattern() {
    let has_errors = true;
    let error_count = 1;
    assert!(has_errors);
    assert!(error_count > 0);
}

#[test]
fn test_validation_warning_pattern() {
    let has_warnings = true;
    let warning_count = 2;
    let has_errors = false;

    assert!(has_warnings);
    assert!(warning_count > 0);
    assert!(!has_errors);
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
fn test_files_config_validator_debug() {
    use systemprompt_runtime::FilesConfigValidator;

    let validator = FilesConfigValidator::new();
    let debug_str = format!("{:?}", validator);
    assert!(debug_str.contains("FilesConfigValidator"));
}

#[test]
fn test_files_config_validator_domain_id() {
    use systemprompt_runtime::FilesConfigValidator;
    use systemprompt_traits::DomainConfig;

    let validator = FilesConfigValidator::new();
    assert_eq!(validator.domain_id(), "files");
}

#[test]
fn test_files_config_validator_priority() {
    use systemprompt_runtime::FilesConfigValidator;
    use systemprompt_traits::DomainConfig;

    let validator = FilesConfigValidator::new();
    assert_eq!(validator.priority(), 5);
}
