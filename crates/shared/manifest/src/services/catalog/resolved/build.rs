//! Building a [`ResolvedCatalog`] from the four registries and the gateway.
//!
//! The registries are checked first (`validate`), then projected: each
//! credential, provider and endpoint once, and each model's deployments into
//! hops with the upstream name, price and governance already applied. The
//! cross-checks that need the projection (routes, default model, pricing)
//! run last in `validate_routes`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_identifiers::{ModelId, ProviderId};

use super::types::{
    ResolvedCredential, ResolvedEndpoint, ResolvedHop, ResolvedModel, ResolvedProvider,
};
use super::{ResolvedCatalog, betas};
use crate::services::catalog::deployments::{DeploymentTarget, PricingOverride};
use crate::services::catalog::error::{CatalogError, CatalogResult};
use crate::services::catalog::models::{ModelEntry, apply_capability_overrides};
use crate::services::catalog::{
    CatalogContext, CredentialRegistry, DeploymentRegistry, EndpointRef, ModelRegistry,
    ProviderRegistry,
};
use crate::services::gateway::GatewayConfig;

/// Borrowed inputs of [`ResolvedCatalog::build`].
#[derive(Debug, Clone, Copy)]
pub struct CatalogInput<'a> {
    pub credentials: &'a CredentialRegistry,
    pub providers: &'a ProviderRegistry,
    pub models: &'a ModelRegistry,
    pub deployments: &'a DeploymentRegistry,
    pub gateway: Option<&'a GatewayConfig>,
    pub default_image_model: Option<&'a ModelId>,
}

impl ResolvedCatalog {
    pub fn build(input: CatalogInput<'_>, context: &CatalogContext) -> CatalogResult<Self> {
        if let Some(gateway) = input.gateway {
            gateway.validate()?;
        }
        super::validate::validate_registries(&input)?;

        let credentials: BTreeMap<_, _> = input
            .credentials
            .iter()
            .map(|(id, entry)| {
                (
                    id.clone(),
                    ResolvedCredential {
                        id: id.clone(),
                        kind: entry.kind,
                        secret: entry.secret.clone(),
                        label: entry.label.clone(),
                    },
                )
            })
            .collect();

        let (providers, endpoints) = project_providers(input.providers, context)?;

        let catalog_route_id = input.gateway.map_or_else(
            || GatewayConfig::default().effective_catalog_route_id(None),
            |gateway| {
                let default_provider = gateway
                    .default_model
                    .as_ref()
                    .and_then(|m| input.deployments.get(m.as_str()))
                    .and_then(<[_]>::first)
                    .map(|target| &target.endpoint().provider);
                gateway.effective_catalog_route_id(default_provider)
            },
        );
        let mut catalog = Self {
            credentials,
            providers,
            endpoints,
            models: BTreeMap::new(),
            deployments: BTreeMap::new(),
            routes: input.gateway.map(|g| g.routes.clone()).unwrap_or_default(),
            default_model: input.gateway.and_then(|g| g.default_model.clone()),
            catalog_route_id,
            gateway_enabled: input.gateway.is_some_and(|g| g.enabled),
            default_image_model: input.default_image_model.cloned(),
        };
        for (id, entry) in input.models.iter() {
            let targets = input
                .deployments
                .get(id.as_str())
                .ok_or_else(|| CatalogError::ModelWithoutDeployment { model: id.clone() })?;
            let model = catalog.project_model(id, entry, targets);
            let hops = targets
                .iter()
                .map(|target| catalog.project_hop(&model, entry, target))
                .collect::<Vec<_>>();
            catalog.deployments.insert(id.clone(), hops);
            catalog.models.insert(id.clone(), model);
        }
        super::validate::validate_routes(&catalog)?;
        Ok(catalog)
    }

    fn project_model(
        &self,
        id: &ModelId,
        entry: &ModelEntry,
        targets: &[DeploymentTarget],
    ) -> ResolvedModel {
        let primary = targets.first();
        let defaults = primary
            .and_then(|t| self.providers.get(&t.endpoint().provider))
            .map(|p| p.capabilities)
            .unwrap_or_default();
        let clients = primary
            .and_then(DeploymentTarget::clients)
            .map_or_else(|| entry.clients.clone(), <[_]>::to_vec);
        ResolvedModel {
            id: id.clone(),
            upstream_model: entry.upstream_name(id).to_owned(),
            display_name: entry.display_name.clone(),
            capabilities: apply_capability_overrides(defaults, &entry.capabilities),
            limits: entry.limits,
            pricing: entry.pricing,
            governance: entry.governance,
            clients,
        }
    }

    fn project_hop(
        &self,
        model: &ResolvedModel,
        entry: &ModelEntry,
        target: &DeploymentTarget,
    ) -> ResolvedHop {
        let key = target.endpoint();
        let endpoint = &self.endpoints[key];
        let provider = &self.providers[&key.provider];
        let credential = self.credentials[&provider.credential].clone();
        ResolvedHop {
            model: model.id.clone(),
            provider: key.provider.clone(),
            endpoint: key.endpoint.clone(),
            credential,
            url: endpoint.url.clone(),
            wire: endpoint.wire,
            betas: endpoint.betas.clone(),
            upstream_model: target
                .upstream_model()
                .map_or_else(|| model.upstream_model.clone(), str::to_owned),
            pricing: PricingOverride::resolve(
                target.pricing(),
                endpoint.price_multiplier,
                entry.pricing,
            ),
            governance: entry.governance.unwrap_or(provider.governance),
            hosting: endpoint.hosting,
            region: endpoint.region.clone(),
            resilience: endpoint.resilience,
            google_search_enabled: provider.google_search_enabled,
            clients: target
                .clients()
                .map_or_else(|| entry.clients.clone(), <[_]>::to_vec),
            weight: None,
        }
    }
}

type ProviderProjection = (
    BTreeMap<ProviderId, ResolvedProvider>,
    BTreeMap<EndpointRef, ResolvedEndpoint>,
);

fn project_providers(
    registry: &ProviderRegistry,
    context: &CatalogContext,
) -> CatalogResult<ProviderProjection> {
    let mut providers = BTreeMap::new();
    let mut endpoints = BTreeMap::new();
    for (id, provider) in registry.iter() {
        let resolved_betas = betas::resolve_provider_betas(id, provider)?;
        for (endpoint_id, endpoint) in &provider.endpoints {
            let key = EndpointRef {
                provider: id.clone(),
                endpoint: endpoint_id.clone(),
            };
            endpoints.insert(
                key,
                ResolvedEndpoint {
                    provider: id.clone(),
                    id: endpoint_id.clone(),
                    wire: endpoint.wire,
                    url: endpoint.url.clone(),
                    betas: resolved_betas.get(endpoint_id).cloned().flatten(),
                    region: endpoint.region.clone(),
                    price_multiplier: endpoint.price_multiplier,
                    hosting: endpoint.effective_hosting(),
                    resilience: endpoint.resilience.unwrap_or(provider.resilience),
                },
            );
        }
        providers.insert(
            id.clone(),
            ResolvedProvider {
                id: id.clone(),
                credential: provider.credential.clone(),
                display_name: provider.display_name.clone(),
                description: provider.description.clone(),
                user_agent: provider.user_agent.as_deref().map(|ua| context.expand(ua)),
                forward_headers: provider.forward_headers.clone(),
                extra_headers: provider
                    .extra_headers
                    .iter()
                    .map(|(name, value)| (name.clone(), context.expand(value)))
                    .collect(),
                governance: provider.governance,
                enabled: provider.enabled,
                capabilities: provider.capabilities,
                resilience: provider.resilience,
                google_search_enabled: provider.google_search_enabled,
                endpoints: provider.endpoints.keys().cloned().collect(),
            },
        );
    }

    Ok((providers, endpoints))
}
