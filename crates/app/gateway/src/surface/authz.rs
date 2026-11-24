//! The gateway's authorization request: what a pre-dispatch decision and
//! the bridge's model admission ask the authorization hook.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use systemprompt_governance::authz::{AuthzContext, AuthzRequest, EntityRef};
use systemprompt_identifiers::{
    Actor, ClientId, ContextId, ModelId, ProviderId, RouteId, SessionId, TraceId, UserId,
};

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
