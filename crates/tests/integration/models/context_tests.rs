use systemprompt_identifiers::{AgentName, ClientId, ContextId, SessionId, TaskId, TraceId, UserId};
use systemprompt_models::auth::UserType;
use systemprompt_models::execution::{CallSource, RequestContext};
use systemprompt_traits::ContextPropagation;

#[test]
fn test_round_trip_basic() {
    let ctx = RequestContext::new(
        SessionId::new("sess_123".to_string()),
        TraceId::new("trace_456".to_string()),
        ContextId::new("ctx_789".to_string()),
        AgentName::new("test-agent".to_string()),
    )
    .with_user_id(UserId::new("user_123".to_string()));

    let headers = ctx.to_headers();
    let ctx2 = RequestContext::from_headers(&headers).unwrap();

    assert_eq!(
        ctx.request.session_id.as_str(),
        ctx2.request.session_id.as_str()
    );
    assert_eq!(
        ctx.execution.trace_id.as_str(),
        ctx2.execution.trace_id.as_str()
    );
    assert_eq!(ctx.auth.user_id.as_str(), ctx2.auth.user_id.as_str());
    assert_eq!(
        ctx.execution.context_id.as_str(),
        ctx2.execution.context_id.as_str()
    );
    assert_eq!(
        ctx.execution.agent_name.as_str(),
        ctx2.execution.agent_name.as_str()
    );
}

#[test]
fn test_round_trip_with_optional_fields() {
    let ctx = RequestContext::new(
        SessionId::new("sess_123".to_string()),
        TraceId::new("trace_456".to_string()),
        ContextId::new("ctx_789".to_string()),
        AgentName::new("test-agent".to_string()),
    )
    .with_user_id(UserId::new("user_123".to_string()))
    .with_task_id(TaskId::new("task_456".to_string()))
    .with_client_id(ClientId::new("client_789".to_string()))
    .with_call_source(CallSource::Direct);

    let headers = ctx.to_headers();
    let ctx2 = RequestContext::from_headers(&headers).unwrap();

    assert_eq!(
        ctx.execution.task_id.as_ref().map(|t| t.as_str()),
        ctx2.execution.task_id.as_ref().map(|t| t.as_str())
    );
    assert_eq!(
        ctx.request.client_id.as_ref().map(|c| c.as_str()),
        ctx2.request.client_id.as_ref().map(|c| c.as_str())
    );
    assert_eq!(ctx.execution.call_source, ctx2.execution.call_source);
}

#[test]
fn test_inject_headers_includes_all_fields() {
    use systemprompt_traits::InjectContextHeaders;
    use axum::http::HeaderMap;

    let ctx = RequestContext::new(
        SessionId::new("sess_123".to_string()),
        TraceId::new("trace_456".to_string()),
        ContextId::new("ctx_789".to_string()),
        AgentName::new("test-agent".to_string()),
    )
    .with_user_id(UserId::new("user_123".to_string()))
    .with_task_id(TaskId::new("task_456".to_string()));

    let mut headers = HeaderMap::new();
    ctx.inject_headers(&mut headers);

    assert!(headers.contains_key("x-session-id"));
    assert!(headers.contains_key("x-trace-id"));
    assert!(headers.contains_key("x-user-id"));
    assert!(headers.contains_key("x-context-id"));
    assert!(headers.contains_key("x-agent-name"));
    assert!(headers.contains_key("x-task-id"));
}

#[test]
fn test_empty_context_id_not_injected() {
    use systemprompt_traits::InjectContextHeaders;
    use axum::http::HeaderMap;

    let ctx = RequestContext::new(
        SessionId::new("sess_123".to_string()),
        TraceId::new("trace_456".to_string()),
        ContextId::new(String::new()),
        AgentName::new("test-agent".to_string()),
    );

    let mut headers = HeaderMap::new();
    ctx.inject_headers(&mut headers);

    assert!(!headers.contains_key("x-context-id"));
}

#[test]
fn test_accessor_methods() {
    let ctx = RequestContext::new(
        SessionId::new("sess_123".to_string()),
        TraceId::new("trace_456".to_string()),
        ContextId::new("ctx_789".to_string()),
        AgentName::new("test-agent".to_string()),
    )
    .with_user_id(UserId::new("user_123".to_string()));

    assert_eq!(ctx.session_id().as_str(), "sess_123");
    assert_eq!(ctx.trace_id().as_str(), "trace_456");
    assert_eq!(ctx.user_id().as_str(), "user_123");
    assert_eq!(ctx.context_id().as_str(), "ctx_789");
    assert_eq!(ctx.agent_name().as_str(), "test-agent");
}

#[test]
fn test_validation_methods() {
    let ctx_without_task = RequestContext::new(
        SessionId::new("sess_123".to_string()),
        TraceId::new("trace_456".to_string()),
        ContextId::new("ctx_789".to_string()),
        AgentName::new("test-agent".to_string()),
    );

    assert!(ctx_without_task.validate_task_execution().is_err());

    let ctx_with_task = ctx_without_task
        .clone()
        .with_task_id(TaskId::new("task_123".to_string()));

    assert!(ctx_with_task.validate_task_execution().is_ok());
}

#[test]
fn test_component_separation() {
    let ctx = RequestContext::new(
        SessionId::new("sess_123".to_string()),
        TraceId::new("trace_456".to_string()),
        ContextId::new("ctx_789".to_string()),
        AgentName::new("test-agent".to_string()),
    )
    .with_user_id(UserId::new("user_123".to_string()))
    .with_user_type(UserType::User)
    .with_client_id(ClientId::new("client_123".to_string()))
    .with_task_id(TaskId::new("task_456".to_string()));

    assert_eq!(ctx.auth.user_id.as_str(), "user_123");
    assert_eq!(ctx.auth.user_type, UserType::User);
    assert_eq!(ctx.request.session_id.as_str(), "sess_123");
    assert_eq!(
        ctx.request.client_id.as_ref().unwrap().as_str(),
        "client_123"
    );
    assert_eq!(ctx.execution.trace_id.as_str(), "trace_456");
    assert_eq!(ctx.execution.context_id.as_str(), "ctx_789");
    assert_eq!(ctx.execution.agent_name.as_str(), "test-agent");
    assert_eq!(ctx.execution.task_id.as_ref().unwrap().as_str(), "task_456");
}
