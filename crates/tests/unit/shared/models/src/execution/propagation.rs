use http::HeaderMap;
use systemprompt_identifiers::{
    AgentName, AiToolCallId, ClientId, ContextId, SessionId, TaskId, TraceId, headers,
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
    )
}

#[test]
fn round_trip_preserves_identity_and_execution_fields() {
    let ctx = base_context()
        .with_task_id(TaskId::new("task-9"))
        .with_ai_tool_call_id(AiToolCallId::new("call-3"))
        .with_call_source(CallSource::Agentic)
        .with_client_id(ClientId::new("client-7"))
        .with_auth_token("tok123");

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
    assert_eq!(restored.client_id().map(ClientId::as_str), Some("client-7"));
    assert_eq!(restored.auth_token().as_str(), "tok123");
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
fn empty_auth_token_omits_authorization_header() {
    let mut hdrs = HeaderMap::new();
    base_context().inject_headers(&mut hdrs);
    assert!(!hdrs.contains_key(headers::AUTHORIZATION));
    assert!(!hdrs.contains_key(headers::PROXY_VERIFIED));
}

#[test]
fn authorization_header_carries_bearer_prefix() {
    let hdrs = base_context().with_auth_token("abc").to_headers();
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
    assert!(restored.auth_token().as_str().is_empty());
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
fn proxy_verified_user_round_trips_permissions() {
    let user = AuthenticatedUser::new_with_roles(
        uuid::Uuid::new_v4(),
        "u".to_owned(),
        "u@example.com".to_owned(),
        vec![Permission::Admin, Permission::Mcp],
        vec!["admin".to_owned(), "analyst".to_owned()],
    );
    let user_id = user.id;
    let ctx = base_context().with_user(user);

    let hdrs = ctx.to_headers();
    assert_eq!(
        hdrs.get(headers::PROXY_VERIFIED).unwrap().to_str().unwrap(),
        "true"
    );
    assert_eq!(
        hdrs.get(headers::USER_PERMISSIONS)
            .unwrap()
            .to_str()
            .unwrap(),
        "admin mcp"
    );
    assert_eq!(
        hdrs.get(headers::USER_ROLES).unwrap().to_str().unwrap(),
        "admin analyst"
    );

    let restored = RequestContext::from_headers(&hdrs).unwrap();
    assert!(restored.is_authenticated());
    let restored_user = restored.user.expect("proxy-verified user reconstructed");
    assert_eq!(restored_user.id, user_id);
    assert_eq!(
        restored_user.permissions,
        vec![Permission::Admin, Permission::Mcp]
    );
    assert_eq!(restored_user.roles, vec!["admin", "analyst"]);
}

#[test]
fn proxy_verified_without_a_roles_header_carries_no_roles() {
    let user = AuthenticatedUser::new_with_roles(
        uuid::Uuid::new_v4(),
        "u".to_owned(),
        "u@example.com".to_owned(),
        vec![Permission::Mcp],
        vec!["admin".to_owned()],
    );
    let mut hdrs = base_context().with_user(user).to_headers();
    hdrs.remove(headers::USER_ROLES);
    let restored = RequestContext::from_headers(&hdrs).unwrap();
    let restored_user = restored.user.expect("proxy-verified user reconstructed");
    assert!(restored_user.roles.is_empty());
}

#[test]
fn proxy_verified_with_invalid_user_uuid_is_an_error() {
    let user = AuthenticatedUser::new(
        uuid::Uuid::new_v4(),
        "u".to_owned(),
        "u@example.com".to_owned(),
        vec![Permission::User],
    );
    let mut hdrs = base_context().with_user(user).to_headers();
    hdrs.insert(headers::USER_ID, "not-a-uuid".parse().unwrap());
    let err = RequestContext::from_headers(&hdrs).unwrap_err();
    assert!(matches!(
        err,
        ContextPropagationError::InvalidHeader { name, .. } if name == headers::USER_ID
    ));
}

#[test]
fn proxy_verified_with_malformed_permissions_is_rejected_not_downgraded() {
    let mut hdrs = base_context().to_headers();
    hdrs.insert(headers::PROXY_VERIFIED, "true".parse().unwrap());
    hdrs.insert(headers::USER_PERMISSIONS, "warp-drive".parse().unwrap());
    let err = RequestContext::from_headers(&hdrs).unwrap_err();
    assert!(matches!(
        err,
        ContextPropagationError::InvalidHeader { name, .. } if name == headers::USER_PERMISSIONS
    ));
}

#[test]
fn proxy_verified_without_a_permissions_header_is_anonymous() {
    let mut hdrs = base_context().to_headers();
    hdrs.insert(headers::PROXY_VERIFIED, "true".parse().unwrap());
    let restored = RequestContext::from_headers(&hdrs).unwrap();
    assert!(restored.user.is_none());
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
fn proxy_verified_header_must_be_exactly_true() {
    let user = AuthenticatedUser::new(
        uuid::Uuid::new_v4(),
        "u".to_owned(),
        "u@example.com".to_owned(),
        vec![Permission::User],
    );
    let mut hdrs = base_context().with_user(user).to_headers();
    hdrs.insert(headers::PROXY_VERIFIED, "TRUE".parse().unwrap());
    let restored = RequestContext::from_headers(&hdrs).unwrap();
    assert!(restored.user.is_none());
}

#[test]
fn invalid_header_value_is_skipped_not_panicked() {
    let ctx = RequestContext::new(
        SessionId::new("sess\nbad"),
        TraceId::new("trace-1"),
        ContextId::try_new(FIXED_CONTEXT).expect("valid ContextId"),
        AgentName::try_new("agent-one").expect("valid AgentName"),
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
