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
use systemprompt_governance::authz::{
    AuthzContext, AuthzDecision, AuthzRequest, EntityRef, SharedAuthzHook,
};
use systemprompt_identifiers::{
    Actor, ClientId, ContextId, ModelId, ProviderId, RouteId, SessionId, TraceId, UserId,
};

use super::super::auth::AuthedPrincipal;
use super::super::error::RejectionError;

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
    // JSON: ABAC attribute bag — JWT claim values are policy-defined and schema-less.
    pub attributes: BTreeMap<String, serde_json::Value>,
    pub act_chain: Vec<Actor>,
    pub trace_id: TraceId,
    pub route_id: Option<RouteId>,
    pub model: ModelId,
    pub provider: Option<ProviderId>,
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
        provider,
        session_id,
        client_id,
        context_id,
    } = input;
    AuthzRequest {
        entity: route_id.map_or_else(|| EntityRef::Model(model.clone()), EntityRef::GatewayRoute),
        actor: Some(Actor::user(user_id.clone())),
        user_id,
        client_id,
        access_scope: None,
        roles,
        attributes,
        trace_id,
        session_id,
        context: AuthzContext::gateway_invocation(&model, provider.as_ref()),
        act_chain,
        context_id: Some(context_id),
        task_id: None,
    }
}

/// What a pre-dispatch decision is about: the route the request resolved to
/// (none when no declared route matched), the model it asked for and the
/// provider serving that model first.
#[derive(Debug, Clone, Copy)]
pub struct DispatchTarget<'a> {
    pub route_id: Option<&'a RouteId>,
    pub model: &'a str,
    pub provider: &'a ProviderId,
}

impl<'a> DispatchTarget<'a> {
    #[must_use]
    pub const fn new(
        route_id: Option<&'a RouteId>,
        model: &'a str,
        provider: &'a ProviderId,
    ) -> Self {
        Self {
            route_id,
            model,
            provider,
        }
    }
}

pub async fn enforce_authz_pre_dispatch(
    principal: &AuthedPrincipal,
    target: DispatchTarget<'_>,
    context_id: &ContextId,
    hook: &SharedAuthzHook,
) -> Result<(), RejectionError> {
    let (roles, attributes, act_chain) = principal.authz_attributes();
    let req = build_gateway_authz_request(GatewayAuthzRequestInput {
        user_id: principal.user_id().clone(),
        roles,
        attributes,
        act_chain,
        trace_id: principal.trace_id().clone(),
        route_id: target.route_id.cloned(),
        model: ModelId::new(target.model),
        provider: Some(target.provider.clone()),
        session_id: Some(principal.session_id().clone()),
        client_id: principal.client_id().cloned(),
        context_id: context_id.clone(),
    });
    match hook.evaluate(req).await {
        AuthzDecision::Allow => Ok(()),
        AuthzDecision::Deny { reason, policy } => Err(RejectionError::client(
            StatusCode::FORBIDDEN,
            format!("authz denied [{policy}]: {reason}"),
        )),
    }
}
