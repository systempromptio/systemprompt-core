//! The resolved catalog: the one runtime view of credentials, providers,
//! endpoints, models, deployments and policy routes.
//!
//! [`ResolvedCatalog::build`] validates the four registries and the gateway
//! together and projects them; [`ResolvedCatalog::resolve`] answers what one
//! request dispatches to. Everything that needs a model's wire, URL, betas,
//! credential, upstream name or price reads it from here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod betas;
mod build;
mod resolve;
mod types;
mod validate;
mod validate_hops;

use std::collections::BTreeMap;

use systemprompt_identifiers::{CredentialId, ModelId, ProviderId, RouteId};
use systemprompt_models::providers::ClientFamily;

use super::endpoint::EndpointRef;
use crate::services::gateway::GatewayRoute;

pub use build::CatalogInput;
pub use resolve::ModelRequest;
pub use types::{
    ResolvedCredential, ResolvedEndpoint, ResolvedHop, ResolvedModel, ResolvedProvider,
    ResolvedTarget,
};

#[derive(Debug, Clone)]
pub struct ResolvedCatalog {
    credentials: BTreeMap<CredentialId, ResolvedCredential>,
    providers: BTreeMap<ProviderId, ResolvedProvider>,
    endpoints: BTreeMap<EndpointRef, ResolvedEndpoint>,
    models: BTreeMap<ModelId, ResolvedModel>,
    deployments: BTreeMap<ModelId, Vec<ResolvedHop>>,
    routes: Vec<GatewayRoute>,
    default_model: Option<ModelId>,
    catalog_route_id: RouteId,
    gateway_enabled: bool,
    default_image_model: Option<ModelId>,
}

impl ResolvedCatalog {
    #[must_use]
    pub fn credential(&self, id: &str) -> Option<&ResolvedCredential> {
        self.credentials.get(id)
    }

    pub fn credentials(&self) -> impl Iterator<Item = &ResolvedCredential> {
        self.credentials.values()
    }

    #[must_use]
    pub fn provider(&self, id: &str) -> Option<&ResolvedProvider> {
        self.providers.get(id)
    }

    pub fn providers(&self) -> impl Iterator<Item = &ResolvedProvider> {
        self.providers.values()
    }

    #[must_use]
    pub fn endpoint(&self, endpoint: &EndpointRef) -> Option<&ResolvedEndpoint> {
        self.endpoints.get(endpoint)
    }

    pub fn endpoints(&self) -> impl Iterator<Item = &ResolvedEndpoint> {
        self.endpoints.values()
    }

    pub fn endpoints_for_credential<'a>(
        &'a self,
        credential: &'a str,
    ) -> impl Iterator<Item = &'a ResolvedEndpoint> {
        self.endpoints.values().filter(move |endpoint| {
            self.providers
                .get(&endpoint.provider)
                .is_some_and(|p| p.credential.as_str() == credential)
        })
    }

    #[must_use]
    pub fn model(&self, id: &str) -> Option<&ResolvedModel> {
        self.models.get(id)
    }

    pub fn models(&self) -> impl Iterator<Item = &ResolvedModel> {
        self.models.values()
    }

    pub fn advertised_models(
        &self,
        family: Option<ClientFamily>,
    ) -> impl Iterator<Item = &ResolvedModel> {
        self.models.values().filter(move |model| {
            family.map_or_else(|| model.is_advertised(), |f| model.is_advertised_to(f))
        })
    }

    #[must_use]
    pub fn deployments(&self, model: &str) -> &[ResolvedHop] {
        self.deployments.get(model).map_or(&[], Vec::as_slice)
    }

    pub fn enabled_hops<'a>(&'a self, model: &str) -> impl Iterator<Item = &'a ResolvedHop> {
        self.deployments(model)
            .iter()
            .filter(|hop| self.is_enabled(hop))
    }

    #[must_use]
    pub fn is_enabled(&self, hop: &ResolvedHop) -> bool {
        self.providers.get(&hop.provider).is_some_and(|p| p.enabled)
    }

    #[must_use]
    pub fn routes(&self) -> &[GatewayRoute] {
        &self.routes
    }

    #[must_use]
    pub fn route(&self, id: &str) -> Option<&GatewayRoute> {
        self.routes.iter().find(|route| route.id.as_str() == id)
    }

    #[must_use]
    pub fn route_ids(&self) -> Vec<RouteId> {
        let mut ids: Vec<RouteId> = self.routes.iter().map(|r| r.id.clone()).collect();
        if !ids.contains(&self.catalog_route_id) {
            ids.push(self.catalog_route_id.clone());
        }
        ids
    }

    #[must_use]
    pub const fn default_model(&self) -> Option<&ModelId> {
        self.default_model.as_ref()
    }

    #[must_use]
    pub const fn default_image_model(&self) -> Option<&ModelId> {
        self.default_image_model.as_ref()
    }

    #[must_use]
    pub const fn catalog_route_id(&self) -> &RouteId {
        &self.catalog_route_id
    }

    #[must_use]
    pub const fn gateway_enabled(&self) -> bool {
        self.gateway_enabled
    }
}
