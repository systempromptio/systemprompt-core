//! Tests for AuthMode::Optional, header extraction, and authorization header format

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_models::auth::UserType;
use systemprompt_security::AuthMode;

use super::helpers::*;

// ============================================================================
// AuthMode::Optional Tests
// ============================================================================

#[test]
fn test_validate_request_optional_no_token() {
    let service = create_test_service();
    let headers = HeaderMap::new();

    let result = service.validate_request(&headers, AuthMode::Optional);
    assert!(result.is_ok());

    let context = result.unwrap();
    assert_eq!(context.request.session_id.as_str(), "anonymous");
    assert_eq!(context.auth.user_type, UserType::Anon);
}

#[test]
fn test_validate_request_optional_invalid_token() {
    let service = create_test_service();
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_static("Bearer invalid_token"),
    );

    let result = service.validate_request(&headers, AuthMode::Optional);
    assert!(result.is_ok());

    let context = result.unwrap();
    assert_eq!(context.request.session_id.as_str(), "anonymous");
    assert_eq!(context.auth.user_type, UserType::Anon);
}

#[test]
fn test_validate_request_optional_valid_token() {
    let service = create_test_service();
    let token = create_valid_jwt(
        "test_secret_key",
        "test_issuer",
        Some("session_456".to_string()),
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );

    let result = service.validate_request(&headers, AuthMode::Optional);
    assert!(result.is_ok());

    let context = result.unwrap();
    assert_eq!(context.auth.user_id.as_str(), "user_123");
    assert_eq!(context.request.session_id.as_str(), "session_456");
    assert_eq!(context.auth.user_type, UserType::User);
}

// ============================================================================
// Header Extraction Tests
// ============================================================================

#[test]
fn test_validate_request_extracts_trace_id() {
    let service = create_test_service();
    let token = create_valid_jwt(
        "test_secret_key",
        "test_issuer",
        Some("session".to_string()),
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );
    headers.insert("x-trace-id", HeaderValue::from_static("custom-trace-id"));

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_ok());

    let context = result.unwrap();
    assert_eq!(context.execution.trace_id.as_str(), "custom-trace-id");
}

#[test]
fn test_validate_request_extracts_context_id() {
    let service = create_test_service();
    let token = create_valid_jwt(
        "test_secret_key",
        "test_issuer",
        Some("session".to_string()),
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );
    headers.insert(
        "x-context-id",
        HeaderValue::from_static("custom-context-id"),
    );

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_ok());

    let context = result.unwrap();
    assert_eq!(context.execution.context_id.as_str(), "custom-context-id");
}

#[test]
fn test_validate_request_extracts_agent_name() {
    let service = create_test_service();
    let token = create_valid_jwt(
        "test_secret_key",
        "test_issuer",
        Some("session".to_string()),
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );
    headers.insert("x-agent-name", HeaderValue::from_static("custom-agent"));

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_ok());

    let context = result.unwrap();
    assert_eq!(context.execution.agent_name.as_str(), "custom-agent");
}

#[test]
fn test_validate_request_generates_trace_id_if_missing() {
    let service = create_test_service();
    let token = create_valid_jwt(
        "test_secret_key",
        "test_issuer",
        Some("session".to_string()),
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_ok());

    let context = result.unwrap();
    assert!(!context.execution.trace_id.as_str().is_empty());
}

#[test]
fn test_validate_request_anonymous_extracts_headers() {
    let service = create_test_service();
    let mut headers = HeaderMap::new();
    headers.insert("x-trace-id", HeaderValue::from_static("anon-trace"));
    headers.insert("x-context-id", HeaderValue::from_static("anon-context"));
    headers.insert("x-agent-name", HeaderValue::from_static("anon-agent"));

    let result = service.validate_request(&headers, AuthMode::Optional);
    assert!(result.is_ok());

    let context = result.unwrap();
    assert_eq!(context.execution.trace_id.as_str(), "anon-trace");
    assert_eq!(context.execution.context_id.as_str(), "anon-context");
    assert_eq!(context.execution.agent_name.as_str(), "anon-agent");
}

// ============================================================================
// Authorization Header Format Tests
// ============================================================================

#[test]
fn test_validate_request_lowercase_authorization() {
    let service = create_test_service();
    let token = create_valid_jwt(
        "test_secret_key",
        "test_issuer",
        Some("session".to_string()),
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_ok());
}

#[test]
fn test_validate_request_no_bearer_prefix() {
    let service = create_test_service();
    let token = create_valid_jwt(
        "test_secret_key",
        "test_issuer",
        Some("session".to_string()),
    );

    let mut headers = HeaderMap::new();
    headers.insert("authorization", HeaderValue::from_str(&token).unwrap());

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_err());
}

#[test]
fn test_validate_request_basic_auth() {
    let service = create_test_service();
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_static("Basic dXNlcjpwYXNz"),
    );

    let result = service.validate_request(&headers, AuthMode::Required);
    assert!(result.is_err());
}
