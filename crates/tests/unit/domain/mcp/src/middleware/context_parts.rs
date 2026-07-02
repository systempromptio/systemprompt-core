//! Tests for the parts-based bearer-token and request-context extraction
//! helpers.

use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_mcp::middleware::{bearer_token_from_parts, request_context_from_parts};
use systemprompt_models::RequestContext;
use systemprompt_traits::InjectContextHeaders;

fn parts_with_headers(headers: &[(&str, &str)]) -> http::request::Parts {
    let mut builder = http::Request::builder().uri("http://localhost/mcp");
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let (parts, ()) = builder.body(()).expect("request").into_parts();
    parts
}

fn ctx() -> RequestContext {
    RequestContext::new(
        SessionId::new("s-parts"),
        TraceId::new("t-parts"),
        ContextId::generate(),
        AgentName::new("agent-parts"),
    )
    .with_actor(Actor::user(UserId::new("user-parts")))
}

#[test]
fn bearer_token_extracted_from_authorization_header() {
    let parts = parts_with_headers(&[("authorization", "Bearer tok-123")]);
    assert_eq!(bearer_token_from_parts(&parts).as_deref(), Some("tok-123"));
}

#[test]
fn non_bearer_authorization_yields_none() {
    let parts = parts_with_headers(&[("authorization", "Basic dXNlcjpwdw==")]);
    assert!(bearer_token_from_parts(&parts).is_none());
}

#[test]
fn missing_authorization_yields_none() {
    let parts = parts_with_headers(&[]);
    assert!(bearer_token_from_parts(&parts).is_none());
}

#[test]
fn request_context_prefers_extension_over_headers() {
    let mut parts = parts_with_headers(&[]);
    parts.extensions.insert(ctx());

    let recovered = request_context_from_parts(&parts).expect("extension context");
    assert_eq!(recovered.session_id().as_str(), "s-parts");
    assert_eq!(recovered.trace_id().as_str(), "t-parts");
}

#[test]
fn request_context_reconstructed_from_headers() {
    let context = ctx();
    let mut header_map = http::HeaderMap::new();
    context.inject_headers(&mut header_map);

    let mut parts = parts_with_headers(&[]);
    parts.headers = header_map;

    let recovered = request_context_from_parts(&parts).expect("header context");
    assert_eq!(recovered.session_id().as_str(), "s-parts");
}

#[test]
fn missing_context_headers_is_an_error() {
    let parts = parts_with_headers(&[]);
    assert!(request_context_from_parts(&parts).is_err());
}
