//! Pricing resolution for gateway requests.
//!
//! A dispatched hop carries its final price already: the catalog resolved it
//! with the precedence a deployment's `pricing` override, then the endpoint's
//! `price_multiplier` applied to the model's base price, then the model's
//! base price ([`resolve_hop`]). With the gateway enabled the catalog refuses
//! to load a deployment that is unpriced, so a dispatch never bills a guessed
//! zero.
//!
//! [`resolve`] prices a completion that carries no pinned price, by model id
//! or upstream name: `candidates` is tried in order — the provider-echoed
//! served model first, then the requested model — and a hop on the served
//! provider wins over another provider's deployment of the same model.
//! Missing pricing is an explicit error; callers must not record an unknown
//! charge as a measured zero.
//!
//! The arithmetic itself is not here: `ModelPricing::cost_microdollars` in the
//! shared models crate is the one cost function, shared with the internal
//! agent path so both bill a `CanonicalUsage` identically.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_manifest::services::ModelPricing;
use systemprompt_manifest::services::catalog::{ResolvedCatalog, ResolvedHop};

#[derive(Debug, thiserror::Error)]
#[error("No configured pricing for provider {provider} and models {models:?}")]
pub struct MissingPricing {
    pub provider: String,
    pub models: Vec<String>,
}

#[must_use]
pub const fn resolve_hop(hop: &ResolvedHop) -> ModelPricing {
    hop.pricing
}

pub fn resolve(
    provider: &str,
    candidates: &[&str],
    catalog: &ResolvedCatalog,
) -> Result<ModelPricing, MissingPricing> {
    for model in candidates.iter().filter(|m| !m.is_empty()) {
        if let Some(pricing) = lookup(catalog, provider, model) {
            return Ok(pricing);
        }
    }
    Err(MissingPricing {
        provider: provider.to_owned(),
        models: candidates.iter().map(|model| (*model).to_owned()).collect(),
    })
}

fn lookup(catalog: &ResolvedCatalog, provider: &str, model: &str) -> Option<ModelPricing> {
    let names = |hop: &&ResolvedHop| hop.model.as_str() == model || hop.upstream_model == model;
    let hops: Vec<&ResolvedHop> = catalog
        .models()
        .flat_map(|m| catalog.deployments(m.id.as_str()))
        .filter(names)
        .collect();
    hops.iter()
        .find(|hop| hop.provider.as_str() == provider)
        .or_else(|| hops.first())
        .map(|hop| hop.pricing)
}
