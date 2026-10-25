//! Resolving one request: `model → deployment → endpoint → provider →
//! credential`.
//!
//! The requested id loses its `[1m]` marker, an alias route may rename it,
//! and it must then be a catalog model. The first declared route (in file
//! order) whose globs match the model and whose `when` matches the request
//! is the policy route: its `deployments` narrow or pin the model's chain,
//! its `strategy` orders it and its `context_fallbacks` follow it. Hops on a
//! disabled provider are dropped. No glob ever selects an endpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::attribution::RequestAttribution;
use systemprompt_models::providers::without_context_variant;
use systemprompt_wire::canonical::CanonicalRequest;

use super::ResolvedCatalog;
use super::types::{ResolvedHop, ResolvedTarget};
use crate::services::catalog::error::{CatalogError, CatalogResult, ResolveError};
use crate::services::gateway::{GatewayRoute, SelectionStrategy};
use systemprompt_identifiers::ModelId;

/// One request to resolve: the requested model id and, when known, the
/// request's scope attribution and canonical body (for `when:` predicates).
#[derive(Debug, Clone, Copy)]
pub struct ModelRequest<'a> {
    pub model: &'a str,
    pub attribution: Option<&'a RequestAttribution>,
    pub request: Option<&'a CanonicalRequest>,
}

impl<'a> ModelRequest<'a> {
    #[must_use]
    pub const fn new(model: &'a str) -> Self {
        Self {
            model,
            attribution: None,
            request: None,
        }
    }

    #[must_use]
    pub const fn with_attribution(mut self, attribution: &'a RequestAttribution) -> Self {
        self.attribution = Some(attribution);
        self
    }

    #[must_use]
    pub const fn with_request(mut self, request: &'a CanonicalRequest) -> Self {
        self.request = Some(request);
        self
    }
}

impl ResolvedCatalog {
    pub fn resolve(&self, request: &ModelRequest<'_>) -> Result<ResolvedTarget, ResolveError> {
        let requested = without_context_variant(request.model);
        let matches_when =
            |route: &GatewayRoute| route.matches_when(request.attribution, request.request);
        let alias = self.routes.iter().find_map(|route| {
            route
                .aliases(requested)
                .filter(|_| matches_when(route))
                .map(|m| (route, m))
        });
        let (route, model_id) = alias.map_or_else(
            || {
                let route = self.routes.iter().find(|route| {
                    route.alias.is_none() && route.matches_model(requested) && matches_when(route)
                });
                (route, requested)
            },
            |(route, model)| (Some(route), model.as_str()),
        );
        let model = self
            .models
            .get(model_id)
            .ok_or_else(|| ResolveError::UnknownModel {
                requested: requested.to_owned(),
            })?;
        let chain: Vec<ResolvedHop> = route
            .map_or_else(
                || self.deployments(model.id.as_str()).to_vec(),
                |route| self.route_chain(route, &model.id).unwrap_or_default(),
            )
            .into_iter()
            .filter(|hop| self.is_enabled(hop))
            .collect();
        if chain.is_empty() {
            return Err(ResolveError::NoEnabledDeployment {
                model: model.id.clone(),
            });
        }
        let context_fallbacks = route
            .map(|r| r.context_fallbacks.as_slice())
            .unwrap_or_default()
            .iter()
            .filter_map(|fallback| self.enabled_hops(fallback.as_str()).next().cloned())
            .collect();
        Ok(ResolvedTarget {
            requested: requested.to_owned(),
            model: model.clone(),
            chain,
            context_fallbacks,
            strategy: route.map_or(SelectionStrategy::Ordered, |r| r.strategy),
            route: route.map(|r| r.id.clone()),
        })
    }

    pub(super) fn route_chain(
        &self,
        route: &GatewayRoute,
        model: &ModelId,
    ) -> CatalogResult<Vec<ResolvedHop>> {
        let hops = self
            .deployments
            .get(model)
            .map(Vec::as_slice)
            .unwrap_or_default();
        if route.deployments.is_empty() {
            return Ok(hops.to_vec());
        }
        route
            .deployments
            .iter()
            .map(|pinned| {
                hops.iter()
                    .find(|hop| hop.endpoint_ref() == *pinned.endpoint())
                    .map(|hop| ResolvedHop {
                        weight: pinned.weight(),
                        ..hop.clone()
                    })
                    .ok_or_else(|| CatalogError::RouteDeploymentNotDeployed {
                        route: route.id.clone(),
                        model: model.clone(),
                        endpoint: pinned.endpoint().to_string(),
                    })
            })
            .collect()
    }
}
