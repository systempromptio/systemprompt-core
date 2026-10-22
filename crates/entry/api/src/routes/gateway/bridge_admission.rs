//! Which models the bridge profile lists for a caller: dispatch's own check.
//!
//! A model is admitted exactly when `/v1/messages` would serve it to this
//! caller: a route resolves for it ([`probe_route`], the same
//! `resolve_route` dispatch runs, so `hidden_requires_route` holds), the
//! route's provider has a credential for this caller (the profile builder's
//! servability check), and the authz hook allows this principal on that route.
//! A `hidden` model stays out of the advertised list whatever admission says;
//! admission only ever removes models from it. The authz
//! question is asked through
//! [`AuthzDecisionHook::probe`](systemprompt_governance::authz::AuthzDecisionHook::probe),
//! which decides without writing an audit row, so building a profile never
//! looks like a burst of refused requests. [`probe_route`] is the one place
//! admission asks the catalog which route serves a model.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;

use systemprompt_governance::authz::{AuthzDecision, SharedAuthzHook};
use systemprompt_identifiers::{ClientId, ContextId, ModelId, SessionId, TraceId, UserId};
use systemprompt_manifest::services::{GatewayConfig, GatewayRoute, ProviderRegistry};
use systemprompt_wire::canonical::CanonicalRequest;

use super::messages::{GatewayAuthzRequestInput, build_gateway_authz_request};

/// The caller a profile is being built for.
#[derive(Debug, Clone)]
pub struct AdmissionPrincipal {
    pub user_id: UserId,
    pub roles: Vec<String>,
    pub client_id: Option<ClientId>,
    pub session_id: SessionId,
}

/// Whether the provider serving a model has a credential for this caller —
/// the profile builder's servability check, passed in so admission and the
/// builder answer it the same way.
pub type Servable<'a> = &'a (dyn Fn(&str) -> bool + Sync);

#[derive(Clone, Copy)]
pub struct ModelAdmission<'a> {
    pub gateway: &'a GatewayConfig,
    pub registry: &'a ProviderRegistry,
    pub hook: &'a SharedAuthzHook,
    pub principal: &'a AdmissionPrincipal,
    pub servable: Servable<'a>,
}

impl std::fmt::Debug for ModelAdmission<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelAdmission")
            .field("principal", &self.principal)
            .finish_non_exhaustive()
    }
}

impl ModelAdmission<'_> {
    pub async fn admits(&self, model: &str) -> bool {
        let Some(route) = probe_route(self.gateway, self.registry, model) else {
            return false;
        };
        if !(self.servable)(model) {
            return false;
        }
        let principal = self.principal;
        let request = build_gateway_authz_request(GatewayAuthzRequestInput {
            user_id: principal.user_id.clone(),
            roles: principal.roles.clone(),
            attributes: std::collections::BTreeMap::new(),
            act_chain: Vec::new(),
            trace_id: TraceId::generate(),
            route_id: route.effective_id(),
            model: ModelId::new(model),
            session_id: Some(principal.session_id.clone()),
            client_id: principal.client_id.clone(),
            context_id: ContextId::derived_from_session(&principal.session_id),
        });
        matches!(self.hook.probe(request).await, AuthzDecision::Allow)
    }

    pub async fn retain(&self, models: Vec<String>) -> Vec<String> {
        let mut admitted = Vec::with_capacity(models.len());
        for model in models {
            if self.admits(&model).await {
                admitted.push(model);
            }
        }
        admitted
    }
}

pub fn probe_route<'a>(
    gateway: &'a GatewayConfig,
    registry: &ProviderRegistry,
    model: &str,
) -> Option<Cow<'a, GatewayRoute>> {
    let probe = CanonicalRequest::new(ModelId::new(model), Vec::new(), 1);
    gateway.resolve_route(registry, &probe)
}
