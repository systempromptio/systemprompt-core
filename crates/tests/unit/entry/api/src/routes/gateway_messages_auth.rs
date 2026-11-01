//! Unit tests for the gateway principal: every credential kind resolves to one
//! `Principal`, so an API key carries its user's roles exactly as a JWT does,
//! and its scopes narrow that principal rather than replacing it.

use std::collections::BTreeMap;

use serde_json::json;
use systemprompt_api::routes::gateway::messages::auth::{AuthedPrincipal, KeyScopes, Principal};
use systemprompt_governance::authz::SubjectAttributes;
use systemprompt_governance::policy::types::AccessScope;
use systemprompt_identifiers::{Actor, ClientId, SessionId, TraceId, UserId};

fn principal(roles: &[&str], scopes: Option<KeyScopes>) -> Principal {
    Principal {
        user_id: UserId::new("user-1"),
        roles: roles.iter().map(|r| (*r).to_owned()).collect(),
        subject: SubjectAttributes::new(),
        attributes: BTreeMap::new(),
        act_chain: Vec::new(),
        client_id: None,
        scopes,
        actor: systemprompt_models::auth::RequestActor::Human,
    }
}

fn key_scopes() -> KeyScopes {
    KeyScopes {
        api_key_id: systemprompt_identifiers::ApiKeyId::generate(),
        limits: systemprompt_users::ApiKeyLimits::default(),
        scopes: Vec::new(),
    }
}

fn authed(principal: Principal, session: &SessionId) -> AuthedPrincipal {
    AuthedPrincipal {
        principal,
        trace_id: TraceId::new("trace-1"),
        session_id: session.clone(),
        client_session: None,
    }
}

#[test]
fn accessors_read_the_principal_and_its_session() {
    let session = SessionId::generate();
    let key = authed(principal(&["user"], Some(key_scopes())), &session);
    assert_eq!(key.user_id().as_str(), "user-1");
    assert_eq!(key.trace_id().as_str(), "trace-1");
    assert_eq!(key.session_id(), &session);
    assert!(key.api_key().is_some());
    assert!(key.client_session().is_none());
}

#[test]
fn an_api_key_principal_carries_its_users_roles() {
    let session = SessionId::generate();
    let key = authed(principal(&["admin"], Some(key_scopes())), &session);
    let (roles, attributes, act_chain) = key.authz_attributes();
    assert_eq!(roles, vec!["admin".to_owned()]);
    assert!(attributes.is_empty());
    assert!(act_chain.is_empty());
    assert_eq!(key.access_scope(), AccessScope::Admin);
}

#[test]
fn jwt_and_api_key_principals_with_the_same_roles_get_the_same_scope() {
    let session = SessionId::generate();
    let jwt = authed(principal(&["user"], None), &session);
    let key = authed(principal(&["user"], Some(key_scopes())), &session);
    assert_eq!(jwt.access_scope(), key.access_scope());
    assert_eq!(jwt.authz_attributes().0, key.authz_attributes().0);
}

#[test]
fn jwt_attributes_and_act_chain_pass_through() {
    let session = SessionId::generate();
    let mut jwt = principal(&["admin"], None);
    jwt.attributes.insert("team".to_owned(), json!("core"));
    jwt.act_chain = vec![Actor::user(UserId::new("delegator"))];
    let (roles, attributes, act_chain) = authed(jwt, &session).authz_attributes();
    assert_eq!(roles, vec!["admin".to_owned()]);
    assert_eq!(attributes.get("team"), Some(&json!("core")));
    assert_eq!(act_chain.len(), 1);
}

#[test]
fn only_the_bridge_client_reads_as_bridge() {
    let session = SessionId::generate();
    let mut bridge = principal(&["user"], None);
    bridge.client_id = Some(ClientId::bridge());
    assert!(authed(bridge, &session).is_bridge());
    assert!(!authed(principal(&["user"], Some(key_scopes())), &session).is_bridge());
}
