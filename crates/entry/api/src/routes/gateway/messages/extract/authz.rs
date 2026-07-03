//! Pre-dispatch authorization for gateway requests.
//!
//! [`build_gateway_authz_request`] and [`GatewayAuthzRequestInput`] are public
//! so the JWT-claims forwarding contract can be exercised directly from unit
//! tests without standing up the full principal/route stack.

use axum::http::StatusCode;
use std::collections::BTreeMap;
use systemprompt_identifiers::{Actor, ModelId, RouteId, SessionId, TraceId, UserId};
use systemprompt_security::authz::{
    AuthzContext, AuthzDecision, AuthzRequest, EntityRef, SharedAuthzHook,
};

use super::super::auth::AuthedPrincipal;

#[derive(Debug, Clone)]
pub struct GatewayAuthzRequestInput {
    pub user_id: UserId,
    pub roles: Vec<String>,
    pub attributes: BTreeMap<String, serde_json::Value>,
    pub act_chain: Vec<Actor>,
    pub trace_id: TraceId,
    pub route_id: RouteId,
    pub model: ModelId,
    pub session_id: Option<SessionId>,
}

#[must_use]
pub fn build_gateway_authz_request(input: GatewayAuthzRequestInput) -> AuthzRequest {
    let GatewayAuthzRequestInput {
        user_id,
        roles,
        attributes,
        act_chain,
        trace_id,
        route_id,
        model,
        session_id,
    } = input;
    AuthzRequest {
        entity: EntityRef::GatewayRoute(route_id),
        user_id,
        roles,
        attributes,
        trace_id,
        session_id,
        context: AuthzContext::gateway_invocation(&model),
        act_chain,
        context_id: None,
        task_id: None,
    }
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub async fn enforce_authz_pre_dispatch(
    principal: &AuthedPrincipal,
    route: &systemprompt_models::profile::GatewayRoute,
    model: &str,
    hook: &SharedAuthzHook,
) -> Result<(), (StatusCode, String)> {
    let route_id = if route.id.as_str().trim().is_empty() {
        systemprompt_models::profile::synthesize_route_id(
            &route.model_pattern,
            route.provider.as_str(),
        )
    } else {
        route.id.clone()
    };
    let (roles, attributes, act_chain) = principal.authz_attributes();
    let req = build_gateway_authz_request(GatewayAuthzRequestInput {
        user_id: principal.user_id().clone(),
        roles,
        attributes,
        act_chain,
        trace_id: principal.trace_id().clone(),
        route_id,
        model: ModelId::new(model),
        session_id: principal.attested_session().cloned(),
    });
    match hook.evaluate(req).await {
        AuthzDecision::Allow => Ok(()),
        AuthzDecision::Deny { reason, policy } => Err((
            StatusCode::FORBIDDEN,
            format!("authz denied [{policy}]: {reason}"),
        )),
    }
}
