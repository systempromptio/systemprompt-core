//! Regression coverage for the JWT-claims → `AuthzRequest` contract on the
//! gateway side. Locks in that `roles` and `department` resolved from the
//! authenticated principal are forwarded verbatim to the authz hook.

use systemprompt_api::routes::gateway::messages::build_gateway_authz_request;
use systemprompt_identifiers::{ModelId, RouteId, TraceId, UserId};
use systemprompt_security::authz::{AuthzContext, EntityRef};

#[test]
fn forwards_roles_and_department_to_authz_request() {
    let req = build_gateway_authz_request(
        UserId::new("user_1"),
        vec!["eng".to_owned(), "platform".to_owned()],
        "infra".to_owned(),
        Vec::new(),
        TraceId::new("trace-x"),
        RouteId::new("route-1"),
        ModelId::new("claude-3"),
    );

    assert_eq!(req.user_id.as_str(), "user_1");
    assert_eq!(req.roles, vec!["eng".to_owned(), "platform".to_owned()]);
    assert_eq!(req.department, "infra");
    assert!(matches!(req.entity, EntityRef::GatewayRoute(_)));
    match req.context {
        AuthzContext::GatewayInvocation { model } => assert_eq!(model.as_str(), "claude-3"),
        other => panic!("expected GatewayInvocation, got {other:?}"),
    }
}
