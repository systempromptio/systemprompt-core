//! Regression lock for token-cost math against the seeded catalog pricing.
//!
//! Production cost accounting (`AiService::estimate_cost`,
//! `StreamStorageWrapper`) and the gateway share one conversion —
//! `ModelPricing::cost_microdollars` over a `CanonicalUsage` — with the
//! per-million rates resolved from the provider catalog via
//! [`catalog_pricing`]. These tests pin both the rates and that conversion so a
//! drift in either is caught.

use systemprompt_ai::services::providers::{ServedModel, catalog_pricing};

use crate::services::providers::mock_http::seed_models;
use systemprompt_test_fixtures::usage;


fn microdollars(models: &[ServedModel], model: &str, input: u32, output: u32) -> i64 {
    catalog_pricing(models, model)
        .expect("model is priced")
        .cost_microdollars(&usage().input(input).output(output).build())
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
        microdollars(&models, "claude-sonnet-5", 1_000_000, 1_000_000),
        12_000_000,
        "1M in @ 2/M + 1M out @ 10/M = 2 + 10 dollars"
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
fn cache_tokens_are_billed_by_the_shared_cost_function() {
    let models = seed_models("anthropic");
    let pricing = catalog_pricing(&models, "claude-sonnet-5").expect("model is priced");
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

// Why: a model the catalogue does not price is withheld rather than priced at
// zero, so a request for it can be refused instead of settled unbilled.
#[test]
fn an_unpriced_model_has_no_pricing() {
    let models = seed_models("anthropic");
    assert!(catalog_pricing(&models, "claude-not-in-catalogue").is_none());
}

// Why: a provider client prices from its endpoint's deployments, so the
// endpoint's price multiplier and a deployment's pricing override reach cost
// accounting without being re-derived from the model entry.
#[test]
fn the_endpoint_multiplier_and_deployment_overrides_reach_client_pricing() {
    use systemprompt_ai::services::providers::served_models_on;
    use systemprompt_manifest::services::catalog::EndpointRef;
    use systemprompt_test_fixtures::catalog::vertex_regions;

    let catalog = vertex_regions().build();
    let global = served_models_on(&catalog, &EndpointRef::new("vertex", "claude-global"));
    let eu = served_models_on(&catalog, &EndpointRef::new("vertex", "claude-eu"));
    let gemini = served_models_on(&catalog, &EndpointRef::new("vertex", "gemini"));

    let base = catalog_pricing(&global, "claude-sonnet-5").expect("priced");
    let premium = catalog_pricing(&eu, "claude-sonnet-5").expect("priced");
    assert!((base.input_per_million - 3.0).abs() < 1e-9);
    assert!((premium.input_per_million - 3.0 * 1.10).abs() < 1e-9);

    let flash = catalog_pricing(&gemini, "vertex-gemini-3.8-flash").expect("priced");
    assert_eq!(flash.cache_write_per_million, Some(0.0));
    assert!(
        catalog_pricing(&gemini, "gemini-3.8-flash").is_some(),
        "the upstream name finds the same deployment"
    );
}
