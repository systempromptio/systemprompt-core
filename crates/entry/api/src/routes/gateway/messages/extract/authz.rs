//! Pre-dispatch authorization for gateway requests.
//!
//! The request itself is built by
//! [`systemprompt_gateway::surface::authz::build_gateway_authz_request`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::StatusCode;
use systemprompt_gateway::surface::authz::{GatewayAuthzRequestInput, build_gateway_authz_request};
use systemprompt_governance::authz::{AuthzDecision, SharedAuthzHook};
use systemprompt_identifiers::{ContextId, ModelId, ProviderId, RouteId};

use super::super::auth::AuthedPrincipal;
use super::super::error::RejectionError;

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
