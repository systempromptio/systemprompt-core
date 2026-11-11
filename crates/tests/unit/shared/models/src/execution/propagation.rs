use http::HeaderMap;
use systemprompt_identifiers::{
    Actor, AgentName, AiToolCallId, ClientId, ContextId, JwtToken, SessionId, TaskId, TraceId,
    UserId, headers,
};
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_models::execution::{CallSource, RequestContext};
use systemprompt_traits::{ContextPropagation, ContextPropagationError, InjectContextHeaders};

const FIXED_CONTEXT: &str = "3f2504e0-4f89-41d3-9a0c-0305e82c3301";

fn base_context() -> RequestContext {
    RequestContext::new(
        SessionId::new("sess-1"),
        TraceId::new("trace-1"),
        ContextId::try_new(FIXED_CONTEXT).expect("valid ContextId"),
        AgentName::try_new("agent-one").expect("valid AgentName"),
        Actor::user(UserId::new("00000000-0000-4000-8000-000000000001")),
    )
}

#[test]
fn round_trip_preserves_correlation_and_execution_fields() {
    let ctx = base_context()
        .with_task_id(TaskId::new("task-9"))
        .with_ai_tool_call_id(AiToolCallId::new("call-3"))
        .with_call_source(CallSource::Agentic)
        .with_auth_token(JwtToken::new("tok123"));

    let restored = RequestContext::from_headers(&ctx.to_headers()).unwrap();

    assert_eq!(restored.session_id().as_str(), "sess-1");
    assert_eq!(restored.trace_id().as_str(), "trace-1");
    assert_eq!(restored.context_id().as_str(), FIXED_CONTEXT);
    assert_eq!(restored.agent_name().as_str(), "agent-one");
    assert_eq!(restored.task_id().map(TaskId::as_str), Some("task-9"));
    assert_eq!(
        restored.ai_tool_call_id().map(AsRef::as_ref),
        Some("call-3")
    );
    assert_eq!(restored.call_source(), Some(CallSource::Agentic));
    assert_eq!(restored.auth_token().map(JwtToken::as_str), Some("tok123"));
}

#[test]
fn missing_required_header_is_an_error() {
    let mut hdrs = base_context().to_headers();
    hdrs.remove(headers::TRACE_ID);
    let err = RequestContext::from_headers(&hdrs).unwrap_err();
    assert!(matches!(
        err,
        ContextPropagationError::MissingHeader(name) if name == headers::TRACE_ID
    ));
}

#[test]
fn a_context_without_a_bearer_injects_no_authorization_header() {
    let ctx = base_context();
    assert!(ctx.auth_token().is_none());
    let mut hdrs = HeaderMap::new();
    ctx.inject_headers(&mut hdrs);
    assert!(!hdrs.contains_key(headers::AUTHORIZATION));
}

#[test]
fn an_empty_bearer_is_not_restored_as_a_token() {
    let mut hdrs = base_context().to_headers();
    hdrs.insert(headers::AUTHORIZATION, "Bearer ".parse().unwrap());
    let restored = RequestContext::from_headers(&hdrs).unwrap();
    assert!(restored.auth_token().is_none());
}

#[test]
fn authorization_header_carries_bearer_prefix() {
    let hdrs = base_context()
        .with_auth_token(JwtToken::new("abc"))
        .to_headers();
    assert_eq!(
        hdrs.get(headers::AUTHORIZATION).unwrap().to_str().unwrap(),
        "Bearer abc"
    );
}

#[test]
fn non_bearer_authorization_is_ignored_inbound() {
    let mut hdrs = base_context().to_headers();
    hdrs.insert(headers::AUTHORIZATION, "Basic xyz".parse().unwrap());
    let restored = RequestContext::from_headers(&hdrs).unwrap();
    assert!(restored.auth_token().is_none());
}

#[test]
fn blank_context_id_header_mints_a_fresh_context() {
    let mut hdrs = base_context().to_headers();
    hdrs.insert(headers::CONTEXT_ID, "".parse().unwrap());
    let restored = RequestContext::from_headers(&hdrs).unwrap();
    assert!(!restored.context_id().as_str().is_empty());
    assert_ne!(restored.context_id().as_str(), FIXED_CONTEXT);
}

#[test]
fn malformed_context_id_header_is_rejected_not_rederived() {
    let mut hdrs = base_context().to_headers();
    hdrs.insert(headers::CONTEXT_ID, "not-a-uuid".parse().unwrap());
    let err = RequestContext::from_headers(&hdrs).unwrap_err();
    assert!(matches!(
        err,
        ContextPropagationError::InvalidHeader { name, .. } if name == headers::CONTEXT_ID
    ));
}

#[test]
fn absent_context_id_header_derives_from_the_session() {
    let mut hdrs = base_context().to_headers();
    hdrs.remove(headers::CONTEXT_ID);
    let restored = RequestContext::from_headers(&hdrs).unwrap();
    assert_eq!(
        restored.context_id(),
        &ContextId::derived_from_session(&SessionId::new("sess-1"))
    );
}

#[test]
fn unknown_call_source_header_is_rejected() {
    let mut hdrs = base_context().to_headers();
    hdrs.insert(headers::CALL_SOURCE, "bogus".parse().unwrap());
    let err = RequestContext::from_headers(&hdrs).unwrap_err();
    assert!(matches!(
        err,
        ContextPropagationError::InvalidHeader { name, .. } if name == headers::CALL_SOURCE
    ));
}

#[test]
fn invalid_header_value_is_skipped_not_panicked() {
    let ctx = RequestContext::new(
        SessionId::new("sess\nbad"),
        TraceId::new("trace-1"),
        ContextId::try_new(FIXED_CONTEXT).expect("valid ContextId"),
        AgentName::try_new("agent-one").expect("valid AgentName"),
        Actor::user(UserId::new("00000000-0000-4000-8000-000000000001")),
    );
    let hdrs = ctx.to_headers();
    assert!(!hdrs.contains_key(headers::SESSION_ID));
    assert!(hdrs.contains_key(headers::TRACE_ID));
}

#[test]
fn reserved_agent_name_header_is_an_invalid_header_error() {
    let mut hdrs = base_context().to_headers();
    hdrs.insert(
        headers::AGENT_NAME,
        http::HeaderValue::from_static("unknown"),
    );
    let err = RequestContext::from_headers(&hdrs).unwrap_err();
    assert!(
        matches!(err, ContextPropagationError::InvalidHeader { ref name, .. } if name == headers::AGENT_NAME),
        "got {err:?}"
    );
}

#[test]
fn identity_never_travels_in_outbound_headers() {
    let user = AuthenticatedUser::new_with_roles(
        UserId::generate(),
        "u".to_owned(),
        "u@example.com".to_owned(),
        vec![Permission::Admin],
        vec!["admin".to_owned()],
    );
    let hdrs = base_context()
        .with_user(user)
        .with_client_id(ClientId::new("client-7"))
        .to_headers();
    for name in [
        "x-proxy-verified",
        "x-user-permissions",
        "x-user-roles",
        headers::USER_ID,
        headers::USER_TYPE,
        headers::CLIENT_ID,
    ] {
        assert!(!hdrs.contains_key(name), "{name} must not be emitted");
    }
}

#[test]
fn inbound_identity_headers_never_yield_an_identity() {
    let mut hdrs = base_context().to_headers();
    hdrs.insert("x-proxy-verified", "true".parse().unwrap());
    hdrs.insert(headers::USER_ID, "forged-admin".parse().unwrap());
    hdrs.insert("x-user-permissions", "admin".parse().unwrap());
    hdrs.insert("x-user-roles", "admin".parse().unwrap());
    hdrs.insert(headers::USER_TYPE, "admin".parse().unwrap());
    hdrs.insert(headers::CLIENT_ID, "forged-client".parse().unwrap());

    let restored = RequestContext::from_headers(&hdrs).unwrap();

    assert!(restored.user.is_none());
    assert!(!restored.is_authenticated());
    assert_ne!(restored.user_id().as_str(), "forged-admin");
    assert!(restored.client_id().is_none());
    assert!(matches!(
        restored.actor().kind,
        systemprompt_identifiers::ActorKind::Anonymous
    ));
}
