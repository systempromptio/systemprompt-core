//! Unit tests for `McpDomainError::classify` resilience classification.

use systemprompt_database::resilience::Outcome;
use systemprompt_mcp::McpDomainError;

fn is_transient(outcome: &Outcome) -> bool {
    matches!(outcome, Outcome::Transient { .. })
}

fn is_permanent(outcome: &Outcome) -> bool {
    matches!(outcome, Outcome::Permanent)
}

#[test]
fn test_connection_failed_is_transient() {
    let err = McpDomainError::ConnectionFailed {
        server: "s".to_string(),
        source: Box::new(std::io::Error::other("refused")),
    };
    assert!(is_transient(&err.classify()));
}

#[test]
fn test_transport_is_transient() {
    let err = McpDomainError::transport("token accessor", std::io::Error::other("flaky network"));
    assert!(is_transient(&err.classify()));
}

#[test]
fn test_timeout_is_transient() {
    let err = McpDomainError::Timeout {
        server: "s".to_string(),
        after_ms: 1000,
    };
    assert!(is_transient(&err.classify()));
}

#[test]
fn test_service_error_is_transient() {
    let err = McpDomainError::from(rmcp::ServiceError::TransportClosed);
    assert!(is_transient(&err.classify()));
}

#[test]
fn test_server_not_found_is_permanent() {
    let err = McpDomainError::ServerNotFound("s".to_string());
    assert!(is_permanent(&err.classify()));
}

#[test]
fn test_tool_execution_failed_is_permanent() {
    let err = McpDomainError::ToolExecutionFailed("nope".to_string());
    assert!(is_permanent(&err.classify()));
}

#[test]
fn test_schema_validation_is_permanent() {
    let err = McpDomainError::SchemaValidation("bad".to_string());
    assert!(is_permanent(&err.classify()));
}

#[test]
fn test_registry_validation_is_permanent() {
    let err = McpDomainError::RegistryValidation("bad".to_string());
    assert!(is_permanent(&err.classify()));
}

#[test]
fn test_tool_call_is_permanent() {
    let err = McpDomainError::tool_call("MCP tool call failed", std::io::Error::other("m"));
    assert!(is_permanent(&err.classify()));
}

#[test]
fn test_operation_is_permanent() {
    let err = McpDomainError::operation("spawn", std::io::Error::other("m"));
    assert!(is_permanent(&err.classify()));
}

#[test]
fn test_port_unavailable_is_permanent() {
    let err = McpDomainError::PortUnavailable {
        port: 8080,
        message: "in use".to_string(),
    };
    assert!(is_permanent(&err.classify()));
}

#[test]
fn test_circuit_open_is_permanent() {
    let err = McpDomainError::CircuitOpen {
        server: "s".to_string(),
    };
    assert!(is_permanent(&err.classify()));
}

#[test]
fn test_dependency_unavailable_is_permanent() {
    let err = McpDomainError::DependencyUnavailable {
        server: "s".to_string(),
    };
    assert!(is_permanent(&err.classify()));
}

#[test]
fn test_internal_is_permanent() {
    let err = McpDomainError::Internal("boom".to_string());
    assert!(is_permanent(&err.classify()));
}

#[test]
fn test_configuration_is_permanent() {
    let err = McpDomainError::Configuration("bad".to_string());
    assert!(is_permanent(&err.classify()));
}

#[test]
fn test_auth_required_is_permanent() {
    let err = McpDomainError::AuthRequired("svc".to_string());
    assert!(is_permanent(&err.classify()));
}

#[test]
fn test_manifest_is_permanent() {
    let err = McpDomainError::Manifest("bad".to_string());
    assert!(is_permanent(&err.classify()));
}

#[test]
fn test_path_is_permanent() {
    let err = McpDomainError::from(systemprompt_config::PathError::NotFound {
        path: "/no/such".into(),
        field: "system",
    });
    assert!(is_permanent(&err.classify()));
}

#[test]
fn test_config_validation_is_permanent() {
    let err = McpDomainError::from(
        systemprompt_models::errors::ConfigValidationError::Required("v".to_string()),
    );
    assert!(is_permanent(&err.classify()));
}
