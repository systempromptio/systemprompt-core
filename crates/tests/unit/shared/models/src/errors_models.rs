use systemprompt_models::errors::{
    GlobalConfigError, MetadataError, ParseEnumError, SecretsError, ServicesValidationError,
    ValidationSubject,
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
    let e = GlobalConfigError::NotInitialized;
    let s = e.to_string();
    assert!(s.contains("Config not initialized"));
}

#[test]
fn config_error_invalid_postgres_url_display() {
    let e = GlobalConfigError::InvalidPostgresUrl;
    let s = e.to_string();
    assert!(s.contains("PostgreSQL"));
}

#[test]
fn config_validation_error_required() {
    let e = ServicesValidationError::required(
        ValidationSubject::named("Agent", "alpha"),
        "name is required",
    );
    assert_eq!(e.to_string(), "Agent 'alpha': name is required");
}

#[test]
fn config_validation_error_invalid_field() {
    let e = ServicesValidationError::invalid_field(
        ValidationSubject::clause("Agent", "alpha"),
        "has invalid port 0",
    );
    assert_eq!(e.to_string(), "Agent 'alpha' has invalid port 0");
}

#[test]
fn config_validation_error_port_conflict() {
    let e = ServicesValidationError::port_conflict(8080, ("agent", "a"), ("mcp_server", "m"));
    assert_eq!(
        e.to_string(),
        "Port conflict: 8080 used by both agent 'a' and mcp_server 'm'"
    );
}

#[test]
fn config_validation_error_unknown_reference() {
    let e = ServicesValidationError::unknown_reference(
        ValidationSubject::scope("marketplace.rings"),
        "ref agent_x not found",
    );
    assert_eq!(e.to_string(), "marketplace.rings: ref agent_x not found");
}

#[test]
fn config_validation_error_circular_dependency() {
    let e = ServicesValidationError::circular_dependency(vec![
        "a".to_owned(),
        "b".to_owned(),
        "a".to_owned(),
    ]);
    assert_eq!(e.to_string(), "a → b → a");
}

#[test]
fn config_validation_error_business_rule() {
    let e = ServicesValidationError::business_rule(ValidationSubject::Global, "max 5 agents");
    assert_eq!(e.to_string(), "max 5 agents");
}

#[test]
fn secrets_error_pepper_too_short_display() {
    let e = SecretsError::PepperTooShort { min: 32, actual: 4 };
    assert_eq!(
        e.to_string(),
        "oauth_at_rest_pepper must be at least 32 characters (got 4)"
    );
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
