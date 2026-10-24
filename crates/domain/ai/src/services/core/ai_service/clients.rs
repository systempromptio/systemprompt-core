//! [`ProviderClients`]: the in-process provider clients, one per endpoint.
//!
//! A client speaks one wire to one URL, so the set is keyed by
//! `provider/endpoint`. It holds every endpoint of an enabled provider that
//! has a model deployed on it and a platform credential whose secret is
//! present; a personal credential is served only by the gateway, per caller,
//! and a provider with `enabled: false` contributes nothing. A request names
//! a model (and optionally a provider); [`ProviderClients::for_request`]
//! follows that model's deployments in failover order to the first endpoint
//! with a client.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::sync::Arc;

use systemprompt_manifest::services::catalog::{EndpointRef, ResolvedCatalog, ResolvedHop};

use crate::error::Result;
use crate::services::providers::{
    ProviderClient, ProviderClientParams, ProviderFactory, served_models_on,
};
use crate::services::upstream::{EndpointBinding, UpstreamTarget};

#[derive(Clone, Default)]
pub struct ProviderClients {
    by_endpoint: BTreeMap<EndpointRef, Arc<dyn ProviderClient>>,
}

impl std::fmt::Debug for ProviderClients {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.by_endpoint.keys()).finish()
    }
}

/// An endpoint the catalog deploys to that has no client, and why.
#[derive(Debug, Clone)]
pub struct WithheldEndpoint {
    pub endpoint: EndpointRef,
    pub reason: String,
}

impl ProviderClients {
    pub fn build(catalog: &ResolvedCatalog) -> Result<(Self, Vec<WithheldEndpoint>)> {
        let mut clients = Self::default();
        let mut withheld = Vec::new();
        for endpoint in catalog.endpoints() {
            let endpoint_ref = endpoint.endpoint_ref();
            let Some(provider) = catalog.provider(endpoint.provider.as_str()) else {
                continue;
            };
            if !provider.enabled {
                continue;
            }
            let models = served_models_on(catalog, &endpoint_ref);
            if models.is_empty() {
                continue;
            }
            let binding = match EndpointBinding::of_endpoint(catalog, &endpoint_ref) {
                Ok(binding) => binding,
                Err(e) => {
                    withheld.push(WithheldEndpoint {
                        endpoint: endpoint_ref,
                        reason: e.to_string(),
                    });
                    continue;
                },
            };
            if binding.credential.is_personal() {
                tracing::debug!(endpoint = %endpoint_ref, "personal-credential endpoint left to the gateway");
                continue;
            }
            let target = match UpstreamTarget::from_secrets(binding) {
                Ok(target) => target,
                Err(e) => {
                    tracing::warn!(
                        endpoint = %endpoint_ref,
                        error = %e,
                        "upstream credential unusable — endpoint withheld from the in-process service"
                    );
                    withheld.push(WithheldEndpoint {
                        endpoint: endpoint_ref,
                        reason: e.to_string(),
                    });
                    continue;
                },
            };
            let params = ProviderClientParams {
                name: provider.id.as_str(),
                target,
                google_search_enabled: provider.google_search_enabled,
                resilience: &endpoint.resilience,
                models: &models,
                default_model: catalog
                    .default_model()
                    .filter(|id| models.iter().any(|m| m.id == **id))
                    .map(systemprompt_identifiers::ModelId::as_str),
            };
            let client = ProviderFactory::create(&params)?;
            clients.by_endpoint.insert(endpoint_ref, client);
        }
        Ok((clients, withheld))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_endpoint.is_empty()
    }

    #[must_use]
    pub fn get(&self, endpoint: &EndpointRef) -> Option<&Arc<dyn ProviderClient>> {
        self.by_endpoint.get(endpoint)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&EndpointRef, &Arc<dyn ProviderClient>)> {
        self.by_endpoint.iter()
    }

    pub fn values(&self) -> impl Iterator<Item = &Arc<dyn ProviderClient>> {
        self.by_endpoint.values()
    }

    #[must_use]
    pub fn endpoints(&self) -> Vec<EndpointRef> {
        self.by_endpoint.keys().cloned().collect()
    }

    #[must_use]
    pub fn hop_for<'c>(
        &self,
        catalog: &'c ResolvedCatalog,
        provider: Option<&str>,
        model: &str,
    ) -> Option<&'c ResolvedHop> {
        let catalog_model = catalog.model(model).map(|m| m.id.clone()).or_else(|| {
            catalog
                .models()
                .find(|m| {
                    catalog
                        .deployments(m.id.as_str())
                        .iter()
                        .any(|hop| hop.upstream_model == model)
                })
                .map(|m| m.id.clone())
        })?;
        catalog
            .enabled_hops(catalog_model.as_str())
            .filter(|hop| provider.is_none_or(|p| hop.provider.as_str() == p))
            .find(|hop| self.by_endpoint.contains_key(&hop.endpoint_ref()))
    }

    #[must_use]
    pub fn for_request(
        &self,
        catalog: &ResolvedCatalog,
        provider: Option<&str>,
        model: &str,
    ) -> Option<Arc<dyn ProviderClient>> {
        if let Some(hop) = self.hop_for(catalog, provider, model) {
            return self.by_endpoint.get(&hop.endpoint_ref()).cloned();
        }
        self.by_endpoint
            .iter()
            .filter(|(endpoint, _)| provider.is_none_or(|p| endpoint.provider.as_str() == p))
            .find(|(_, client)| client.supports_model(model))
            .map(|(_, client)| Arc::clone(client))
    }
}
