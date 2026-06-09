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
fn test_transport_error_display() {
    let err = McpDomainError::Transport("eof".to_string());
    assert!(err.to_string().contains("eof"));
}

#[test]
fn test_path_error_display() {
    let err = McpDomainError::Path("/no/such".to_string());
    assert!(err.to_string().contains("/no/such"));
}

#[test]
fn test_config_validation_display() {
    let err = McpDomainError::ConfigValidation("nope".to_string());
    assert!(err.to_string().contains("nope"));
}

#[test]
fn test_service_error_display() {
    let err = McpDomainError::ServiceError {
        message: "ouch".to_string(),
    };
    assert!(err.to_string().contains("ouch"));
}

#[test]
fn test_client_initialize_display() {
    let err = McpDomainError::ClientInitialize("badinit".to_string());
    assert!(err.to_string().contains("badinit"));
}
