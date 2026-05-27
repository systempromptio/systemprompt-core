//! Regression coverage for the JWT-claims → `AuthzRequest` contract on the
//! gateway side. Locks in that `roles` and `attributes` resolved from the
//! authenticated principal are forwarded verbatim to the authz hook.

use std::collections::BTreeMap;

use systemprompt_api::routes::gateway::messages::{
    GatewayAuthzRequestInput, build_gateway_authz_request,
};
use systemprompt_identifiers::{ModelId, RouteId, TraceId, UserId};
use systemprompt_security::authz::{AuthzContext, EntityRef};

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
        route_id: RouteId::new("route-1"),
        model: ModelId::new("claude-3"),
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
}
