//! Which models the bridge profile lists for a caller: dispatch's own check.
//!
//! A model is admitted exactly when `/v1/messages` would serve it to this
//! caller: the catalog resolves it ([`probe_route`], the same resolution
//! dispatch runs) and the authz hook allows this principal on
//! the access route id the resolution names. Servability (a hop with a
//! credential for this caller) is the profile builder's filter, applied
//! before admission. A model not advertised
//! (`clients: []`) stays out of the list whatever admission says;
//! admission only ever removes models from it. The authz
//! question is asked through
//! [`AuthzDecisionHook::probe`](systemprompt_governance::authz::AuthzDecisionHook::probe),
//! which decides without writing an audit row, so building a profile never
//! looks like a burst of refused requests. [`probe_route`] is the one place
//! admission asks the catalog which route serves a model.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_governance::authz::{AuthzDecision, SharedAuthzHook};
use systemprompt_identifiers::{ClientId, ContextId, ModelId, RouteId, SessionId, TraceId, UserId};
use systemprompt_manifest::services::catalog::{ModelRequest, ResolvedCatalog};

use super::messages::{GatewayAuthzRequestInput, build_gateway_authz_request};

/// The caller a profile is being built for.
#[derive(Debug, Clone)]
pub struct AdmissionPrincipal {
    pub user_id: UserId,
    pub roles: Vec<String>,
    pub client_id: Option<ClientId>,
    pub session_id: SessionId,
}

#[derive(Clone, Copy)]
pub struct ModelAdmission<'a> {
    pub catalog: &'a ResolvedCatalog,
    pub hook: &'a SharedAuthzHook,
    pub principal: &'a AdmissionPrincipal,
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
        let Some(route_id) = probe_route(self.catalog, model) else {
            return false;
        };
        let principal = self.principal;
        let request = build_gateway_authz_request(GatewayAuthzRequestInput {
            user_id: principal.user_id.clone(),
            roles: principal.roles.clone(),
            attributes: std::collections::BTreeMap::new(),
            act_chain: Vec::new(),
            trace_id: TraceId::generate(),
            route_id,
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

pub fn probe_route(catalog: &ResolvedCatalog, model: &str) -> Option<RouteId> {
    catalog
        .resolve(&ModelRequest::new(model))
        .ok()
        .map(|target| target.access_route_id)
}
