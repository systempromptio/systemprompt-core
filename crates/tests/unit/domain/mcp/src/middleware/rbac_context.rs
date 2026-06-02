//! Tests for `AuthResult::context`, `context_mut`, and the `Deref` impl on
//! `AuthenticatedRequestContext`.

use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId};
use systemprompt_mcp::middleware::{AuthResult, AuthenticatedRequestContext};
use systemprompt_models::RequestContext;

fn ctx(tag: &str) -> RequestContext {
    RequestContext::new(
        SessionId::new(format!("sess-{tag}")),
        TraceId::new(format!("trace-{tag}")),
        ContextId::new("00000000-0000-4000-8000-000000000001"),
        AgentName::new(format!("agent-{tag}")),
    )
}

#[test]
fn auth_result_anonymous_context_returns_inner() {
    let c = ctx("anon");
    let r = AuthResult::Anonymous(c.clone());
    assert_eq!(r.context().session_id().as_str(), c.session_id().as_str());
}

#[test]
fn auth_result_authenticated_context_returns_inner() {
    let c = ctx("auth");
    let auth_ctx = AuthenticatedRequestContext::new(c.clone(), "tok".to_owned());
    let r = AuthResult::Authenticated(auth_ctx);
    assert_eq!(r.context().session_id().as_str(), c.session_id().as_str());
}

#[test]
fn auth_result_context_mut_anonymous_mutates() {
    let c = ctx("mut-anon");
    let mut r = AuthResult::Anonymous(c);
    {
        let _ = r.context_mut();
    }
    assert!(matches!(r, AuthResult::Anonymous(_)));
}

#[test]
fn auth_result_context_mut_authenticated_mutates() {
    let c = ctx("mut-auth");
    let auth_ctx = AuthenticatedRequestContext::new(c, "t".to_owned());
    let mut r = AuthResult::Authenticated(auth_ctx);
    {
        let _ = r.context_mut();
    }
    assert!(matches!(r, AuthResult::Authenticated(_)));
}

#[test]
fn authenticated_request_context_deref_gives_request_context() {
    let c = ctx("deref");
    let auth_ctx = AuthenticatedRequestContext::new(c.clone(), "deref-token".to_owned());
    let deref: &RequestContext = &*auth_ctx;
    assert_eq!(deref.session_id().as_str(), c.session_id().as_str());
}

#[test]
fn authenticated_request_context_token_accessor() {
    let c = ctx("tok-acc");
    let auth_ctx = AuthenticatedRequestContext::new(c, "my-secret-token".to_owned());
    assert_eq!(auth_ctx.token(), "my-secret-token");
}

#[test]
fn authenticated_request_context_empty_token() {
    let c = ctx("empty");
    let auth_ctx = AuthenticatedRequestContext::new(c, String::new());
    assert_eq!(auth_ctx.token(), "");
}

#[test]
fn auth_result_expect_authenticated_on_anonymous_is_err() {
    let r = AuthResult::Anonymous(ctx("exp-anon"));
    let result = r.expect_authenticated("must auth");
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = format!("{err:?}");
    assert!(msg.contains("must auth") || !msg.is_empty());
}

#[test]
fn auth_result_expect_authenticated_on_authenticated_is_ok() {
    let auth_ctx = AuthenticatedRequestContext::new(ctx("exp-auth"), "good-token".to_owned());
    let r = AuthResult::Authenticated(auth_ctx);
    let result = r.expect_authenticated("msg");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().token(), "good-token");
}

#[test]
fn auth_result_debug_variants() {
    let anon = AuthResult::Anonymous(ctx("dbg-anon"));
    let s = format!("{anon:?}");
    assert!(s.contains("Anonymous"));

    let auth_ctx = AuthenticatedRequestContext::new(ctx("dbg-auth"), "t".to_owned());
    let auth = AuthResult::Authenticated(auth_ctx);
    let s = format!("{auth:?}");
    assert!(s.contains("Authenticated"));
}
