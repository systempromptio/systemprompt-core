//! Regression lock for token-cost math against the seeded catalog pricing.
//!
//! Production cost accounting (`AiService::estimate_cost`,
//! `StreamStorageWrapper`) converts usage to microdollars as
//! `round(input * input_per_million + output * output_per_million)`, with the
//! per-million rates resolved from the provider catalog via
//! [`catalog_pricing`]. These tests pin both the rates and that conversion so a
//! drift in either is caught.

use systemprompt_ai::services::providers::catalog_pricing;
use systemprompt_models::profile::{ProviderModel, ProviderRegistry};

fn seed_models(provider: &str) -> Vec<ProviderModel> {
    ProviderRegistry::default_seed()
        .expect("embedded default catalog parses")
        .find_provider(provider)
        .unwrap_or_else(|| panic!("provider '{provider}' present in seed"))
        .models
        .clone()
}

fn microdollars(models: &[ProviderModel], model: &str, input: u32, output: u32) -> i64 {
    let pricing = catalog_pricing(models, model);
    let input_cost = (f64::from(input) / 1_000_000.0) * pricing.input_per_million;
    let output_cost = (f64::from(output) / 1_000_000.0) * pricing.output_per_million;
    ((input_cost + output_cost) * 1_000_000.0).round() as i64
}

#[test]
fn anthropic_haiku_cost_is_exact() {
    let models = seed_models("anthropic");
    assert_eq!(
        microdollars(&models, "claude-3-haiku-20240307", 1_000, 500),
        875,
        "1k in @ 0.25/M + 0.5k out @ 1.25/M = 250 + 625 microdollars"
    );
}

#[test]
fn anthropic_sonnet_cost_is_exact() {
    let models = seed_models("anthropic");
    assert_eq!(
        microdollars(&models, "claude-sonnet-4-6-20250610", 1_000_000, 1_000_000),
        18_000_000,
        "1M in @ 3/M + 1M out @ 15/M = 3 + 15 dollars"
    );
}

#[test]
fn openai_gpt4o_mini_cost_is_exact() {
    let models = seed_models("openai");
    assert_eq!(
        microdollars(&models, "gpt-4o-mini", 10_000, 2_000),
        2_700,
        "10k in @ 0.15/M + 2k out @ 0.6/M = 1500 + 1200 microdollars"
    );
}

#[test]
fn gemini_flash_cost_is_exact() {
    let models = seed_models("gemini");
    assert_eq!(
        microdollars(&models, "gemini-2.0-flash", 1_000_000, 500_000),
        300_000,
        "1M in @ 0.1/M + 0.5M out @ 0.4/M = 100k + 200k microdollars"
    );
}

#[test]
fn unknown_model_costs_zero() {
    let models = seed_models("openai");
    assert_eq!(
        microdollars(&models, "no-such-model", 1_000, 1_000),
        0,
        "an unknown model resolves to default (zero) pricing"
    );
}
