use systemprompt_identifiers::{
    AgentName, AiToolCallId, ClientId, ContextId, McpExecutionId, SessionId, TaskId, TraceId,
    UserId,
};
use systemprompt_models::auth::UserType;
use systemprompt_models::execution::{
    CallSource, ContextExtractionError, ContextIdSource, RequestContext,
    TASK_BASED_CONTEXT_MARKER,
};

fn test_context() -> RequestContext {
    RequestContext::new(
        SessionId::new("sess-1"),
        TraceId::new("trace-1"),
        ContextId::new("ctx-1"),
        AgentName::new("test-agent"),
    )
}

// ============================================================================
// RequestContext::new Tests
// ============================================================================

#[test]
fn request_context_new_sets_session_id() {
    let ctx = test_context();
    assert_eq!(ctx.session_id().as_str(), "sess-1");
}

#[test]
fn request_context_new_sets_trace_id() {
    let ctx = test_context();
    assert_eq!(ctx.trace_id().as_str(), "trace-1");
}

#[test]
fn request_context_new_sets_context_id() {
    let ctx = test_context();
    assert_eq!(ctx.context_id().as_str(), "ctx-1");
}

#[test]
fn request_context_new_sets_agent_name() {
    let ctx = test_context();
    assert_eq!(ctx.agent_name().as_str(), "test-agent");
}

#[test]
fn request_context_new_defaults_to_anonymous_user() {
    let ctx = test_context();
    assert!(ctx.user_id().is_anonymous());
}

#[test]
fn request_context_new_defaults_to_anon_user_type() {
    let ctx = test_context();
    assert_eq!(ctx.user_type(), UserType::Anon);
}

#[test]
fn request_context_new_has_no_task_id() {
    let ctx = test_context();
    assert!(ctx.task_id().is_none());
}

#[test]
fn request_context_new_has_no_client_id() {
    let ctx = test_context();
    assert!(ctx.client_id().is_none());
}

#[test]
fn request_context_new_has_no_ai_tool_call_id() {
    let ctx = test_context();
    assert!(ctx.ai_tool_call_id().is_none());
}

#[test]
fn request_context_new_has_no_mcp_execution_id() {
    let ctx = test_context();
    assert!(ctx.mcp_execution_id().is_none());
}

#[test]
fn request_context_new_has_no_call_source() {
    let ctx = test_context();
    assert!(ctx.call_source().is_none());
}

#[test]
fn request_context_new_is_not_authenticated() {
    let ctx = test_context();
    assert!(!ctx.is_authenticated());
}

// ============================================================================
// Builder Chain Tests
// ============================================================================

#[test]
fn request_context_with_user_id() {
    let ctx = test_context().with_user_id(UserId::new("user-42"));
    assert_eq!(ctx.user_id().as_str(), "user-42");
}

#[test]
fn request_context_with_agent_name() {
    let ctx = test_context().with_agent_name(AgentName::new("other-agent"));
    assert_eq!(ctx.agent_name().as_str(), "other-agent");
}

#[test]
fn request_context_with_context_id() {
    let ctx = test_context().with_context_id(ContextId::new("new-ctx"));
    assert_eq!(ctx.context_id().as_str(), "new-ctx");
}

#[test]
fn request_context_with_task_id() {
    let task_id = TaskId::new("task-1");
    let ctx = test_context().with_task_id(task_id);
    assert_eq!(ctx.task_id().unwrap().as_str(), "task-1");
}

#[test]
fn request_context_with_task_sets_both_task_and_source() {
    let ctx = test_context().with_task(
        TaskId::new("task-2"),
        CallSource::Agentic,
    );
    assert_eq!(ctx.task_id().unwrap().as_str(), "task-2");
    assert_eq!(ctx.call_source(), Some(CallSource::Agentic));
}

#[test]
fn request_context_with_ai_tool_call_id() {
    let ctx = test_context().with_ai_tool_call_id(AiToolCallId::from("ai-call-1".to_string()));
    assert!(ctx.ai_tool_call_id().is_some());
}

#[test]
fn request_context_with_mcp_execution_id() {
    let ctx = test_context()
        .with_mcp_execution_id(McpExecutionId::from("mcp-exec-1".to_string()));
    assert!(ctx.mcp_execution_id().is_some());
}

#[test]
fn request_context_with_client_id() {
    let ctx = test_context().with_client_id(ClientId::new("client-1"));
    assert_eq!(ctx.client_id().unwrap().as_str(), "client-1");
}

#[test]
fn request_context_with_user_type() {
    let ctx = test_context().with_user_type(UserType::Admin);
    assert_eq!(ctx.user_type(), UserType::Admin);
}

#[test]
fn request_context_with_auth_token() {
    let ctx = test_context().with_auth_token("my-jwt-token");
    assert_eq!(ctx.auth_token().as_str(), "my-jwt-token");
}

#[test]
fn request_context_with_call_source() {
    let ctx = test_context().with_call_source(CallSource::Direct);
    assert_eq!(ctx.call_source(), Some(CallSource::Direct));
}

#[test]
fn request_context_with_budget() {
    let ctx = test_context().with_budget(500);
    assert_eq!(ctx.settings.max_budget_cents, Some(500));
}

#[test]
fn request_context_with_tracked() {
    let ctx = test_context().with_tracked(false);
    assert!(!ctx.request.is_tracked);
}

#[test]
fn request_context_with_fingerprint_hash() {
    let ctx = test_context().with_fingerprint_hash("abc123");
    assert_eq!(ctx.fingerprint_hash(), Some("abc123"));
}

#[test]
fn request_context_builder_chain_multiple() {
    let ctx = test_context()
        .with_user_id(UserId::new("u1"))
        .with_user_type(UserType::User)
        .with_auth_token("token")
        .with_task_id(TaskId::new("t1"))
        .with_call_source(CallSource::Direct)
        .with_budget(100);

    assert_eq!(ctx.user_id().as_str(), "u1");
    assert_eq!(ctx.user_type(), UserType::User);
    assert_eq!(ctx.auth_token().as_str(), "token");
    assert_eq!(ctx.task_id().unwrap().as_str(), "t1");
    assert_eq!(ctx.call_source(), Some(CallSource::Direct));
    assert_eq!(ctx.settings.max_budget_cents, Some(100));
}

// ============================================================================
// Validation Tests
// ============================================================================

#[test]
fn request_context_validate_task_execution_fails_without_task_id() {
    let ctx = test_context();
    let result = ctx.validate_task_execution();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("task_id"));
}

#[test]
fn request_context_validate_task_execution_fails_with_empty_context_id() {
    let ctx = RequestContext::new(
        SessionId::new("sess-1"),
        TraceId::new("trace-1"),
        ContextId::new(""),
        AgentName::new("agent"),
    )
    .with_task_id(TaskId::new("task-1"));
    let result = ctx.validate_task_execution();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("context_id"));
}

#[test]
fn request_context_validate_task_execution_succeeds_when_valid() {
    let ctx = test_context().with_task_id(TaskId::new("task-1"));
    assert!(ctx.validate_task_execution().is_ok());
}

#[test]
fn request_context_validate_authenticated_fails_with_empty_token() {
    let ctx = test_context();
    let result = ctx.validate_authenticated();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("token"));
}

#[test]
fn request_context_validate_authenticated_fails_with_anonymous_user() {
    let ctx = test_context().with_auth_token("some-token");
    let result = ctx.validate_authenticated();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not authenticated"));
}

#[test]
fn request_context_validate_authenticated_succeeds_when_valid() {
    let ctx = test_context()
        .with_auth_token("valid-token")
        .with_user_id(UserId::new("real-user"));
    assert!(ctx.validate_authenticated().is_ok());
}

#[test]
fn request_context_elapsed_returns_duration() {
    let ctx = test_context();
    let elapsed = ctx.elapsed();
    assert!(elapsed.as_secs() < 2);
}

// ============================================================================
// ContextIdSource Tests
// ============================================================================

#[test]
fn context_id_source_direct() {
    let source = ContextIdSource::Direct("ctx-123".to_string());
    match source {
        ContextIdSource::Direct(id) => assert_eq!(id, "ctx-123"),
        _ => panic!("Expected Direct variant"),
    }
}

#[test]
fn context_id_source_from_task() {
    let source = ContextIdSource::FromTask {
        task_id: "task-abc".to_string(),
    };
    match source {
        ContextIdSource::FromTask { task_id } => assert_eq!(task_id, "task-abc"),
        _ => panic!("Expected FromTask variant"),
    }
}

// ============================================================================
// ContextExtractionError Tests
// ============================================================================

#[test]
fn context_extraction_error_missing_header_display() {
    let err = ContextExtractionError::MissingHeader("x-session-id".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("x-session-id"));
}

#[test]
fn context_extraction_error_missing_auth_header() {
    let err = ContextExtractionError::MissingAuthHeader;
    let msg = format!("{}", err);
    assert!(msg.contains("Authorization"));
}

#[test]
fn context_extraction_error_missing_context_id() {
    let err = ContextExtractionError::MissingContextId;
    let msg = format!("{}", err);
    assert!(msg.contains("context-id") || msg.contains("contextId"));
}

#[test]
fn context_extraction_error_invalid_header_value() {
    let err = ContextExtractionError::InvalidHeaderValue {
        header: "x-test".to_string(),
        reason: "bad encoding".to_string(),
    };
    let msg = format!("{}", err);
    assert!(msg.contains("x-test"));
    assert!(msg.contains("bad encoding"));
}

// ============================================================================
// TASK_BASED_CONTEXT_MARKER Tests
// ============================================================================

#[test]
fn task_based_context_marker_is_not_empty() {
    assert!(!TASK_BASED_CONTEXT_MARKER.is_empty());
}

#[test]
fn task_based_context_marker_value() {
    assert_eq!(TASK_BASED_CONTEXT_MARKER, "__task_based__");
}
