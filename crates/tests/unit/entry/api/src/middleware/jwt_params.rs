//! Unit tests for JWT middleware context assembly and common header
//! extraction.

use std::collections::BTreeMap;

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_api::services::middleware::jwt::test_api::{
    BuildContextParams, build_context, extract_common_headers,
};
use systemprompt_identifiers::{
    Actor, AgentName, ClientId, ContextId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_models::auth::{Permission, UserType};
use systemprompt_security::{JwtUserContext, TokenExtractor};

fn jwt_context(user_id: &UserId, session_id: &SessionId) -> JwtUserContext {
    JwtUserContext {
        user_id: user_id.clone(),
        session_id: session_id.clone(),
        role: Permission::User,
        user_type: UserType::User,
        client_id: None,
        act_chain: vec![Actor::user(UserId::new("delegator"))],
        attributes: BTreeMap::new(),
        jti: "jti-123".to_owned(),
        exp: 1_900_000_000,
    }
}

fn params(jwt: JwtUserContext) -> BuildContextParams {
    BuildContextParams {
        session_id: jwt.session_id.clone(),
        user_id: jwt.user_id.clone(),
        trace_id: TraceId::new("trace-1"),
        context_id: ContextId::generate(),
        agent_name: AgentName::new("agent-x"),
        task_id: None,
        auth_token: None,
        user_type: UserType::User,
        jwt_context: jwt,
    }
}

#[test]
fn build_context_carries_identity_and_claims() {
    let user_id = UserId::new("user-1");
    let session_id = SessionId::generate();
    let ctx = build_context(params(jwt_context(&user_id, &session_id)));

    assert_eq!(ctx.auth.actor, Actor::user(user_id));
    assert_eq!(ctx.auth.user_type, UserType::User);
    assert_eq!(ctx.auth.jti, "jti-123");
    assert_eq!(ctx.auth.token_exp, 1_900_000_000);
    assert_eq!(ctx.auth.act_chain.len(), 1);
    assert_eq!(ctx.request.session_id, session_id);
    assert_eq!(ctx.execution.trace_id.as_str(), "trace-1");
    assert_eq!(ctx.execution.agent_name.as_str(), "agent-x");
    assert!(ctx.execution.task_id.is_none());
    assert!(ctx.request.client_id.is_none());
    assert!(ctx.auth.auth_token.as_str().is_empty());
}

#[test]
fn build_context_applies_optional_fields() {
    let user_id = UserId::new("user-2");
    let session_id = SessionId::generate();
    let mut jwt = jwt_context(&user_id, &session_id);
    jwt.client_id = Some(ClientId::new("client-9"));
    let mut p = params(jwt);
    p.task_id = Some(TaskId::new("task-7"));
    p.auth_token = Some("token-abc".to_owned());

    let ctx = build_context(p);
    assert_eq!(
        ctx.request.client_id.as_ref().map(ClientId::as_str),
        Some("client-9")
    );
    assert_eq!(
        ctx.execution.task_id.as_ref().map(TaskId::as_str),
        Some("task-7")
    );
    assert_eq!(ctx.auth.auth_token.as_str(), "token-abc");
}

#[test]
fn extract_common_headers_reads_all_known_headers() {
    let mut headers = HeaderMap::new();
    headers.insert("x-trace-id", HeaderValue::from_static("trace-42"));
    headers.insert("x-task-id", HeaderValue::from_static("task-42"));
    headers.insert("x-agent-name", HeaderValue::from_static("agent-42"));
    headers.insert("authorization", HeaderValue::from_static("Bearer tok-42"));

    let (trace_id, task_id, token, agent_name) =
        extract_common_headers(&TokenExtractor::api_only(), &headers);
    assert_eq!(trace_id.as_str(), "trace-42");
    assert_eq!(task_id.as_ref().map(TaskId::as_str), Some("task-42"));
    assert_eq!(token.as_deref(), Some("tok-42"));
    assert_eq!(agent_name.as_str(), "agent-42");
}

#[test]
fn extract_common_headers_defaults_when_absent() {
    let headers = HeaderMap::new();
    let (trace_id, task_id, token, agent_name) =
        extract_common_headers(&TokenExtractor::api_only(), &headers);
    assert!(!trace_id.as_str().is_empty());
    assert!(task_id.is_none());
    assert!(token.is_none());
    assert_eq!(agent_name, AgentName::system());
}
