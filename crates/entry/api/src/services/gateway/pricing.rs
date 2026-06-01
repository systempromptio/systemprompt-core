//! Pricing resolution for gateway requests.
//!
//! Resolution order, top-down:
//!   1. Profile `GatewayRoute.pricing` whose `model_pattern` matches the
//!      effective model (operator override, fastest signal of "we are paying a
//!      custom rate for this provider/route").
//!   2. The matching `ProviderModel.pricing` in `profile.providers` — the route
//!      provider's catalog entry for the model, else any provider that serves
//!      it.
//!   3. Static defaults baked into this module — list prices for the major
//!      providers (Anthropic, `OpenAI`, Google Gemini, `MiniMax`). These match
//!      the canonical published rates as of 2026-05.
//!   4. Fallback: emit an `unknown` WARN and return zero pricing. The WARN is
//!      the right signal — a model with no override and no default is a real
//!      gap, not noise to silence.

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

    if let Some(p) = lookup_default(provider, model) {
        return p;
    }

    tracing::warn!(
        provider = provider,
        model = model,
        "Gateway pricing lookup: no entry for (provider, model) — cost_microdollars will be 0"
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

fn lookup_default(provider: &str, model: &str) -> Option<ModelPricing> {
    let model_lc = model.to_ascii_lowercase();
    let m = model_lc.as_str();

    if provider.eq_ignore_ascii_case("anthropic") {
        return anthropic(m);
    }
    if provider.eq_ignore_ascii_case("openai") {
        return openai(m);
    }
    if provider.eq_ignore_ascii_case("google") || provider.eq_ignore_ascii_case("gemini") {
        return gemini(m);
    }
    if provider.eq_ignore_ascii_case("minimax") {
        return minimax(m);
    }
    None
}

const fn pricing(input: f64, output: f64) -> ModelPricing {
    ModelPricing {
        input_per_million: input,
        output_per_million: output,
        per_image_cents: None,
    }
}

fn anthropic(m: &str) -> Option<ModelPricing> {
    Some(match m {
        x if x.starts_with("claude-opus-4-7") || x.starts_with("claude-opus-4") => {
            pricing(15.0, 75.0)
        },
        x if x.starts_with("claude-sonnet-4") => pricing(3.0, 15.0),
        x if x.starts_with("claude-haiku-4") => pricing(1.0, 5.0),
        x if x.starts_with("claude-3-5-sonnet") => pricing(3.0, 15.0),
        x if x.starts_with("claude-3-5-haiku") => pricing(0.80, 4.0),
        x if x.starts_with("claude-3-opus") => pricing(15.0, 75.0),
        x if x.starts_with("claude-3-sonnet") => pricing(3.0, 15.0),
        x if x.starts_with("claude-3-haiku") => pricing(0.25, 1.25),
        _ => return None,
    })
}

fn openai(m: &str) -> Option<ModelPricing> {
    Some(match m {
        x if x.starts_with("gpt-4o-mini") => pricing(0.15, 0.60),
        x if x.starts_with("gpt-4o") => pricing(2.50, 10.0),
        x if x.starts_with("gpt-4-turbo") => pricing(10.0, 30.0),
        x if x.starts_with("gpt-4") => pricing(30.0, 60.0),
        x if x.starts_with("gpt-3.5-turbo") => pricing(0.50, 1.50),
        x if x.starts_with("o1-mini") => pricing(3.0, 12.0),
        x if x.starts_with("o1-preview") || x.starts_with("o1") => pricing(15.0, 60.0),
        x if x.starts_with("o3-mini") => pricing(1.10, 4.40),
        _ => return None,
    })
}

fn gemini(m: &str) -> Option<ModelPricing> {
    Some(match m {
        x if x.starts_with("gemini-2.0-flash") => pricing(0.10, 0.40),
        x if x.starts_with("gemini-1.5-flash") => pricing(0.075, 0.30),
        x if x.starts_with("gemini-1.5-pro") => pricing(1.25, 5.0),
        x if x.starts_with("gemini-1.0-pro") || x.starts_with("gemini-pro") => pricing(0.50, 1.50),
        _ => return None,
    })
}

fn minimax(m: &str) -> Option<ModelPricing> {
    Some(match m {
        x if x.starts_with("minimax-m2") => pricing(0.30, 1.20),
        x if x.starts_with("minimax-m1") || x == "abab7-chat-preview" => pricing(0.40, 2.20),
        x if x.starts_with("minimax-text-01") || x.starts_with("abab6.5") => pricing(0.20, 1.10),
        _ => return None,
    })
}

pub fn cost_microdollars(pricing: ModelPricing, input_tokens: u32, output_tokens: u32) -> i64 {
    let input = f64::from(input_tokens);
    let output = f64::from(output_tokens);
    let input_cost = (input / 1_000_000.0) * pricing.input_per_million;
    let output_cost = (output / 1_000_000.0) * pricing.output_per_million;
    ((input_cost + output_cost) * 1_000_000.0).round() as i64
}
