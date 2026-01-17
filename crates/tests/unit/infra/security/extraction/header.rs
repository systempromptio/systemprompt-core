use axum::http::HeaderMap;
use systemprompt_core_security::HeaderInjector;
use systemprompt_identifiers::{headers, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::auth::UserType;
use systemprompt_models::execution::context::RequestContext;

#[test]
fn test_header_session_id_constant() {
    assert_eq!(headers::SESSION_ID, "x-session-id");
}

#[test]
fn test_header_user_id_constant() {
    assert_eq!(headers::USER_ID, "x-user-id");
}

#[test]
fn test_header_trace_id_constant() {
    assert_eq!(headers::TRACE_ID, "x-trace-id");
}

#[test]
fn test_header_context_id_constant() {
    assert_eq!(headers::CONTEXT_ID, "x-context-id");
}

#[test]
fn test_header_agent_name_constant() {
    assert_eq!(headers::AGENT_NAME, "x-agent-name");
}

// ============================================================================
// Individual Header Injection Tests
// ============================================================================

#[test]
fn test_inject_session_id_success() {
    let mut headers = HeaderMap::new();
    let session_id = SessionId::new("session_123".to_string());

    let result = HeaderInjector::inject_session_id(&mut headers, &session_id);
    assert!(result.is_ok());
    assert_eq!(
        headers.get("x-session-id").unwrap().to_str().unwrap(),
        "session_123"
    );
}

#[test]
fn test_inject_user_id_success() {
    let mut headers = HeaderMap::new();
    let user_id = UserId::new("user_456".to_string());

    let result = HeaderInjector::inject_user_id(&mut headers, &user_id);
    assert!(result.is_ok());
    assert_eq!(
        headers.get("x-user-id").unwrap().to_str().unwrap(),
        "user_456"
    );
}

#[test]
fn test_inject_trace_id_success() {
    let mut headers = HeaderMap::new();
    let trace_id = TraceId::new("trace_789".to_string());

    let result = HeaderInjector::inject_trace_id(&mut headers, &trace_id);
    assert!(result.is_ok());
    assert_eq!(
        headers.get("x-trace-id").unwrap().to_str().unwrap(),
        "trace_789"
    );
}

#[test]
fn test_inject_context_id_success() {
    let mut headers = HeaderMap::new();
    let context_id = ContextId::new("context_abc".to_string());

    let result = HeaderInjector::inject_context_id(&mut headers, &context_id);
    assert!(result.is_ok());
    assert_eq!(
        headers.get("x-context-id").unwrap().to_str().unwrap(),
        "context_abc"
    );
}

#[test]
fn test_inject_context_id_empty_skips() {
    let mut headers = HeaderMap::new();
    let context_id = ContextId::new(String::new());

    let result = HeaderInjector::inject_context_id(&mut headers, &context_id);
    assert!(result.is_ok());
    assert!(headers.get("x-context-id").is_none());
}

#[test]
fn test_inject_agent_name_success() {
    let mut headers = HeaderMap::new();

    let result = HeaderInjector::inject_agent_name(&mut headers, "test-agent");
    assert!(result.is_ok());
    assert_eq!(
        headers.get("x-agent-name").unwrap().to_str().unwrap(),
        "test-agent"
    );
}

// ============================================================================
// Header Overwrite Tests
// ============================================================================

#[test]
fn test_inject_overwrites_existing_header() {
    let mut headers = HeaderMap::new();
    let session_id1 = SessionId::new("first_session".to_string());
    let session_id2 = SessionId::new("second_session".to_string());

    HeaderInjector::inject_session_id(&mut headers, &session_id1).unwrap();
    HeaderInjector::inject_session_id(&mut headers, &session_id2).unwrap();

    assert_eq!(
        headers.get("x-session-id").unwrap().to_str().unwrap(),
        "second_session"
    );
}

// ============================================================================
// RequestContext Injection Tests
// ============================================================================

#[test]
fn test_inject_from_request_context_success() {
    let mut headers = HeaderMap::new();

    let ctx = RequestContext::new(
        SessionId::new("ctx_session".to_string()),
        TraceId::new("ctx_trace".to_string()),
        ContextId::new("ctx_context".to_string()),
        AgentName::new("ctx_agent".to_string()),
    )
    .with_user_id(UserId::new("ctx_user".to_string()))
    .with_user_type(UserType::User);

    let result = HeaderInjector::inject_from_request_context(&mut headers, &ctx);
    assert!(result.is_ok());

    assert_eq!(
        headers.get("x-session-id").unwrap().to_str().unwrap(),
        "ctx_session"
    );
    assert_eq!(
        headers.get("x-user-id").unwrap().to_str().unwrap(),
        "ctx_user"
    );
    assert_eq!(
        headers.get("x-trace-id").unwrap().to_str().unwrap(),
        "ctx_trace"
    );
    assert_eq!(
        headers.get("x-context-id").unwrap().to_str().unwrap(),
        "ctx_context"
    );
    assert_eq!(
        headers.get("x-agent-name").unwrap().to_str().unwrap(),
        "ctx_agent"
    );
}

#[test]
fn test_inject_from_request_context_empty_context_id() {
    let mut headers = HeaderMap::new();

    let ctx = RequestContext::new(
        SessionId::new("session".to_string()),
        TraceId::new("trace".to_string()),
        ContextId::new(String::new()),
        AgentName::new("agent".to_string()),
    )
    .with_user_id(UserId::new("user".to_string()))
    .with_user_type(UserType::User);

    let result = HeaderInjector::inject_from_request_context(&mut headers, &ctx);
    assert!(result.is_ok());

    assert!(headers.get("x-session-id").is_some());
    assert!(headers.get("x-user-id").is_some());
    assert!(headers.get("x-trace-id").is_some());
    assert!(headers.get("x-context-id").is_none());
    assert!(headers.get("x-agent-name").is_some());
}

// ============================================================================
// Various ID Format Tests
// ============================================================================

#[test]
fn test_inject_uuid_format() {
    let mut headers = HeaderMap::new();
    let session_id = SessionId::new("550e8400-e29b-41d4-a716-446655440000".to_string());

    let result = HeaderInjector::inject_session_id(&mut headers, &session_id);
    assert!(result.is_ok());
    assert_eq!(
        headers.get("x-session-id").unwrap().to_str().unwrap(),
        "550e8400-e29b-41d4-a716-446655440000"
    );
}

#[test]
fn test_inject_alphanumeric_id() {
    let mut headers = HeaderMap::new();
    let user_id = UserId::new("user_abc123XYZ".to_string());

    let result = HeaderInjector::inject_user_id(&mut headers, &user_id);
    assert!(result.is_ok());
    assert_eq!(
        headers.get("x-user-id").unwrap().to_str().unwrap(),
        "user_abc123XYZ"
    );
}

#[test]
fn test_inject_underscore_id() {
    let mut headers = HeaderMap::new();
    let trace_id = TraceId::new("trace_with_underscores_123".to_string());

    let result = HeaderInjector::inject_trace_id(&mut headers, &trace_id);
    assert!(result.is_ok());
    assert_eq!(
        headers.get("x-trace-id").unwrap().to_str().unwrap(),
        "trace_with_underscores_123"
    );
}

#[test]
fn test_inject_hyphenated_id() {
    let mut headers = HeaderMap::new();
    let context_id = ContextId::new("context-with-hyphens-456".to_string());

    let result = HeaderInjector::inject_context_id(&mut headers, &context_id);
    assert!(result.is_ok());
    assert_eq!(
        headers.get("x-context-id").unwrap().to_str().unwrap(),
        "context-with-hyphens-456"
    );
}

// ============================================================================
// HeaderInjector Debug Tests
// ============================================================================

#[test]
fn test_header_injector_debug() {
    let injector = HeaderInjector;
    let debug_str = format!("{:?}", injector);
    assert!(debug_str.contains("HeaderInjector"));
}

#[test]
fn test_header_injector_clone() {
    let injector = HeaderInjector;
    let cloned = injector;
    let _ = format!("{:?}", cloned);
}

#[test]
fn test_header_injector_copy() {
    let injector = HeaderInjector;
    let copied: HeaderInjector = injector;
    let _ = format!("{:?}", copied);
}
