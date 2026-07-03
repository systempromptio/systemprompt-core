//! Unit-level coverage for the pure helpers re-exported from
//! `routes::gateway::messages`: `extract_credential` (Authorization /
//! x-api-key header parsing) and `build_gateway_authz_request` (JWT-claims →
//! authz request envelope). Both are `pub` precisely so this contract can be
//! locked in without standing up the full dispatch stack.

use std::collections::BTreeMap;

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_api::routes::gateway::messages::{
    GatewayAuthzRequestInput, build_gateway_authz_request, extract_credential,
};
use systemprompt_identifiers::{Actor, ContextId, ModelId, RouteId, TraceId, UserId};
use systemprompt_security::authz::EntityRef;

fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
    use axum::http::HeaderName;
    let mut h = HeaderMap::new();
    for (k, v) in pairs {
        let name = HeaderName::from_bytes(k.as_bytes()).expect("header name");
        h.insert(name, HeaderValue::from_str(v).expect("header value"));
    }
    h
}

#[test]
fn extract_credential_strips_bearer_prefix() {
    let h = headers(&[("authorization", "Bearer abc.def.ghi")]);
    assert_eq!(extract_credential(&h).as_deref(), Some("abc.def.ghi"));
}

#[test]
fn extract_credential_prefers_authorization_over_api_key() {
    let h = headers(&[
        ("x-api-key", "sk-test-123"),
        ("authorization", "Bearer preferred"),
    ]);
    assert_eq!(extract_credential(&h).as_deref(), Some("preferred"));
}

#[test]
fn extract_credential_falls_back_to_api_key() {
    let h = headers(&[("x-api-key", "sk-test-123")]);
    assert_eq!(extract_credential(&h).as_deref(), Some("sk-test-123"));
}

#[test]
fn extract_credential_returns_none_for_empty_bearer() {
    let h = headers(&[("authorization", "Bearer ")]);
    assert!(extract_credential(&h).is_none());
}

#[test]
fn extract_credential_returns_none_when_no_header() {
    let h = HeaderMap::new();
    assert!(extract_credential(&h).is_none());
}

#[test]
fn extract_credential_accepts_raw_token_without_bearer_prefix() {
    let h = headers(&[("authorization", "raw-token-without-bearer")]);
    assert_eq!(
        extract_credential(&h).as_deref(),
        Some("raw-token-without-bearer")
    );
}

#[test]
fn build_authz_request_carries_user_route_and_model() {
    let user_id = UserId::new("user-1");
    let route_id = RouteId::new("route-1");
    let model = ModelId::new("claude-sonnet-4-6");
    let trace_id = TraceId::new("trace-1");

    let req = build_gateway_authz_request(GatewayAuthzRequestInput {
        user_id: user_id.clone(),
        roles: vec!["admin".to_owned()],
        attributes: BTreeMap::new(),
        act_chain: vec![],
        trace_id: trace_id.clone(),
        route_id: route_id.clone(),
        model: model.clone(),
        session_id: None,
        context_id: ContextId::new("33333333-3333-4333-8333-333333333333"),
    });

    assert_eq!(req.user_id, user_id);
    assert_eq!(req.trace_id, trace_id);
    assert_eq!(req.roles, vec!["admin"]);
    assert!(req.act_chain.is_empty());
    match &req.entity {
        EntityRef::GatewayRoute(r) => assert_eq!(r, &route_id),
        other => panic!("expected GatewayRoute entity, got {other:?}"),
    }
}

#[test]
fn build_authz_request_preserves_act_chain_and_attributes() {
    let user_id = UserId::new("user-2");
    let actor = Actor::user(UserId::new("delegator"));
    let mut attrs = BTreeMap::new();
    attrs.insert("tier".to_owned(), serde_json::json!("pro"));

    let req = build_gateway_authz_request(GatewayAuthzRequestInput {
        user_id,
        roles: vec![],
        attributes: attrs,
        act_chain: vec![actor.clone()],
        trace_id: TraceId::new("t2"),
        route_id: RouteId::new("r2"),
        model: ModelId::new("gpt-5"),
        session_id: None,
        context_id: ContextId::new("33333333-3333-4333-8333-333333333333"),
    });

    assert_eq!(req.act_chain, vec![actor]);
    assert_eq!(
        req.attributes.get("tier").and_then(|v| v.as_str()),
        Some("pro")
    );
}
