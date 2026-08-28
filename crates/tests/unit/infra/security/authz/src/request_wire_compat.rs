//! `AuthzRequest` crosses the wire to an out-of-process hook, so the identity
//! fields added after `user_id` must be optional on the wire and derivable
//! from it. These tests pin that contract in both directions.

use systemprompt_identifiers::{Actor, ActorKind, ClientId, RouteId, TraceId, UserId};
use systemprompt_security::authz::{AuthzContext, AuthzRequest, EntityRef};
use systemprompt_security::policy::types::AccessScope;

fn legacy_wire() -> serde_json::Value {
    serde_json::json!({
        "entity": { "kind": "gateway_route", "id": "claude-3" },
        "user_id": "u1",
        "roles": ["eng"],
        "trace_id": "trace-1",
    })
}

fn request() -> AuthzRequest {
    AuthzRequest {
        entity: EntityRef::GatewayRoute(RouteId::new("claude-3")),
        user_id: UserId::new("u1"),
        actor: None,
        client_id: None,
        access_scope: None,
        roles: Vec::new(),
        attributes: std::collections::BTreeMap::new(),
        trace_id: TraceId::new("trace-1"),
        session_id: None,
        context: AuthzContext::none(),
        context_id: None,
        task_id: None,
        act_chain: Vec::new(),
    }
}

#[test]
fn wire_without_actor_falls_back_to_the_user() {
    let parsed: AuthzRequest = serde_json::from_value(legacy_wire()).expect("legacy shape parses");
    assert!(parsed.actor.is_none());
    assert!(parsed.client_id.is_none());
    assert!(parsed.access_scope.is_none());
    let actor = parsed.actor();
    assert_eq!(actor.user_id.as_str(), "u1");
    assert!(matches!(actor.kind, ActorKind::User));
}

#[test]
fn mcp_actor_and_client_round_trip() {
    let mut req = request().for_actor(Actor::mcp(UserId::new("u1"), "comms"));
    req.client_id = Some(ClientId::bridge());
    let wire = serde_json::to_value(&req).expect("serialize");
    let parsed: AuthzRequest = serde_json::from_value(wire).expect("deserialize");
    let actor = parsed.actor();
    assert!(matches!(actor.kind, ActorKind::Mcp { ref server_name } if server_name == "comms"));
    assert_eq!(parsed.client_id, Some(ClientId::bridge()));
}

#[test]
fn for_actor_keeps_user_id_in_step() {
    let req = request().for_actor(Actor::mcp(UserId::new("someone-else"), "email"));
    assert_eq!(req.user_id.as_str(), "someone-else");
    assert_eq!(req.actor().user_id.as_str(), "someone-else");
}

#[test]
fn unattributed_optionals_are_omitted_on_the_wire() {
    let wire = serde_json::to_value(request()).expect("serialize");
    for key in ["actor", "client_id", "access_scope"] {
        assert!(wire.get(key).is_none(), "{key} must be absent when unset");
    }
}

#[test]
fn verified_agent_id_comes_only_from_an_agent_delegate() {
    let mut req = request();
    assert!(req.verified_agent_id().is_none());

    req.act_chain = vec![Actor::user(UserId::new("delegate"))];
    assert!(
        req.verified_agent_id().is_none(),
        "a plain user delegate is not an agent identity"
    );

    req.act_chain = vec![
        Actor::agent(UserId::new("u1"), "planner"),
        Actor::user(UserId::new("origin")),
    ];
    assert_eq!(req.verified_agent_id(), Some("planner"));
}

#[test]
fn access_scope_survives_the_wire() {
    let mut req = request();
    req.access_scope = Some(AccessScope::Admin);
    let wire = serde_json::to_value(&req).expect("serialize");
    let parsed: AuthzRequest = serde_json::from_value(wire).expect("deserialize");
    assert_eq!(parsed.access_scope, Some(AccessScope::Admin));
}
