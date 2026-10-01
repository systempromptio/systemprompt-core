use systemprompt_models::errors::{
    ConfigError, ConfigValidationError, MetadataError, ParseEnumError, SecretsError,
};

#[test]
fn parse_enum_error_display_includes_kind_and_value() {
    let e = ParseEnumError::new("color", "ultraviolet");
    assert_eq!(e.to_string(), "invalid color: ultraviolet");
    assert_eq!(e.kind, "color");
    assert_eq!(e.value, "ultraviolet");
}

#[test]
fn parse_enum_error_equality() {
    let a = ParseEnumError::new("kind", "v");
    let b = ParseEnumError::new("kind", "v");
    let c = ParseEnumError::new("other", "v");
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn parse_enum_error_owns_value_string() {
    let val = String::from("dynamic");
    let e = ParseEnumError::new("thing", val);
    assert_eq!(e.value, "dynamic");
}

#[test]
fn config_error_not_initialized_display() {
    let e = ConfigError::NotInitialized;
    let s = e.to_string();
    assert!(s.contains("Config not initialized"));
}

#[test]
fn config_error_invalid_postgres_url_display() {
    let e = ConfigError::InvalidPostgresUrl;
    let s = e.to_string();
    assert!(s.contains("PostgreSQL"));
}

#[test]
fn config_validation_error_required() {
    let e = ConfigValidationError::required("name is required");
    assert_eq!(e.to_string(), "name is required");
}

#[test]
fn config_validation_error_invalid_field() {
    let e = ConfigValidationError::invalid_field("bad port");
    assert_eq!(e.to_string(), "bad port");
}

#[test]
fn config_validation_error_port_conflict() {
    let e = ConfigValidationError::port_conflict("port 8080 in use");
    assert_eq!(e.to_string(), "port 8080 in use");
}

#[test]
fn config_validation_error_unknown_reference() {
    let e = ConfigValidationError::unknown_reference("ref agent_x not found");
    assert_eq!(e.to_string(), "ref agent_x not found");
}

#[test]
fn config_validation_error_circular_dependency() {
    let e = ConfigValidationError::circular_dependency("a → b → a");
    assert_eq!(e.to_string(), "a → b → a");
}

#[test]
fn config_validation_error_business_rule() {
    let e = ConfigValidationError::business_rule("max 5 agents");
    assert_eq!(e.to_string(), "max 5 agents");
}

#[test]
fn config_validation_error_missing_system_admin_display() {
    let e = ConfigValidationError::MissingSystemAdmin;
    let s = e.to_string();
    assert!(s.contains("system_admin.username"));
}

#[test]
fn secrets_error_invalid_display() {
    let e = SecretsError::Invalid("bad key".to_owned());
    assert_eq!(e.to_string(), "bad key");
}

#[test]
fn metadata_error_variants_display() {
    assert!(
        MetadataError::MissingExecutionId
            .to_string()
            .contains("mcp_execution_id")
    );
    assert!(MetadataError::MetaMissing.to_string().contains("_meta"));
    assert!(
        MetadataError::NotJsonObject
            .to_string()
            .contains("JSON object")
    );
}

