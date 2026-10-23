//! Models and deployments from replayed legacy dispatch.
//!
//! Every legacy model id that 0.72.1 would dispatch becomes one model, with
//! its declaring entry's upstream name, base price, capabilities, limits and
//! governance, advertised to its entry's client family (`surface`) unless it
//! was `hidden` or `backend` (`clients: []`). Its deployments are the
//! replayed chain, in order: each hop's endpoint, with an `upstream_model`
//! when the hop sent a different name and a price override when the hop's
//! price is not the base price. A hop 0.72.1 had no price for was never
//! served (dispatch skipped it), so it is not deployed. An endpoint on which
//! every hop is the base price times one ratio gets that ratio as its
//! `price_multiplier` instead of per-deployment overrides. An id 0.72.1 would
//! not dispatch (unexposed, unrouted, or unpriced at its primary) is dropped
//! and listed in the legacy map.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

use systemprompt_identifiers::ModelId;
use systemprompt_manifest::services::catalog::{
    DeploymentDetail, DeploymentRegistry, DeploymentTarget, EndpointRef, ModelEntry, ModelRegistry,
    PricingOverride, capability_flags,
};
use systemprompt_manifest::services::{ModelCapabilities, ModelPricing};
use systemprompt_models::providers::ClientFamily;

use super::fold::Folded;
use super::legacy::{LegacyModel, LegacyProvider, LegacySurface};
use super::replay::{LegacyHop, LegacyTree};

#[derive(Debug, Clone)]
struct Hop {
    endpoint: EndpointRef,
    upstream_model: String,
    pricing: ModelPricing,
}

#[derive(Debug, Default)]
pub struct ModelsOut {
    pub models: ModelRegistry,
    pub deployments: DeploymentRegistry,
    pub dropped: Vec<String>,
    pub multipliers: BTreeMap<EndpointRef, f64>,
}

const fn clients_of(surface: LegacySurface) -> Option<ClientFamily> {
    match surface {
        LegacySurface::Anthropic => Some(ClientFamily::Claude),
        LegacySurface::OpenAi => Some(ClientFamily::OpenAi),
        LegacySurface::Gemini => Some(ClientFamily::Gemini),
        LegacySurface::Backend => None,
    }
}

fn model_entry(entry: &LegacyProvider, model: &LegacyModel) -> ModelEntry {
    let defaults = ModelCapabilities::default();
    let mut out = ModelEntry::new(if model.hidden {
        Vec::new()
    } else {
        clients_of(entry.surface).into_iter().collect()
    });
    out.upstream_model.clone_from(&model.upstream_model);
    out.capabilities = capability_flags(model.capabilities)
        .into_iter()
        .zip(capability_flags(defaults))
        .filter(|((_, value), (_, default))| value != default)
        .map(|((capability, value), _)| (capability, value))
        .collect();
    out.limits = model.limits;
    out.pricing = model.pricing;
    out.governance = model.governance;
    out
}

fn hops(folded: &Folded, chain: &[LegacyHop]) -> Option<Vec<Hop>> {
    if chain
        .first()
        .is_none_or(|primary| primary.pricing.is_none())
    {
        return None;
    }
    let mut seen = BTreeSet::new();
    Some(
        chain
            .iter()
            .filter_map(|hop| {
                let pricing = hop.pricing?;
                let target = folded.entries.get(&hop.provider)?;
                seen.insert(target.endpoint.clone()).then(|| Hop {
                    endpoint: target.endpoint.clone(),
                    upstream_model: hop.upstream_model.clone(),
                    pricing,
                })
            })
            .collect(),
    )
}

pub fn build(tree: &LegacyTree, folded: &Folded) -> ModelsOut {
    let mut out = ModelsOut::default();
    let mut planned: BTreeMap<ModelId, (ModelEntry, Vec<Hop>)> = BTreeMap::new();
    for entry in &tree.providers {
        for model in &entry.models {
            let hops = tree.resolve(&model.id).and_then(|r| hops(folded, &r.chain));
            match hops {
                Some(hops) if !hops.is_empty() => {
                    planned.insert(ModelId::new(&model.id), (model_entry(entry, model), hops));
                },
                _ => out.dropped.push(model.id.clone()),
            }
        }
    }
    out.multipliers = multipliers(&planned);
    for (id, (entry, hops)) in planned {
        let upstream = entry.upstream_name(&id).to_owned();
        let targets = hops
            .into_iter()
            .map(|hop| {
                let multiplier = out.multipliers.get(&hop.endpoint).copied();
                let expected = PricingOverride::resolve(None, multiplier, entry.pricing);
                deployment(hop, &upstream, expected)
            })
            .collect();
        out.deployments.deployments.insert(id.clone(), targets);
        out.models.models.insert(id, entry);
    }
    out
}

fn deployment(hop: Hop, upstream: &str, expected: ModelPricing) -> DeploymentTarget {
    let pricing = (!pricing_close(&hop.pricing, &expected)).then_some(PricingOverride {
        input_per_million: Some(hop.pricing.input_per_million),
        output_per_million: Some(hop.pricing.output_per_million),
        cache_read_per_million: hop.pricing.cache_read_per_million,
        cache_write_per_million: hop.pricing.cache_write_per_million,
        per_image_cents: hop.pricing.per_image_cents,
    });
    let upstream_model = (hop.upstream_model != upstream).then_some(hop.upstream_model);
    if pricing.is_none() && upstream_model.is_none() {
        return DeploymentTarget::Endpoint(hop.endpoint);
    }
    DeploymentTarget::Detailed(DeploymentDetail {
        endpoint: hop.endpoint,
        upstream_model,
        pricing,
        clients: None,
    })
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
}

fn close_opt(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => close(a, b),
        (None, None) => true,
        _ => false,
    }
}

#[must_use]
pub fn pricing_close(a: &ModelPricing, b: &ModelPricing) -> bool {
    close(a.input_per_million, b.input_per_million)
        && close(a.output_per_million, b.output_per_million)
        && close_opt(a.cache_read_per_million, b.cache_read_per_million)
        && close_opt(a.cache_write_per_million, b.cache_write_per_million)
        && close_opt(a.per_image_cents, b.per_image_cents)
}

fn ratio(base: &ModelPricing, hop: &ModelPricing) -> Option<f64> {
    let pairs = [
        (Some(base.input_per_million), Some(hop.input_per_million)),
        (Some(base.output_per_million), Some(hop.output_per_million)),
        (base.cache_read_per_million, hop.cache_read_per_million),
        (base.cache_write_per_million, hop.cache_write_per_million),
        (base.per_image_cents, hop.per_image_cents),
    ];
    let mut found: Option<f64> = None;
    for pair in pairs {
        match pair {
            (None, None) => {},
            (Some(b), Some(h)) if b == 0.0 && h == 0.0 => {},
            (Some(b), Some(h)) if b > 0.0 => {
                let r = h / b;
                if found.is_some_and(|f| !close(f, r)) {
                    return None;
                }
                found = Some(r);
            },
            _ => return None,
        }
    }
    found
}

fn round_ratio(r: f64) -> f64 {
    (r * 1e6).round() / 1e6
}

fn multipliers(planned: &BTreeMap<ModelId, (ModelEntry, Vec<Hop>)>) -> BTreeMap<EndpointRef, f64> {
    let mut per_endpoint: BTreeMap<EndpointRef, Vec<Option<f64>>> = BTreeMap::new();
    for (entry, hops) in planned.values() {
        for hop in hops {
            per_endpoint
                .entry(hop.endpoint.clone())
                .or_default()
                .push(ratio(&entry.pricing, &hop.pricing));
        }
    }
    per_endpoint
        .into_iter()
        .filter_map(|(endpoint, ratios)| {
            let first = (*ratios.first()?)?;
            let uniform = ratios.iter().all(|r| r.is_some_and(|r| close(r, first)));
            let scaled = !close(first, 1.0);
            (uniform && scaled).then(|| (endpoint, round_ratio(first)))
        })
        .collect()
}
