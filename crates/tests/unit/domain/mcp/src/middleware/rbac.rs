//! Unit tests for RBAC middleware types

use systemprompt_identifiers::{Actor, AgentName, ContextId, JwtToken, SessionId, TraceId, UserId};
use systemprompt_mcp::middleware::{AuthResult, AuthenticatedRequestContext};
use systemprompt_models::RequestContext;

const TEST_CONTEXT_ID_A: &str = "00000000-0000-4000-8000-000000000001";

fn create_test_context() -> RequestContext {
    RequestContext::new(
        SessionId::new("test_session".to_string()),
        TraceId::new("test_trace".to_string()),
        ContextId::try_new(TEST_CONTEXT_ID_A).expect("valid ContextId"),
        AgentName::try_new("test_agent".to_string()).expect("valid AgentName"),
        Actor::user(UserId::new("00000000-0000-4000-8000-000000000001")),
    )
}

#[test]
fn test_authenticated_request_context_new() {
    let context = create_test_context();
    let token = JwtToken::new("test_token");
    let auth_ctx = AuthenticatedRequestContext::new(context, token.clone());

    assert_eq!(
        auth_ctx.auth_token().map(JwtToken::as_str),
        Some("test_token")
    );
}

#[test]
fn test_authenticated_request_context_token() {
    let context = create_test_context();
    let token = JwtToken::new("bearer_abc123");
    let auth_ctx = AuthenticatedRequestContext::new(context, token);

    assert_eq!(
        auth_ctx.auth_token().map(JwtToken::as_str),
        Some("bearer_abc123")
    );
}

#[test]
fn test_auth_result_expect_authenticated_success() {
    let context = create_test_context();
    let token = JwtToken::new("test_token");
    let auth_ctx = AuthenticatedRequestContext::new(context, token);
    let auth_result = AuthResult::Authenticated(auth_ctx);

    let result = auth_result.expect_authenticated("should be authenticated");
    let val = result.expect("expected success");
    assert_eq!(val.auth_token().map(JwtToken::as_str), Some("test_token"));
}

#[test]
fn test_auth_result_expect_authenticated_failure() {
    let context = create_test_context();
    let auth_result = AuthResult::Anonymous(context);

    let result = auth_result.expect_authenticated("authentication required");
    result.unwrap_err();
}
