//! Unit tests for `McpDomainError` `From` conversions.

use systemprompt_mcp::McpDomainError;

#[test]
fn test_from_sqlx_row_not_found() {
    let err: McpDomainError = sqlx::Error::RowNotFound.into();
    let s = err.to_string();
    assert!(!s.is_empty());
}

#[test]
fn test_from_serde_json() {
    let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
    let err: McpDomainError = json_err.into();
    let s = err.to_string();
    assert!(!s.is_empty());
}

#[test]
fn test_from_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
    let err: McpDomainError = io_err.into();
    let s = err.to_string();
    assert!(s.contains("missing") || !s.is_empty());
}

#[test]
fn test_internal_error_construction() {
    let err = McpDomainError::Internal("oops".to_string());
    assert!(err.to_string().contains("oops"));
}

#[test]
fn test_circuit_open_display_contains_server() {
    let err = McpDomainError::CircuitOpen {
        server: "alpha".to_string(),
    };
    assert!(err.to_string().contains("alpha"));
}

#[test]
fn test_dependency_unavailable_display_contains_server() {
    let err = McpDomainError::DependencyUnavailable {
        server: "beta".to_string(),
    };
    assert!(err.to_string().contains("beta"));
}

#[test]
fn test_timeout_display_contains_server_and_ms() {
    let err = McpDomainError::Timeout {
        server: "x".to_string(),
        after_ms: 4242,
    };
    let s = err.to_string();
    assert!(s.contains("x"));
    assert!(s.contains("4242"));
}

#[test]
fn test_manifest_error_display() {
    let err = McpDomainError::Manifest("bad manifest".to_string());
    assert!(err.to_string().contains("bad manifest"));
}

#[test]
fn test_transport_error_keeps_its_source() {
    let err = McpDomainError::transport("token accessor", std::io::Error::other("eof"));
    assert!(err.to_string().contains("eof"));
    let source = std::error::Error::source(&err).expect("transport keeps its cause");
    assert_eq!(source.to_string(), "eof");
}

#[test]
fn test_from_path_error_is_typed() {
    let err: McpDomainError = systemprompt_config::PathError::NotFound {
        path: "/no/such".into(),
        field: "system",
    }
    .into();
    assert!(matches!(err, McpDomainError::Path(_)));
    assert!(err.to_string().contains("/no/such"));
}

#[test]
fn test_operation_error_keeps_its_source() {
    let err =
        McpDomainError::operation("Failed to read schema file", std::io::Error::other("ouch"));
    assert!(err.to_string().contains("Failed to read schema file"));
    assert!(std::error::Error::source(&err).is_some());
}

#[test]
fn test_from_rmcp_service_error_is_typed() {
    let err: McpDomainError = rmcp::ServiceError::TransportClosed.into();
    assert!(matches!(err, McpDomainError::ServiceError(_)));
    assert!(std::error::Error::source(&err).is_some());
}

#[test]
fn test_from_rmcp_service_error_is_transient() {
    let err: McpDomainError = rmcp::ServiceError::TransportClosed.into();
    assert!(err.to_string().contains("Transport closed"));
    assert!(matches!(
        err.classify(),
        systemprompt_database::resilience::Outcome::Transient { retry_after: None }
    ));
}

#[test]
fn test_from_config_validation_error() {
    let source = systemprompt_models::errors::ConfigValidationError::Required(
        "database.url is required".to_string(),
    );
    let err: McpDomainError = source.into();
    assert!(matches!(err, McpDomainError::ConfigValidation(_)));
    assert!(err.to_string().contains("database.url is required"));
    assert!(matches!(
        err.classify(),
        systemprompt_database::resilience::Outcome::Permanent
    ));
}
