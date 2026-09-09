//! Pricing resolution for gateway requests.
//!
//! `candidates` is tried in priority order — typically the provider-echoed
//! served model first, then the route's upstream model, then the
//! client-requested model. A provider that echoes a dated alias
//! (`gpt-5-mini-2025-08-07`) absent from the catalog must still bill against
//! the configured model, so the first candidate that resolves wins. For each
//! candidate, resolution is top-down:
//!   1. Profile `GatewayRoute.pricing` whose `model_pattern` matches (operator
//!      override, the strongest "we pay a custom rate here" signal).
//!   2. The matching `ProviderModel.pricing` in the services provider registry
//!      — the route provider's catalog entry, else any provider that serves it.
//!      The provider registry is the single source of model pricing.
//!
//! Missing pricing is an explicit error. Callers must not record an unknown
//! provider charge as a measured zero.
//!
//! The arithmetic itself is not here: `ModelPricing::cost_microdollars` in the
//! shared models crate is the one cost function, shared with the internal
//! agent path so both bill a `CanonicalUsage` identically.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::services::{GatewayConfig, ModelPricing, ProviderRegistry};

#[derive(Debug, thiserror::Error)]
#[error("No configured pricing for provider {provider} and models {models:?}")]
pub struct MissingPricing {
    pub provider: String,
    pub models: Vec<String>,
}

pub fn resolve(
    provider: &str,
    candidates: &[&str],
    gateway: Option<&GatewayConfig>,
    registry: &ProviderRegistry,
) -> Result<ModelPricing, MissingPricing> {
    for model in candidates.iter().filter(|m| !m.is_empty()) {
        if let Some(p) = lookup(model, gateway, registry) {
            return Ok(p);
        }
    }

    Err(MissingPricing {
        provider: provider.to_owned(),
        models: candidates.iter().map(|model| (*model).to_owned()).collect(),
    })
}

fn lookup(
    model: &str,
    gateway: Option<&GatewayConfig>,
    registry: &ProviderRegistry,
) -> Option<ModelPricing> {
    if let Some(gw) = gateway
        && let Some(route) = gw.find_route(model)
        && let Some(p) = route.pricing
    {
        return Some(p);
    }
    registry_pricing(registry, gateway, model)
}

fn registry_pricing(
    registry: &ProviderRegistry,
    gateway: Option<&GatewayConfig>,
    model: &str,
) -> Option<ModelPricing> {
    if let Some(route) = gateway.and_then(|gw| gw.find_route(model))
        && let Some(m) = route
            .resolve(registry)
            .and_then(|entry| entry.find_model(model))
    {
        return Some(m.pricing);
    }
    registry
        .providers
        .iter()
        .find_map(|entry| entry.find_model(model))
        .map(|m| m.pricing)
}

pub fn resolve_selected(
    route: &systemprompt_models::services::GatewayRoute,
    provider: &systemprompt_models::services::ProviderEntry,
    requested_model: &str,
) -> Result<ModelPricing, MissingPricing> {
    let model = route.upstream_model.as_deref().unwrap_or(requested_model);
    route
        .pricing
        .or_else(|| provider.find_model(model).map(|entry| entry.pricing))
        .ok_or_else(|| MissingPricing {
            provider: route.provider.to_string(),
            models: vec![model.to_owned()],
        })
}
