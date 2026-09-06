//! Regression lock for token-cost math against the seeded catalog pricing.
//!
//! Production cost accounting (`AiService::estimate_cost`,
//! `StreamStorageWrapper`) and the gateway share one conversion —
//! `ModelPricing::cost_microdollars` over a `CanonicalUsage` — with the
//! per-million rates resolved from the provider catalog via
//! [`catalog_pricing`]. These tests pin both the rates and that conversion so a
//! drift in either is caught.

use systemprompt_ai::services::providers::catalog_pricing;
use systemprompt_models::services::{ProviderModel, ProviderRegistry};
use systemprompt_test_fixtures::usage;

fn seed_models(provider: &str) -> Vec<ProviderModel> {
    ProviderRegistry::default_seed()
        .expect("embedded default catalog parses")
        .find_provider(provider)
        .unwrap_or_else(|| panic!("provider '{provider}' present in seed"))
        .models
        .clone()
}

fn microdollars(models: &[ProviderModel], model: &str, input: u32, output: u32) -> i64 {
    catalog_pricing(models, model).cost_microdollars(&usage().input(input).output(output).build())
}

#[test]
fn anthropic_haiku_cost_is_exact() {
    let models = seed_models("anthropic");
    assert_eq!(
        microdollars(&models, "claude-haiku-4-5-20251001", 1_000, 500),
        3_500,
        "1k in @ 1.0/M + 0.5k out @ 5.0/M = 1000 + 2500 microdollars"
    );
}

#[test]
fn anthropic_sonnet_cost_is_exact() {
    let models = seed_models("anthropic");
    assert_eq!(
        microdollars(&models, "claude-sonnet-4-6", 1_000_000, 1_000_000),
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

#[test]
fn cache_tokens_are_billed_by_the_shared_cost_function() {
    let models = seed_models("anthropic");
    let pricing = catalog_pricing(&models, "claude-sonnet-4-6");
    let cached = usage()
        .input(1_000)
        .output(500)
        .cache_read(100_000)
        .cache_creation(10_000)
        .build();
    assert_eq!(cached.billable_total(), 111_500);
    assert!(
        pricing.cost_microdollars(&cached)
            > pricing.cost_microdollars(&usage().input(1_000).output(500).build()),
        "cache tokens must be billed, not dropped"
    );
}
