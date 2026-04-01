//! Unit tests for ConfigValidator::validate() - required variables, URL format, unresolved variables

use std::collections::HashMap;
use systemprompt_config::{ConfigValidator, DeployEnvironment, EnvironmentConfig};

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

