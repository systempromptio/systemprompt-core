//! Tests that the act-chain plumbing reaches the governance audit row.

use systemprompt_identifiers::{RouteId, TraceId, UserId};
use systemprompt_identifiers::{Actor, ActorKind};
use systemprompt_security::authz::{AuthzContext, AuthzRequest, EntityRef};

fn request_with_chain(chain: Vec<Actor>) -> AuthzRequest {
    AuthzRequest {
        entity: EntityRef::GatewayRoute(RouteId::new("claude-3")),
        user_id: UserId::new("u1"),
        roles: vec!["eng".to_owned()],
        department: "platform".to_owned(),
        trace_id: TraceId::new("trace-1"),
        context: AuthzContext::None,
        act_chain: chain,
    }
}

#[test]
fn authz_request_carries_act_chain_through_serde() {
    let chain = vec![
        Actor::user(UserId::new("outer")),
        Actor::user(UserId::new("inner")),
    ];
    let req = request_with_chain(chain.clone());
    let wire = serde_json::to_string(&req).expect("serialize");
    let parsed: AuthzRequest = serde_json::from_str(&wire).expect("deserialize");
    assert_eq!(parsed.act_chain.len(), 2);
    assert_eq!(parsed.act_chain[0].user_id.as_str(), "outer");
    assert!(matches!(parsed.act_chain[0].kind, ActorKind::User));
    assert_eq!(parsed.act_chain[1].user_id.as_str(), "inner");
}

#[test]
fn empty_act_chain_is_omitted_on_the_wire() {
    let req = request_with_chain(Vec::new());
    let wire = serde_json::to_value(&req).expect("serialize");
    assert!(
        wire.get("act_chain").is_none(),
        "empty act_chain must skip serialization"
    );
}

#[test]
fn act_chain_round_trips_through_request_context() {
    use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId};
    use systemprompt_models::execution::context::RequestContext;

    let chain = vec![Actor::user(UserId::new("delegate"))];
    let ctx = RequestContext::new(
        SessionId::new("s1"),
        TraceId::new("t1"),
        ContextId::generate(),
        AgentName::system(),
    )
    .with_act_chain(chain);

    assert_eq!(ctx.act_chain().len(), 1);
    assert_eq!(ctx.act_chain()[0].user_id.as_str(), "delegate");
}
