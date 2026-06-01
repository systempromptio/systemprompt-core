//! Pricing resolution for gateway requests.
//!
//! Resolution order, top-down:
//!   1. Profile `GatewayRoute.pricing` whose `model_pattern` matches the
//!      effective model (operator override, fastest signal of "we are paying a
//!      custom rate for this provider/route").
//!   2. The matching `ProviderModel.pricing` in `profile.providers` — the route
//!      provider's catalog entry for the model, else any provider that serves
//!      it. The provider registry is the single source of model pricing.
//!   3. Fallback: emit an `unknown` WARN and return zero pricing. The WARN is
//!      the right signal — a model with no override and no registry entry is a
//!      real configuration gap, not noise to silence.

use systemprompt_models::profile::{GatewayConfig, ProviderRegistry};
use systemprompt_models::services::ModelPricing;

pub fn resolve(
    provider: &str,
    model: &str,
    gateway: Option<&GatewayConfig>,
    registry: &ProviderRegistry,
) -> ModelPricing {
    if let Some(gw) = gateway {
        if let Some(route) = gw.find_route(model) {
            if let Some(p) = route.pricing {
                return p;
            }
        }
    }

    if let Some(p) = registry_pricing(registry, gateway, model) {
        return p;
    }

    tracing::warn!(
        provider = provider,
        model = model,
        "Gateway pricing lookup: no override and no registry entry — cost_microdollars will be 0"
    );
    ModelPricing::default()
}

// Prefer the route provider's own catalog entry for the model so a per-provider
// rate wins; otherwise accept any provider that serves the model id/alias.
fn registry_pricing(
    registry: &ProviderRegistry,
    gateway: Option<&GatewayConfig>,
    model: &str,
) -> Option<ModelPricing> {
    if let Some(route) = gateway.and_then(|gw| gw.find_route(model)) {
        if let Some(m) = route
            .resolve(registry)
            .and_then(|entry| entry.find_model(model))
        {
            return Some(m.pricing);
        }
    }
    registry
        .providers
        .iter()
        .find_map(|entry| entry.find_model(model))
        .map(|m| m.pricing)
}

pub fn cost_microdollars(pricing: ModelPricing, input_tokens: u32, output_tokens: u32) -> i64 {
    let input = f64::from(input_tokens);
    let output = f64::from(output_tokens);
    let input_cost = (input / 1_000_000.0) * pricing.input_per_million;
    let output_cost = (output / 1_000_000.0) * pricing.output_per_million;
    ((input_cost + output_cost) * 1_000_000.0).round() as i64
}
