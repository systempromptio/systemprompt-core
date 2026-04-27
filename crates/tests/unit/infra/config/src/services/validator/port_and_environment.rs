//! Unit tests for ConfigValidator - port validation and environment-specific
//! checks

use std::collections::HashMap;
use systemprompt_config::{ConfigValidator, DeployEnvironment, EnvironmentConfig};

fn create_valid_config(environment: DeployEnvironment) -> EnvironmentConfig {
    let mut variables = HashMap::new();
    variables.insert("SERVICE_NAME".to_string(), "test-service".to_string());
    variables.insert("SYSTEM_PATH".to_string(), "/var/www/app".to_string());
    variables.insert(
        "DATABASE_URL".to_string(),
        "postgresql://localhost/test".to_string(),
    );
    variables.insert("HOST".to_string(), "0.0.0.0".to_string());
    variables.insert("PORT".to_string(), "8080".to_string());
    variables.insert(
        "API_SERVER_URL".to_string(),
        "http://localhost:8080".to_string(),
    );
    variables.insert(
        "JWT_SECRET".to_string(),
        "super_secret_key_12345".to_string(),
    );
    variables.insert("JWT_ISSUER".to_string(), "test-issuer".to_string());

    EnvironmentConfig {
        environment,
        variables,
    }
}

// ============================================================================
// Additional Validation Tests
// ============================================================================

#[test]
fn test_validate_unresolved_variable_in_nested_value() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert(
        "API_SERVER_URL".to_string(),
        "http://${HOST}:${PORT}".to_string(),
    );

    ConfigValidator::validate(&config).unwrap_err();
}

#[test]
fn test_validate_empty_required_variable_treated_as_missing() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config
        .variables
        .insert("SERVICE_NAME".to_string(), String::new());

    ConfigValidator::validate(&config).unwrap_err();
}

// ============================================================================
// Port Validation Tests
// ============================================================================

#[test]
fn test_validate_invalid_port_not_numeric() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config
        .variables
        .insert("PORT".to_string(), "abc".to_string());

    ConfigValidator::validate(&config).unwrap_err();
}

#[test]
fn test_validate_invalid_port_zero() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("PORT".to_string(), "0".to_string());

    ConfigValidator::validate(&config).unwrap_err();
}

#[test]
fn test_validate_invalid_port_negative() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config
        .variables
        .insert("PORT".to_string(), "-1".to_string());

    ConfigValidator::validate(&config).unwrap_err();
}

#[test]
fn test_validate_invalid_port_too_large() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config
        .variables
        .insert("PORT".to_string(), "70000".to_string());

    ConfigValidator::validate(&config).unwrap_err();
}

#[test]
fn test_validate_valid_port_boundary_1() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.insert("PORT".to_string(), "1".to_string());

    ConfigValidator::validate(&config).expect("port 1 should be valid");
}

#[test]
fn test_validate_valid_port_boundary_65535() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config
        .variables
        .insert("PORT".to_string(), "65535".to_string());

    ConfigValidator::validate(&config).expect("port 65535 should be valid");
}

#[test]
fn test_validate_valid_port_common_values() {
    for port in ["80", "443", "3000", "8080", "5432"] {
        let mut config = create_valid_config(DeployEnvironment::Local);
        config
            .variables
            .insert("PORT".to_string(), port.to_string());

        ConfigValidator::validate(&config)
            .unwrap_or_else(|_| panic!("Port {} should be valid", port));
    }
}

#[test]
fn test_validate_missing_port_adds_warning() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config.variables.remove("PORT");

    ConfigValidator::validate(&config).unwrap_err();
}

// ============================================================================
// Environment-Specific Tests
// ============================================================================

#[test]
fn test_validate_production_use_https_warning() {
    let mut config = create_valid_config(DeployEnvironment::Production);
    config
        .variables
        .insert("USE_HTTPS".to_string(), "false".to_string());

    let report = ConfigValidator::validate(&config)
        .expect("production config with USE_HTTPS=false should pass with warnings");
    assert!(report.warnings.iter().any(|w| w.contains("USE_HTTPS")));
}

#[test]
fn test_validate_production_debug_log_warning() {
    let mut config = create_valid_config(DeployEnvironment::Production);
    config
        .variables
        .insert("RUST_LOG".to_string(), "debug".to_string());

    let report = ConfigValidator::validate(&config)
        .expect("production config with debug log should pass with warnings");
    assert!(report.warnings.iter().any(|w| w.contains("RUST_LOG")));
}

#[test]
fn test_validate_production_with_https_true_no_warning() {
    let mut config = create_valid_config(DeployEnvironment::Production);
    config
        .variables
        .insert("USE_HTTPS".to_string(), "true".to_string());

    let report = ConfigValidator::validate(&config)
        .expect("production config with USE_HTTPS=true should pass");
    assert!(!report.warnings.iter().any(|w| w.contains("USE_HTTPS")));
}

#[test]
fn test_validate_production_with_info_log_no_warning() {
    let mut config = create_valid_config(DeployEnvironment::Production);
    config
        .variables
        .insert("RUST_LOG".to_string(), "info".to_string());

    let report =
        ConfigValidator::validate(&config).expect("production config with info log should pass");
    assert!(!report.warnings.iter().any(|w| w.contains("RUST_LOG")));
}

#[test]
fn test_validate_local_environment_no_production_warnings() {
    let mut config = create_valid_config(DeployEnvironment::Local);
    config
        .variables
        .insert("USE_HTTPS".to_string(), "false".to_string());
    config
        .variables
        .insert("RUST_LOG".to_string(), "debug".to_string());

    let report = ConfigValidator::validate(&config)
        .expect("local config should pass without production warnings");
    assert!(!report.warnings.iter().any(|w| w.contains("USE_HTTPS")));
    assert!(!report.warnings.iter().any(|w| w.contains("RUST_LOG")));
}

#[test]
fn test_validate_docker_dev_environment_no_production_warnings() {
    let mut config = create_valid_config(DeployEnvironment::DockerDev);
    config
        .variables
        .insert("USE_HTTPS".to_string(), "false".to_string());
    config
        .variables
        .insert("RUST_LOG".to_string(), "debug".to_string());

    let report = ConfigValidator::validate(&config)
        .expect("docker-dev config should pass without production warnings");
    assert!(!report.warnings.iter().any(|w| w.contains("Production")));
}
