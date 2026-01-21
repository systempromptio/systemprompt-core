//! Unit tests for schema validation functions
//!
//! Tests cover:
//! - validate_yaml_str for various YAML inputs
//! - generate_schema for struct serialization
//! - ConfigValidationError variants

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_config::{generate_schema, validate_yaml_str, ConfigValidationError};

// ============================================================================
// Test Structs
// ============================================================================

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct SimpleConfig {
    name: String,
    port: u16,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct OptionalConfig {
    required_field: String,
    #[serde(default)]
    optional_field: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct NestedConfig {
    database: DatabaseConfig,
    server: ServerSettings,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct DatabaseConfig {
    url: String,
    max_connections: u32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ServerSettings {
    host: String,
    port: u16,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ConfigWithDefaults {
    #[serde(default = "default_name")]
    name: String,
    #[serde(default)]
    enabled: bool,
}

fn default_name() -> String {
    "default".to_string()
}

// ============================================================================
// validate_yaml_str Tests
// ============================================================================

#[test]
fn test_validate_yaml_str_simple_config() {
    let yaml = "name: test\nport: 8080";
    let result: Result<SimpleConfig, _> = validate_yaml_str(yaml);
    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.name, "test");
    assert_eq!(config.port, 8080);
}

#[test]
fn test_validate_yaml_str_missing_field() {
    let yaml = "name: test";
    let result: Result<SimpleConfig, _> = validate_yaml_str(yaml);
    assert!(result.is_err());
}

#[test]
fn test_validate_yaml_str_wrong_type() {
    let yaml = "name: test\nport: not_a_number";
    let result: Result<SimpleConfig, _> = validate_yaml_str(yaml);
    assert!(result.is_err());
}

#[test]
fn test_validate_yaml_str_empty() {
    let yaml = "";
    let result: Result<SimpleConfig, _> = validate_yaml_str(yaml);
    assert!(result.is_err());
}

#[test]
fn test_validate_yaml_str_invalid_yaml() {
    let yaml = "name: [invalid: yaml: structure";
    let result: Result<SimpleConfig, _> = validate_yaml_str(yaml);
    assert!(result.is_err());
}

#[test]
fn test_validate_yaml_str_optional_fields_present() {
    let yaml = "required_field: value\noptional_field: optional_value";
    let result: Result<OptionalConfig, _> = validate_yaml_str(yaml);
    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.required_field, "value");
    assert_eq!(config.optional_field, Some("optional_value".to_string()));
}

#[test]
fn test_validate_yaml_str_optional_fields_missing() {
    let yaml = "required_field: value";
    let result: Result<OptionalConfig, _> = validate_yaml_str(yaml);
    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.required_field, "value");
    assert!(config.optional_field.is_none());
}

#[test]
fn test_validate_yaml_str_nested_config() {
    let yaml = r#"
database:
  url: postgres://localhost
  max_connections: 10
server:
  host: 0.0.0.0
  port: 8080
"#;
    let result: Result<NestedConfig, _> = validate_yaml_str(yaml);
    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.database.url, "postgres://localhost");
    assert_eq!(config.database.max_connections, 10);
    assert_eq!(config.server.host, "0.0.0.0");
    assert_eq!(config.server.port, 8080);
}

#[test]
fn test_validate_yaml_str_with_defaults() {
    let yaml = "";
    let result: Result<ConfigWithDefaults, _> = validate_yaml_str(yaml);
    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.name, "default");
    assert!(!config.enabled);
}

#[test]
fn test_validate_yaml_str_override_defaults() {
    let yaml = "name: custom\nenabled: true";
    let result: Result<ConfigWithDefaults, _> = validate_yaml_str(yaml);
    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.name, "custom");
    assert!(config.enabled);
}

#[test]
fn test_validate_yaml_str_extra_fields() {
    let yaml = "name: test\nport: 8080\nextra_field: ignored";
    let result: Result<SimpleConfig, _> = validate_yaml_str(yaml);
    assert!(result.is_ok());
}

#[test]
fn test_validate_yaml_str_boundary_port() {
    let yaml = "name: test\nport: 65535";
    let result: Result<SimpleConfig, _> = validate_yaml_str(yaml);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().port, 65535);
}

#[test]
fn test_validate_yaml_str_zero_port() {
    let yaml = "name: test\nport: 0";
    let result: Result<SimpleConfig, _> = validate_yaml_str(yaml);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().port, 0);
}

// ============================================================================
// generate_schema Tests
// ============================================================================

#[test]
fn test_generate_schema_simple() {
    let result = generate_schema::<SimpleConfig>();
    assert!(result.is_ok());
    let schema = result.unwrap();
    assert!(schema.is_object());
}

#[test]
fn test_generate_schema_contains_properties() {
    let result = generate_schema::<SimpleConfig>();
    assert!(result.is_ok());
    let schema = result.unwrap();
    let properties = schema.get("properties");
    assert!(properties.is_some());
}

#[test]
fn test_generate_schema_has_name_field() {
    let result = generate_schema::<SimpleConfig>();
    assert!(result.is_ok());
    let schema = result.unwrap();
    let properties = schema.get("properties").unwrap();
    assert!(properties.get("name").is_some());
}

#[test]
fn test_generate_schema_has_port_field() {
    let result = generate_schema::<SimpleConfig>();
    assert!(result.is_ok());
    let schema = result.unwrap();
    let properties = schema.get("properties").unwrap();
    assert!(properties.get("port").is_some());
}

#[test]
fn test_generate_schema_nested() {
    let result = generate_schema::<NestedConfig>();
    assert!(result.is_ok());
    let schema = result.unwrap();
    let properties = schema.get("properties").unwrap();
    assert!(properties.get("database").is_some());
    assert!(properties.get("server").is_some());
}

#[test]
fn test_generate_schema_optional_fields() {
    let result = generate_schema::<OptionalConfig>();
    assert!(result.is_ok());
    let schema = result.unwrap();
    let properties = schema.get("properties").unwrap();
    assert!(properties.get("required_field").is_some());
    assert!(properties.get("optional_field").is_some());
}

#[test]
fn test_generate_schema_is_valid_json() {
    let result = generate_schema::<SimpleConfig>();
    assert!(result.is_ok());
    let schema = result.unwrap();
    let json_str = serde_json::to_string(&schema);
    assert!(json_str.is_ok());
}

#[test]
fn test_generate_schema_has_schema_field() {
    let result = generate_schema::<SimpleConfig>();
    assert!(result.is_ok());
    let schema = result.unwrap();
    assert!(schema.get("$schema").is_some());
}

// ============================================================================
// ConfigValidationError Tests
// ============================================================================

#[test]
fn test_config_validation_error_parse() {
    let yaml = "invalid: [yaml";
    let result: Result<SimpleConfig, ConfigValidationError> = validate_yaml_str(yaml);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = err.to_string();
    assert!(err_str.contains("YAML") || err_str.contains("parse"));
}

#[test]
fn test_config_validation_error_display() {
    let yaml = "name: 123\nport: not_a_number";
    let result: Result<SimpleConfig, ConfigValidationError> = validate_yaml_str(yaml);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let display = format!("{}", err);
    assert!(!display.is_empty());
}

#[test]
fn test_config_validation_error_debug() {
    let yaml = "invalid: [yaml";
    let result: Result<SimpleConfig, ConfigValidationError> = validate_yaml_str(yaml);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let debug = format!("{:?}", err);
    assert!(!debug.is_empty());
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_validate_yaml_str_whitespace_only() {
    // Whitespace-only YAML is parsed as null, which won't deserialize to a struct
    let yaml = "   \n\t\n   ";
    let result: Result<ConfigWithDefaults, ConfigValidationError> = validate_yaml_str(yaml);
    // This should fail because whitespace-only parses as null, not as empty map
    assert!(result.is_err());
}

#[test]
fn test_validate_yaml_str_unicode() {
    let yaml = "name: 测试名称\nport: 8080";
    let result: Result<SimpleConfig, _> = validate_yaml_str(yaml);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().name, "测试名称");
}

#[test]
fn test_validate_yaml_str_multiline_string() {
    let yaml = r#"name: |
  This is a
  multiline string
port: 8080"#;
    let result: Result<SimpleConfig, _> = validate_yaml_str(yaml);
    assert!(result.is_ok());
}

#[test]
fn test_validate_yaml_str_quoted_numbers() {
    let yaml = r#"name: "8080"
port: 8080"#;
    let result: Result<SimpleConfig, _> = validate_yaml_str(yaml);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().name, "8080");
}

#[test]
fn test_validate_yaml_str_comments() {
    let yaml = r#"
# This is a comment
name: test  # inline comment
port: 8080
"#;
    let result: Result<SimpleConfig, _> = validate_yaml_str(yaml);
    assert!(result.is_ok());
}

#[test]
fn test_validate_yaml_str_null_values() {
    let yaml = "required_field: value\noptional_field: null";
    let result: Result<OptionalConfig, _> = validate_yaml_str(yaml);
    assert!(result.is_ok());
    assert!(result.unwrap().optional_field.is_none());
}

#[test]
fn test_validate_yaml_str_tilde_null() {
    let yaml = "required_field: value\noptional_field: ~";
    let result: Result<OptionalConfig, _> = validate_yaml_str(yaml);
    assert!(result.is_ok());
    assert!(result.unwrap().optional_field.is_none());
}
