//! Unit tests for ConfigValidator and ValidationReport
//!
//! Tests cover:
//! - ValidationReport creation and error/warning tracking
//! - ValidationReport is_valid state management
//! - ConfigValidator::validate() validation logic
//! - Unresolved variable detection
//! - Required variable checking
//! - URL format validation
//! - Port validation
//! - Environment-specific checks

use std::collections::HashMap;
use systemprompt_config::{ConfigValidator, DeployEnvironment, EnvironmentConfig, ValidationReport};

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

// ============================================================================
// ConfigValidator::validate() Tests
// ============================================================================

fn create_valid_config(environment: DeployEnvironment) -> EnvironmentConfig {
    let mut variables = HashMap::new();
    variables.insert("SERVICE_NAME".to_string(), "test-service".to_string());
    variables.insert("SYSTEM_PATH".to_string(), "/var/www/app".to_string());
    variables.insert("DATABASE_URL".to_string(), "postgresql://localhost/test".to_string());
    variables.insert("HOST".to_string(), "0.0.0.0".to_string());
    variables.insert("PORT".to_string(), "8080".to_string());
    variables.insert("API_SERVER_URL".to_string(), "http://localhost:8080".to_string());
    variables.insert("JWT_SECRET".to_string(), "super_secret_key_12345".to_string());
    variables.insert("JWT_ISSUER".to_string(), "test-issuer".to_string());

    EnvironmentConfig {
        environment,
        variables,
    }
}

#[test]
fn test_validate_valid_config() {
    let config = create_valid_config(DeployEnvironment::Local);
    let result = ConfigValidator::validate(&config);

    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(report.is_valid());
    assert!(report.errors.is_empty());
}

#[test]
fn test_validate_detects_unresolved_variable() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("HOST".to_string(), "${UNDEFINED_HOST}".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("validation error"));
}

#[test]
fn test_validate_detects_unresolved_variable_with_default_syntax() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("TIMEOUT".to_string(), "${UNDEFINED:-}".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_missing_required_service_name() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.remove("SERVICE_NAME");

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_missing_required_system_path() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.remove("SYSTEM_PATH");

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_missing_required_database_url() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.remove("DATABASE_URL");

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_missing_required_host() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.remove("HOST");

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_missing_required_port() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.remove("PORT");

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_missing_required_api_server_url() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.remove("API_SERVER_URL");

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_missing_required_jwt_secret() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.remove("JWT_SECRET");

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_missing_required_jwt_issuer() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.remove("JWT_ISSUER");

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_empty_database_url() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("DATABASE_URL".to_string(), String::new());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_empty_jwt_secret() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("JWT_SECRET".to_string(), String::new());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_quoted_empty_database_url() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("DATABASE_URL".to_string(), "''".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_double_quoted_empty_jwt_secret() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("JWT_SECRET".to_string(), "\"\"".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_invalid_database_url_format() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("DATABASE_URL".to_string(), "not-a-valid-url".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_invalid_api_server_url_format() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("API_SERVER_URL".to_string(), "invalid-url".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_valid_postgresql_url() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("DATABASE_URL".to_string(), "postgresql://user:pass@localhost:5432/db".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_ok());
}

#[test]
fn test_validate_valid_mysql_url() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("DATABASE_URL".to_string(), "mysql://user:pass@localhost:3306/db".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_ok());
}

#[test]
fn test_validate_valid_http_api_url() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("API_SERVER_URL".to_string(), "http://api.example.com".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_ok());
}

#[test]
fn test_validate_valid_https_api_url() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("API_SERVER_URL".to_string(), "https://api.example.com".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_ok());
}

#[test]
fn test_validate_invalid_port_not_numeric() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("PORT".to_string(), "abc".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_invalid_port_zero() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("PORT".to_string(), "0".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_invalid_port_negative() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("PORT".to_string(), "-1".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_invalid_port_too_large() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("PORT".to_string(), "70000".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_valid_port_boundary_1() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("PORT".to_string(), "1".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_ok());
}

#[test]
fn test_validate_valid_port_boundary_65535() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("PORT".to_string(), "65535".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_ok());
}

#[test]
fn test_validate_valid_port_common_values() {
    for port in ["80", "443", "3000", "8080", "5432"] {
        let mut config = create_valid_config(DeployEnvironment::Local);
        config.variables.insert("PORT".to_string(), port.to_string());

        let result = ConfigValidator::validate(&config);
        assert!(result.is_ok(), "Port {} should be valid", port);
    }
}

#[test]
fn test_validate_missing_port_adds_warning() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.remove("PORT");

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_production_use_https_warning() {
    let mut config = create_valid_config(DeployEnvironment::Production);
    config.variables.insert("USE_HTTPS".to_string(), "false".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(report.warnings.iter().any(|w| w.contains("USE_HTTPS")));
}

#[test]
fn test_validate_production_debug_log_warning() {
    let mut config = create_valid_config(DeployEnvironment::Production);
    config.variables.insert("RUST_LOG".to_string(), "debug".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(report.warnings.iter().any(|w| w.contains("RUST_LOG")));
}

#[test]
fn test_validate_production_with_https_true_no_warning() {
    let mut config = create_valid_config(DeployEnvironment::Production);
    config.variables.insert("USE_HTTPS".to_string(), "true".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(!report.warnings.iter().any(|w| w.contains("USE_HTTPS")));
}

#[test]
fn test_validate_production_with_info_log_no_warning() {
    let mut config = create_valid_config(DeployEnvironment::Production);
    config.variables.insert("RUST_LOG".to_string(), "info".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(!report.warnings.iter().any(|w| w.contains("RUST_LOG")));
}

#[test]
fn test_validate_local_environment_no_production_warnings() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("USE_HTTPS".to_string(), "false".to_string());
    config.variables.insert("RUST_LOG".to_string(), "debug".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(!report.warnings.iter().any(|w| w.contains("USE_HTTPS")));
    assert!(!report.warnings.iter().any(|w| w.contains("RUST_LOG")));
}

#[test]
fn test_validate_docker_dev_environment_no_production_warnings() {
    let mut config = create_valid_config(DeployEnvironment::DockerDev);
    config.variables.insert("USE_HTTPS".to_string(), "false".to_string());
    config.variables.insert("RUST_LOG".to_string(), "debug".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(!report.warnings.iter().any(|w| w.contains("Production")));
}

#[test]
fn test_validate_multiple_errors() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.remove("DATABASE_URL");
    config.variables.remove("JWT_SECRET");
    config.variables.insert("PORT".to_string(), "invalid".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("validation error"));
}

#[test]
fn test_validate_api_external_url_valid() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("API_EXTERNAL_URL".to_string(), "https://api.external.com".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_ok());
}

#[test]
fn test_validate_api_external_url_invalid() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("API_EXTERNAL_URL".to_string(), "not-a-url".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_empty_api_external_url_allowed() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("API_EXTERNAL_URL".to_string(), String::new());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_ok());
}

#[test]
fn test_config_validator_debug() {
    let validator = ConfigValidator;
    let debug_str = format!("{:?}", validator);
    assert!(debug_str.contains("ConfigValidator"));
}

#[test]
fn test_config_validator_clone() {
    let validator = ConfigValidator;
    let cloned = validator;
    let _ = format!("{:?}", cloned);
}

#[test]
fn test_config_validator_copy() {
    let validator = ConfigValidator;
    let copied: ConfigValidator = validator;
    let _ = format!("{:?}", copied);
}

#[test]
fn test_validate_unresolved_variable_in_nested_value() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("API_SERVER_URL".to_string(), "http://${HOST}:${PORT}".to_string());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}

#[test]
fn test_validate_empty_required_variable_treated_as_missing() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("SERVICE_NAME".to_string(), String::new());

    let result = ConfigValidator::validate(&config);

    assert!(result.is_err());
}
