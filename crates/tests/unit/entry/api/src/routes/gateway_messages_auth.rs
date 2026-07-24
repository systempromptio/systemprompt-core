//! Unit tests for the gateway message-dispatch principal: accessor mapping and
//! session-binding enforcement for JWT vs API-key credentials.

use std::collections::BTreeMap;

use axum::http::StatusCode;
use serde_json::json;
use systemprompt_api::routes::gateway::messages::test_api::{
    ApiKeyPrincipal, AuthedPrincipal, JwtPrincipal,
};
use systemprompt_identifiers::{Actor, SessionId, TraceId, UserId};

fn jwt_principal(session: &SessionId) -> AuthedPrincipal {
    let mut attributes = BTreeMap::new();
    attributes.insert("team".to_owned(), json!("core"));
    AuthedPrincipal::Jwt(JwtPrincipal {
        user_id: UserId::new("user-jwt"),
        trace_id: TraceId::new("trace-jwt"),
        roles: vec!["admin".to_owned()],
        attributes,
        act_chain: vec![Actor::user(UserId::new("delegator"))],
        attested_session: session.clone(),
    })
}

fn api_key_principal(session: &SessionId) -> AuthedPrincipal {
    AuthedPrincipal::ApiKey(ApiKeyPrincipal {
        user_id: UserId::new("user-key"),
        trace_id: TraceId::new("trace-key"),
        attested_session: session.clone(),
    })
}

#[test]
fn accessors_map_per_variant() {
    let session = SessionId::generate();
    let jwt = jwt_principal(&session);
    assert_eq!(jwt.user_id().as_str(), "user-jwt");
    assert_eq!(jwt.trace_id().as_str(), "trace-jwt");
    assert_eq!(jwt.attested_session(), &session);

    let key = api_key_principal(&session);
    assert_eq!(key.user_id().as_str(), "user-key");
    assert_eq!(key.trace_id().as_str(), "trace-key");
    assert_eq!(key.attested_session(), &session);
}

#[test]
fn authz_attributes_come_from_jwt_only() {
    let session = SessionId::generate();
    let (roles, attributes, act_chain) = jwt_principal(&session).authz_attributes();
    assert_eq!(roles, vec!["admin".to_owned()]);
    assert_eq!(attributes.get("team"), Some(&json!("core")));
    assert_eq!(act_chain.len(), 1);

    let (roles, attributes, act_chain) = api_key_principal(&session).authz_attributes();
    assert!(roles.is_empty());
    assert!(attributes.is_empty());
    assert!(act_chain.is_empty());
}

#[test]
fn session_binding_accepts_matching_session() {
    let session = SessionId::generate();
    assert!(
        jwt_principal(&session)
            .enforce_session_binding(&session)
            .is_ok()
    );
}

#[test]
fn session_binding_rejects_mismatched_session() {
    let session = SessionId::generate();
    let other = SessionId::generate();
    let (status, message) = jwt_principal(&session)
        .enforce_session_binding(&other)
        .expect_err("mismatched session must be rejected");
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(message.contains("X-Session-ID"));
}

#[test]
fn session_binding_accepts_attested_api_key_session() {
    let session = SessionId::generate();
    assert!(
        api_key_principal(&session)
            .enforce_session_binding(&session)
            .is_ok()
    );
}

#[test]
fn session_binding_rejects_mismatched_api_key_session() {
    let session = SessionId::generate();
    let other = SessionId::generate();
    let (status, message) = api_key_principal(&session)
        .enforce_session_binding(&other)
        .expect_err("an api-key principal is bound to its attested session too");
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(message.contains("X-Session-ID"));
}
