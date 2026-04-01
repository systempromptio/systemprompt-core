//! Unit tests for RBAC middleware types

use systemprompt_mcp::middleware::{AuthResult, AuthenticatedRequestContext};
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId};
use systemprompt_models::RequestContext;

/// Create a test RequestContext for use in unit tests
fn create_test_context() -> RequestContext {
    RequestContext::new(
        SessionId::new("test_session".to_string()),
        TraceId::new("test_trace".to_string()),
        ContextId::new("test_context".to_string()),
        AgentName::new("test_agent".to_string()),
    )
}

// ============================================================================
// AuthenticatedRequestContext Tests
// ============================================================================

#[test]
fn test_authenticated_request_context_new() {
    let context = create_test_context();
    let token = "test_token".to_string();
    let auth_ctx = AuthenticatedRequestContext::new(context, token.clone());

    assert_eq!(auth_ctx.token(), "test_token");
}

#[test]
fn test_authenticated_request_context_token() {
    let context = create_test_context();
    let token = "bearer_abc123".to_string();
    let auth_ctx = AuthenticatedRequestContext::new(context, token);

    assert_eq!(auth_ctx.token(), "bearer_abc123");
}

#[test]
fn test_authenticated_request_context_empty_token() {
    let context = create_test_context();
    let token = String::new();
    let auth_ctx = AuthenticatedRequestContext::new(context, token);

    assert_eq!(auth_ctx.token(), "");
}

#[test]
fn test_authenticated_request_context_debug() {
    let context = create_test_context();
    let token = "test_token".to_string();
    let auth_ctx = AuthenticatedRequestContext::new(context, token);

    let debug_str = format!("{:?}", auth_ctx);
    assert!(debug_str.contains("AuthenticatedRequestContext"));
}

// ============================================================================
// AuthResult Tests
// ============================================================================

#[test]
fn test_auth_result_expect_authenticated_success() {
    let context = create_test_context();
    let token = "test_token".to_string();
    let auth_ctx = AuthenticatedRequestContext::new(context, token);
    let auth_result = AuthResult::Authenticated(auth_ctx);

    let result = auth_result.expect_authenticated("should be authenticated");
    let val = result.expect("expected success");
    assert_eq!(val.token(), "test_token");
}

#[test]
fn test_auth_result_expect_authenticated_failure() {
    let context = create_test_context();
    let auth_result = AuthResult::Anonymous(context);

    let result = auth_result.expect_authenticated("authentication required");
    result.unwrap_err();
}

#[test]
fn test_auth_result_debug() {
    let context = create_test_context();
    let auth_result = AuthResult::Anonymous(context);

    let debug_str = format!("{:?}", auth_result);
    assert!(debug_str.contains("Anonymous"));
}

#[test]
fn test_auth_result_debug_authenticated() {
    let context = create_test_context();
    let token = "test_token".to_string();
    let auth_ctx = AuthenticatedRequestContext::new(context, token);
    let auth_result = AuthResult::Authenticated(auth_ctx);

    let debug_str = format!("{:?}", auth_result);
    assert!(debug_str.contains("Authenticated"));
}
