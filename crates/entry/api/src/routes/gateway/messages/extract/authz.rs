//! Pre-dispatch authorization for gateway requests.
//!
//! [`build_gateway_authz_request`] and [`GatewayAuthzRequestInput`] are public
//! so the JWT-claims forwarding contract can be exercised directly from unit
//! tests without standing up the full principal/route stack.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::StatusCode;
use std::collections::BTreeMap;
use systemprompt_identifiers::{
    Actor, ClientId, ContextId, ModelId, RouteId, SessionId, TraceId, UserId,
};
use systemprompt_security::authz::{
    AuthzContext, AuthzDecision, AuthzRequest, EntityRef, SharedAuthzHook,
};

use super::super::auth::AuthedPrincipal;

/// The inputs to a pre-dispatch gateway authorization decision.
///
/// `context_id` is the context the request was already resolved into. Leaving
/// it unset made the audit sink re-derive one from the bridge session, so every
/// pre-dispatch decision landed in a different context from the request it
/// authorized.
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
    pub client_id: Option<ClientId>,
    pub context_id: ContextId,
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
        client_id,
        context_id,
    } = input;
    AuthzRequest {
        entity: EntityRef::GatewayRoute(route_id),
        actor: Some(Actor::user(user_id.clone())),
        user_id,
        client_id,
        access_scope: None,
        roles,
        attributes,
        trace_id,
        session_id,
        context: AuthzContext::gateway_invocation(&model),
        act_chain,
        context_id: Some(context_id),
        task_id: None,
    }
}

pub async fn enforce_authz_pre_dispatch(
    principal: &AuthedPrincipal,
    route: &systemprompt_models::services::GatewayRoute,
    model: &str,
    context_id: &ContextId,
    hook: &SharedAuthzHook,
) -> Result<(), (StatusCode, String)> {
    let route_id = if route.id.as_str().trim().is_empty() {
        systemprompt_models::services::synthesize_route_id(
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
        session_id: Some(principal.attested_session().clone()),
        client_id: principal.client_id().cloned(),
        context_id: context_id.clone(),
    });
    match hook.evaluate(req).await {
        AuthzDecision::Allow => Ok(()),
        AuthzDecision::Deny { reason, policy } => Err((
            StatusCode::FORBIDDEN,
            format!("authz denied [{policy}]: {reason}"),
        )),
    }
}
