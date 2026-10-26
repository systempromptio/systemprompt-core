//! Regression coverage for the JWT-claims → `AuthzRequest` contract on the
//! gateway side. Locks in that `roles` and `attributes` resolved from the
//! authenticated principal are forwarded verbatim to the authz hook.

use std::collections::BTreeMap;

use systemprompt_api::routes::gateway::messages::{
    GatewayAuthzRequestInput, build_gateway_authz_request,
};
use systemprompt_governance::authz::{AuthzContext, EntityRef};
use systemprompt_identifiers::{ContextId, ModelId, RouteId, TraceId, UserId};

#[test]
fn forwards_roles_and_attributes_to_authz_request() {
    let mut attrs = BTreeMap::new();
    attrs.insert("acme.desk".to_owned(), serde_json::json!("fixed-income"));

    let req = build_gateway_authz_request(GatewayAuthzRequestInput {
        user_id: UserId::new("user_1"),
        roles: vec!["eng".to_owned(), "platform".to_owned()],
        attributes: attrs.clone(),
        act_chain: Vec::new(),
        trace_id: TraceId::new("trace-x"),
        route_id: Some(RouteId::new("route-1")),
        model: ModelId::new("claude-3"),
        provider: None,
        session_id: None,
        client_id: None,
        context_id: ContextId::from_uuid(uuid::Uuid::from_u128(1)),
    });

    assert_eq!(req.user_id.as_str(), "user_1");
    assert_eq!(req.roles, vec!["eng".to_owned(), "platform".to_owned()]);
    assert_eq!(req.attributes, attrs);
    assert!(matches!(req.entity, EntityRef::GatewayRoute(_)));
    assert_eq!(req.context.kind, AuthzContext::GATEWAY_INVOCATION_KIND);
    assert_eq!(
        req.context
            .gateway_invocation_model()
            .expect("model")
            .as_str(),
        "claude-3"
    );
    assert_eq!(
        req.context_id.as_ref(),
        Some(&ContextId::from_uuid(uuid::Uuid::from_u128(1)))
    );
    assert!(req.task_id.is_none());
}

#[test]
fn a_request_no_route_matched_is_decided_on_its_model() {
    let req = build_gateway_authz_request(GatewayAuthzRequestInput {
        user_id: UserId::new("user_1"),
        roles: Vec::new(),
        attributes: BTreeMap::new(),
        act_chain: Vec::new(),
        trace_id: TraceId::new("trace-x"),
        route_id: None,
        model: ModelId::new("claude-3"),
        provider: None,
        session_id: None,
        client_id: None,
        context_id: ContextId::from_uuid(uuid::Uuid::from_u128(1)),
    });

    assert!(matches!(req.entity, EntityRef::Model(ref model) if model.as_str() == "claude-3"));
}
