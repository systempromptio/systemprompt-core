//! The verified hop identity → `AuthzRequest` forwarding on the MCP
//! middleware path: the identity's roles, attributes, act chain, client and
//! session reach the hook input verbatim, joined to the request's own trace.

use std::collections::BTreeMap;

use systemprompt_governance::authz::{AuthzContext, EntityRef};
use systemprompt_identifiers::{
    Actor, AgentName, ClientId, ContextId, McpServerId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_mcp::middleware::rbac::build_hop_authz_request;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::{AuthenticatedUser, Permission};

const CONTEXT: &str = "22222222-2222-4222-8222-222222222222";

fn context_with(
    roles: Vec<String>,
    attributes: BTreeMap<String, serde_json::Value>,
    task: Option<&str>,
) -> RequestContext {
    let mut user = AuthenticatedUser::new_with_roles(
        UserId::new("user_42"),
        "u".to_owned(),
        "u@example.com".to_owned(),
        vec![Permission::User],
        roles,
    );
    user.attributes = attributes;
    let context = RequestContext::new(
        SessionId::new("s"),
        TraceId::generate(),
        ContextId::try_new(CONTEXT).expect("valid ContextId"),
        AgentName::system(),
        Actor::user(UserId::new("user_42")),
    )
    .with_user(user)
    .with_client_id(ClientId::new("c"))
    .with_act_chain(vec![Actor::user(UserId::new("user_42"))]);
    match task {
        Some(task) => context.with_task_id(TaskId::new(task)),
        None => context,
    }
}

fn server() -> McpServerId {
    McpServerId::try_new("server-x").expect("valid")
}

#[test]
fn forwards_roles_attributes_and_act_chain_from_the_identity() {
    let mut attrs = BTreeMap::new();
    attrs.insert("acme.desk".to_owned(), serde_json::json!("fixed-income"));
    let context = context_with(
        vec!["eng".to_owned(), "platform".to_owned()],
        attrs.clone(),
        Some("task-7"),
    );

    let req = build_hop_authz_request(&server(), &context, None);

    assert_eq!(req.user_id.as_str(), "user_42");
    assert_eq!(req.roles, vec!["eng".to_owned(), "platform".to_owned()]);
    assert_eq!(req.attributes, attrs);
    assert_eq!(req.act_chain.len(), 1);
    assert!(matches!(req.entity, EntityRef::McpServer(_)));
    assert_eq!(
        req.context_id,
        Some(ContextId::try_new(CONTEXT).expect("valid ContextId"))
    );
    assert_eq!(req.task_id, Some(TaskId::new("task-7")));
}

#[test]
fn an_anonymous_context_forwards_no_roles() {
    let context = RequestContext::new(
        SessionId::new("s"),
        TraceId::generate(),
        ContextId::try_new(CONTEXT).expect("valid ContextId"),
        AgentName::system(),
        Actor::anonymous(UserId::new("anon")),
    );
    let req = build_hop_authz_request(&server(), &context, None);
    assert!(req.roles.is_empty());
    assert!(req.attributes.is_empty());
    assert!(req.task_id.is_none());
}

#[test]
fn no_floor_yields_plain_none_context() {
    let req = build_hop_authz_request(
        &server(),
        &context_with(vec![], BTreeMap::new(), None),
        None,
    );
    assert!(req.context.is_none());
    assert!(req.context.marketplace_floor().is_none());
}

#[test]
fn carries_marketplace_floor_when_present() {
    let mut floor = BTreeMap::new();
    floor.insert(
        "boeing.clearance".to_owned(),
        serde_json::json!(["Internal", "CUI"]),
    );
    let req = build_hop_authz_request(
        &server(),
        &context_with(vec![], BTreeMap::new(), None),
        Some(&floor),
    );
    assert_eq!(req.context.kind, AuthzContext::NONE_KIND);
    assert_eq!(req.context.marketplace_floor(), Some(floor));
}

#[test]
fn records_the_mcp_surface_and_joins_to_the_request() {
    let context = context_with(vec![], BTreeMap::new(), None);
    let req = build_hop_authz_request(&server(), &context, None);

    let actor = req.actor();
    assert_eq!(actor.user_id.as_str(), "user_42");
    assert!(
        matches!(actor.kind, systemprompt_identifiers::ActorKind::Mcp { ref server_name } if server_name == "server-x"),
        "the MCP server is the surface the user acted through"
    );
    assert_eq!(&req.trace_id, context.trace_id());
    assert_eq!(req.session_id, Some(SessionId::new("s")));
    assert_eq!(req.client_id, Some(ClientId::new("c")));
}
