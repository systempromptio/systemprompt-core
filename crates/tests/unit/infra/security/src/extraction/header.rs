use axum::http::{HeaderMap, HeaderValue};
use systemprompt_identifiers::{headers, AgentName, ContextId, SessionId, TaskId, TraceId, UserId};
use systemprompt_models::auth::UserType;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_security::{HeaderExtractor, HeaderInjectionError, HeaderInjector};

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

// ============================================================================
// HeaderExtractor Tests
// ============================================================================

#[test]
fn test_header_extractor_extract_trace_id_present() {
    let mut headers = HeaderMap::new();
    headers.insert("x-trace-id", HeaderValue::from_static("my-trace-id-123"));

    let trace_id = HeaderExtractor::extract_trace_id(&headers);
    assert_eq!(trace_id.as_str(), "my-trace-id-123");
}

#[test]
fn test_header_extractor_extract_trace_id_missing() {
    let headers = HeaderMap::new();

    let trace_id = HeaderExtractor::extract_trace_id(&headers);
    assert!(!trace_id.as_str().is_empty(), "Should generate a trace ID when missing");
}

#[test]
fn test_header_extractor_extract_trace_id_generates_unique() {
    let headers = HeaderMap::new();

    let trace_id1 = HeaderExtractor::extract_trace_id(&headers);
    let trace_id2 = HeaderExtractor::extract_trace_id(&headers);

    assert_ne!(
        trace_id1.as_str(),
        trace_id2.as_str(),
        "Generated trace IDs should be unique"
    );
}

#[test]
fn test_header_extractor_extract_context_id_present() {
    let mut headers = HeaderMap::new();
    headers.insert("x-context-id", HeaderValue::from_static("my-context-id"));

    let context_id = HeaderExtractor::extract_context_id(&headers);
    assert_eq!(context_id.as_str(), "my-context-id");
}

#[test]
fn test_header_extractor_extract_context_id_missing() {
    let headers = HeaderMap::new();

    let context_id = HeaderExtractor::extract_context_id(&headers);
    assert!(context_id.as_str().is_empty(), "Should return empty context ID when missing");
}

#[test]
fn test_header_extractor_extract_context_id_empty_value() {
    let mut headers = HeaderMap::new();
    headers.insert("x-context-id", HeaderValue::from_static(""));

    let context_id = HeaderExtractor::extract_context_id(&headers);
    assert!(context_id.as_str().is_empty(), "Should return empty for empty header value");
}

#[test]
fn test_header_extractor_extract_task_id_present() {
    let mut headers = HeaderMap::new();
    headers.insert("x-task-id", HeaderValue::from_static("task-12345"));

    let task_id = HeaderExtractor::extract_task_id(&headers);
    assert!(task_id.is_some());
    assert_eq!(task_id.unwrap().as_str(), "task-12345");
}

#[test]
fn test_header_extractor_extract_task_id_missing() {
    let headers = HeaderMap::new();

    let task_id = HeaderExtractor::extract_task_id(&headers);
    assert!(task_id.is_none());
}

#[test]
fn test_header_extractor_extract_agent_name_present() {
    let mut headers = HeaderMap::new();
    headers.insert("x-agent-name", HeaderValue::from_static("my-custom-agent"));

    let agent_name = HeaderExtractor::extract_agent_name(&headers);
    assert_eq!(agent_name.as_str(), "my-custom-agent");
}

#[test]
fn test_header_extractor_extract_agent_name_missing() {
    let headers = HeaderMap::new();

    let agent_name = HeaderExtractor::extract_agent_name(&headers);
    assert_eq!(agent_name.as_str(), "system", "Should return 'system' when missing");
}

#[test]
fn test_header_extractor_with_all_headers() {
    let mut headers = HeaderMap::new();
    headers.insert("x-trace-id", HeaderValue::from_static("trace-abc"));
    headers.insert("x-context-id", HeaderValue::from_static("context-def"));
    headers.insert("x-task-id", HeaderValue::from_static("task-ghi"));
    headers.insert("x-agent-name", HeaderValue::from_static("agent-jkl"));

    let trace_id = HeaderExtractor::extract_trace_id(&headers);
    let context_id = HeaderExtractor::extract_context_id(&headers);
    let task_id = HeaderExtractor::extract_task_id(&headers);
    let agent_name = HeaderExtractor::extract_agent_name(&headers);

    assert_eq!(trace_id.as_str(), "trace-abc");
    assert_eq!(context_id.as_str(), "context-def");
    assert_eq!(task_id.unwrap().as_str(), "task-ghi");
    assert_eq!(agent_name.as_str(), "agent-jkl");
}

#[test]
fn test_header_extractor_uuid_values() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-trace-id",
        HeaderValue::from_static("550e8400-e29b-41d4-a716-446655440000"),
    );
    headers.insert(
        "x-context-id",
        HeaderValue::from_static("6ba7b810-9dad-11d1-80b4-00c04fd430c8"),
    );

    let trace_id = HeaderExtractor::extract_trace_id(&headers);
    let context_id = HeaderExtractor::extract_context_id(&headers);

    assert_eq!(trace_id.as_str(), "550e8400-e29b-41d4-a716-446655440000");
    assert_eq!(context_id.as_str(), "6ba7b810-9dad-11d1-80b4-00c04fd430c8");
}

#[test]
fn test_header_extractor_special_characters() {
    let mut headers = HeaderMap::new();
    headers.insert("x-agent-name", HeaderValue::from_static("agent_with-special.chars"));

    let agent_name = HeaderExtractor::extract_agent_name(&headers);
    assert_eq!(agent_name.as_str(), "agent_with-special.chars");
}

#[test]
fn test_header_extractor_debug() {
    let extractor = HeaderExtractor;
    let debug_str = format!("{:?}", extractor);
    assert!(debug_str.contains("HeaderExtractor"));
}

#[test]
fn test_header_extractor_clone() {
    let extractor = HeaderExtractor;
    let cloned = extractor;
    let _ = format!("{:?}", cloned);
}

#[test]
fn test_header_extractor_copy() {
    let extractor = HeaderExtractor;
    let copied: HeaderExtractor = extractor;
    let _ = format!("{:?}", copied);
}

// ============================================================================
// HeaderInjectionError Tests
// ============================================================================

#[test]
fn test_header_injection_error_display() {
    let error = HeaderInjectionError;
    assert_eq!(
        format!("{}", error),
        "Header value contains invalid characters"
    );
}

#[test]
fn test_header_injection_error_debug() {
    let error = HeaderInjectionError;
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("HeaderInjectionError"));
}

#[test]
fn test_header_injection_error_is_std_error() {
    let error: Box<dyn std::error::Error> = Box::new(HeaderInjectionError);
    assert!(error.to_string().contains("invalid characters"));
}

#[test]
fn test_header_injection_error_clone() {
    let error = HeaderInjectionError;
    let cloned = error;
    assert_eq!(format!("{}", error), format!("{}", cloned));
}

#[test]
fn test_header_injection_error_copy() {
    let error = HeaderInjectionError;
    let copied: HeaderInjectionError = error;
    assert_eq!(format!("{}", error), format!("{}", copied));
}

#[test]
fn test_header_injection_error_equality() {
    let error1 = HeaderInjectionError;
    let error2 = HeaderInjectionError;
    assert_eq!(error1, error2);
}

// ============================================================================
// Additional HeaderInjector Tests for task_id
// ============================================================================

#[test]
fn test_inject_task_id_success() {
    let mut headers = HeaderMap::new();
    let task_id = TaskId::new("task_123".to_string());

    let result = HeaderInjector::inject_task_id(&mut headers, &task_id);
    assert!(result.is_ok());
    assert_eq!(
        headers.get("x-task-id").unwrap().to_str().unwrap(),
        "task_123"
    );
}

#[test]
fn test_inject_task_id_uuid_format() {
    let mut headers = HeaderMap::new();
    let task_id = TaskId::new("550e8400-e29b-41d4-a716-446655440000".to_string());

    let result = HeaderInjector::inject_task_id(&mut headers, &task_id);
    assert!(result.is_ok());
    assert_eq!(
        headers.get("x-task-id").unwrap().to_str().unwrap(),
        "550e8400-e29b-41d4-a716-446655440000"
    );
}
