//! Unit tests for AgentServiceError
//!
//! Tests cover:
//! - Error variant creation and display messages
//! - Error conversions from other error types

use systemprompt_core_agent::services::shared::error::AgentServiceError;

// ============================================================================
// AgentServiceError Variant Tests
// ============================================================================

#[test]
fn test_agent_service_error_database() {
    let error = AgentServiceError::Database("Connection refused".to_string());
    assert!(error.to_string().contains("database operation failed"));
    assert!(error.to_string().contains("Connection refused"));
}

#[test]
fn test_agent_service_error_repository() {
    let error = AgentServiceError::Repository("Entity not found".to_string());
    assert!(error.to_string().contains("repository operation failed"));
    assert!(error.to_string().contains("Entity not found"));
}

#[test]
fn test_agent_service_error_network() {
    let error = AgentServiceError::Network("https://api.example.com".to_string());
    assert!(error.to_string().contains("network request failed"));
    assert!(error.to_string().contains("https://api.example.com"));
}

#[test]
fn test_agent_service_error_authentication() {
    let error = AgentServiceError::Authentication("Invalid token".to_string());
    assert!(error.to_string().contains("authentication failed"));
    assert!(error.to_string().contains("Invalid token"));
}

#[test]
fn test_agent_service_error_authorization() {
    let error = AgentServiceError::Authorization("admin-resource".to_string());
    assert!(error.to_string().contains("authorization failed"));
    assert!(error.to_string().contains("admin-resource"));
}

#[test]
fn test_agent_service_error_validation() {
    let error = AgentServiceError::Validation("email".to_string(), "invalid format".to_string());
    assert!(error.to_string().contains("validation failed"));
    assert!(error.to_string().contains("email"));
    assert!(error.to_string().contains("invalid format"));
}

#[test]
fn test_agent_service_error_not_found() {
    let error = AgentServiceError::NotFound("Agent agent-123".to_string());
    assert!(error.to_string().contains("resource not found"));
    assert!(error.to_string().contains("Agent agent-123"));
}

#[test]
fn test_agent_service_error_service_unavailable() {
    let error = AgentServiceError::ServiceUnavailable("Database is down".to_string());
    assert!(error.to_string().contains("service unavailable"));
    assert!(error.to_string().contains("Database is down"));
}

#[test]
fn test_agent_service_error_timeout() {
    let error = AgentServiceError::Timeout(30000);
    assert!(error.to_string().contains("operation timed out"));
    assert!(error.to_string().contains("30000"));
}

#[test]
fn test_agent_service_error_configuration() {
    let error = AgentServiceError::Configuration(
        "ServiceConfig".to_string(),
        "missing required field".to_string(),
    );
    assert!(error.to_string().contains("configuration error"));
    assert!(error.to_string().contains("ServiceConfig"));
    assert!(error.to_string().contains("missing required field"));
}

#[test]
fn test_agent_service_error_conflict() {
    let error = AgentServiceError::Conflict("Agent with same name exists".to_string());
    assert!(error.to_string().contains("conflict"));
    assert!(error.to_string().contains("Agent with same name exists"));
}

#[test]
fn test_agent_service_error_internal() {
    let error = AgentServiceError::Internal("Unexpected state".to_string());
    assert!(error.to_string().contains("internal error"));
    assert!(error.to_string().contains("Unexpected state"));
}

#[test]
fn test_agent_service_error_logging() {
    let error = AgentServiceError::Logging("Failed to write log".to_string());
    assert!(error.to_string().contains("logging error"));
    assert!(error.to_string().contains("Failed to write log"));
}

#[test]
fn test_agent_service_error_capacity() {
    let error = AgentServiceError::Capacity("Connection pool exhausted".to_string());
    assert!(error.to_string().contains("capacity exceeded"));
    assert!(error.to_string().contains("Connection pool exhausted"));
}

// ============================================================================
// Error Debug Tests
// ============================================================================

#[test]
fn test_agent_service_error_debug_database() {
    let error = AgentServiceError::Database("test".to_string());
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("Database"));
}

#[test]
fn test_agent_service_error_debug_validation() {
    let error = AgentServiceError::Validation("field".to_string(), "reason".to_string());
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("Validation"));
    assert!(debug_str.contains("field"));
    assert!(debug_str.contains("reason"));
}

#[test]
fn test_agent_service_error_debug_timeout() {
    let error = AgentServiceError::Timeout(5000);
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("Timeout"));
    assert!(debug_str.contains("5000"));
}

// ============================================================================
// Error Type Matching Tests
// ============================================================================

#[test]
fn test_agent_service_error_match_database() {
    let error = AgentServiceError::Database("error".to_string());
    match error {
        AgentServiceError::Database(msg) => assert_eq!(msg, "error"),
        _ => panic!("Expected Database variant"),
    }
}

#[test]
fn test_agent_service_error_match_not_found() {
    let error = AgentServiceError::NotFound("resource".to_string());
    match error {
        AgentServiceError::NotFound(resource) => assert_eq!(resource, "resource"),
        _ => panic!("Expected NotFound variant"),
    }
}

#[test]
fn test_agent_service_error_match_timeout() {
    let error = AgentServiceError::Timeout(1000);
    match error {
        AgentServiceError::Timeout(ms) => assert_eq!(ms, 1000),
        _ => panic!("Expected Timeout variant"),
    }
}

// ============================================================================
// Result Type Tests
// ============================================================================

#[test]
fn test_result_ok() {
    let result: systemprompt_core_agent::services::shared::error::Result<i32> = Ok(42);
    assert_eq!(result.unwrap(), 42);
}

#[test]
fn test_result_err() {
    let result: systemprompt_core_agent::services::shared::error::Result<i32> =
        Err(AgentServiceError::NotFound("item".to_string()));
    assert!(result.is_err());
}

// ============================================================================
// Error Message Formatting Tests
// ============================================================================

#[test]
fn test_error_message_includes_context() {
    let error = AgentServiceError::Validation(
        "password".to_string(),
        "must be at least 8 characters".to_string(),
    );
    let message = error.to_string();

    assert!(message.contains("password"));
    assert!(message.contains("must be at least 8 characters"));
}

#[test]
fn test_error_message_timeout_includes_duration() {
    let error = AgentServiceError::Timeout(60000);
    let message = error.to_string();

    assert!(message.contains("60000"));
    assert!(message.contains("ms"));
}
