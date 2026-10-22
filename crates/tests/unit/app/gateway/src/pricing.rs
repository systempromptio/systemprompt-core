use systemprompt_catalog_fixtures::{CatalogFixture, model, platform};
use systemprompt_gateway::pricing::resolve;
use systemprompt_manifest::services::ModelPricing;
use systemprompt_manifest::services::catalog::{
    DeploymentDetail, DeploymentTarget, EndpointConfig, EndpointRef, PricingOverride,
    ProviderConfig, ResolvedCatalog,
};
use systemprompt_models::providers::ClientFamily;
use systemprompt_test_fixtures::usage;
use systemprompt_wire::WireProtocol;

fn price(input: f64, output: f64) -> ModelPricing {
    ModelPricing {
        input_per_million: input,
        output_per_million: output,
        cache_read_per_million: Some(0.0),
        ..ModelPricing::default()
    }
}

fn catalog() -> ResolvedCatalog {
    CatalogFixture::new()
        .credential("anthropic", platform("anthropic"))
        .credential("openai", platform("openai"))
        .provider("anthropic", ProviderConfig::new("anthropic"))
        .provider("openai", ProviderConfig::new("openai"))
        .endpoint(
            "anthropic",
            "api",
            EndpointConfig::new(WireProtocol::Anthropic, "https://api.anthropic.com/v1"),
        )
        .endpoint(
            "anthropic",
            "eu",
            EndpointConfig {
                price_multiplier: Some(1.1),
                ..EndpointConfig::new(WireProtocol::Anthropic, "https://eu.example.test/v1")
            },
        )
        .endpoint(
            "openai",
            "api",
            EndpointConfig::new(WireProtocol::OpenAiResponses, "https://api.openai.com/v1"),
        )
        .model(
            "claude-sonnet-4-rare",
            model(&[ClientFamily::Claude], 0, price(7.0, 9.0)),
        )
        .model(
            "gpt-5-mini",
            model(&[ClientFamily::OpenAi], 0, price(0.25, 2.0)),
        )
        .model(
            "claude-eu",
            model(&[ClientFamily::Claude], 0, price(10.0, 20.0)),
        )
        .deploy("claude-sonnet-4-rare", ["anthropic/api"])
        .deploy("gpt-5-mini", ["openai/api"])
        .deploy(
            "claude-eu",
            [
                DeploymentTarget::Endpoint(EndpointRef::new("anthropic", "eu")),
                DeploymentTarget::Detailed(DeploymentDetail {
                    endpoint: EndpointRef::new("openai", "api"),
                    upstream_model: Some("claude-eu-on-openai".to_owned()),
                    pricing: Some(PricingOverride {
                        input_per_million: Some(1.0),
                        ..PricingOverride::default()
                    }),
                    clients: None,
                }),
            ],
        )
        .build()
}

#[test]
fn model_base_price_is_used_for_a_plain_deployment() {
    let p = resolve("anthropic", &["claude-sonnet-4-rare"], &catalog()).unwrap();
    assert!((p.input_per_million - 7.0).abs() < f64::EPSILON);
    assert!((p.output_per_million - 9.0).abs() < f64::EPSILON);
}

#[test]
fn endpoint_multiplier_scales_the_base_price() {
    let p = resolve("anthropic", &["claude-eu"], &catalog()).unwrap();
    assert!((p.input_per_million - 11.0).abs() < 1e-9);
    assert!((p.output_per_million - 22.0).abs() < 1e-9);
}

#[test]
fn deployment_override_beats_the_multiplied_base_on_the_served_provider() {
    let p = resolve("openai", &["claude-eu"], &catalog()).unwrap();
    assert!((p.input_per_million - 1.0).abs() < 1e-9);
    assert!((p.output_per_million - 20.0).abs() < 1e-9);
}

#[test]
fn an_echoed_upstream_name_prices_its_deployment() {
    let p = resolve("openai", &["claude-eu-on-openai"], &catalog()).unwrap();
    assert!((p.input_per_million - 1.0).abs() < 1e-9);
}

#[test]
fn resolve_falls_back_to_configured_model_when_served_alias_unknown() {
    let p = resolve(
        "openai",
        &["gpt-5-mini-2025-08-07", "gpt-5-mini"],
        &catalog(),
    )
    .unwrap();
    assert!((p.input_per_million - 0.25).abs() < f64::EPSILON);
    assert!((p.output_per_million - 2.0).abs() < f64::EPSILON);
}

#[test]
fn resolve_reads_pricing_from_the_default_catalog() {
    let catalog = CatalogFixture::embedded_default().build();
    let p = resolve("anthropic", &["claude-haiku-4-5-20251001"], &catalog).unwrap();
    assert!((p.input_per_million - 1.0).abs() < 1e-9);
    assert!((p.output_per_million - 5.0).abs() < 1e-9);
}

#[test]
fn an_empty_catalog_is_missing_pricing() {
    let err = resolve(
        "anthropic",
        &["claude-3-haiku-20240307"],
        &CatalogFixture::new().build(),
    )
    .expect_err("an empty catalog prices nothing");
    assert_eq!(err.provider, "anthropic");
    assert_eq!(err.models, vec!["claude-3-haiku-20240307".to_owned()]);
}

#[test]
fn unknown_model_is_missing_pricing() {
    let err = resolve("anthropic", &["claude-99-mystery"], &catalog())
        .expect_err("an unpriced model must not be billed at zero");
    assert_eq!(err.models, vec!["claude-99-mystery".to_owned()]);
}

#[test]
fn cost_microdollars_uses_per_million_units() {
    let p = ModelPricing {
        input_per_million: 1.0,
        output_per_million: 2.0,
        ..ModelPricing::default()
    };
    // 1M input * $1 + 1M output * $2 = $3 = 3_000_000 microdollars.
    let u = usage().input(1_000_000).output(1_000_000).build();
    assert_eq!(p.cost_microdollars(&u), 3_000_000);
}

#[test]
fn cost_microdollars_zero_for_zero_tokens() {
    let p = ModelPricing {
        input_per_million: 5.0,
        output_per_million: 5.0,
        ..ModelPricing::default()
    };
    assert_eq!(p.cost_microdollars(&usage().build()), 0);
}

#[test]
fn cost_microdollars_rounds_to_nearest() {
    // 1 input @ $1/1M = $1e-6 → 1 microdollar.
    let p = ModelPricing {
        input_per_million: 1.0,
        output_per_million: 0.0,
        ..ModelPricing::default()
    };
    let input_only = |n| usage().input(n).build();
    assert_eq!(p.cost_microdollars(&input_only(1)), 1);
    assert_eq!(p.cost_microdollars(&input_only(500_000)), 500_000);
}

#[test]
fn cost_microdollars_prices_cache_tokens_at_their_own_rates() {
    let p = ModelPricing {
        input_per_million: 5.0,
        output_per_million: 25.0,
        cache_read_per_million: Some(0.5),
        cache_write_per_million: Some(6.25),
        per_image_cents: None,
    };
    let tokens = usage()
        .input(1_000)
        .output(2_000)
        .cache_read(100_000)
        .cache_creation(10_000)
        .build();
    // Per million: 1k*$5 + 2k*$25 + 100k*$0.50 + 10k*$6.25, in microdollars.
    let expected = 5_000 + 50_000 + 50_000 + 62_500;
    assert_eq!(p.cost_microdollars(&tokens), expected);

    let without_cache = p.cost_microdollars(&usage().input(1_000).output(2_000).build());
    assert_eq!(without_cache, 55_000);
    assert!(
        without_cache * 2 < expected,
        "cache tokens carry most of this bill: {without_cache} vs {expected}"
    );
}

#[test]
fn is_billable_rejects_zeroed_token_rates_but_accepts_image_pricing() {
    assert!(!ModelPricing::default().is_billable());
    assert!(
        !ModelPricing {
            input_per_million: 5.0,
            ..ModelPricing::default()
        }
        .is_billable(),
        "an output rate of zero still bills half the request at zero"
    );
    assert!(
        ModelPricing {
            per_image_cents: Some(4.0),
            ..ModelPricing::default()
        }
        .is_billable()
    );
}

#[test]
fn a_reasoning_only_gemini_turn_is_billed_for_its_thinking() {
    // The live defect this pins: max_tokens 200 on gemini-2.5-flash, the whole
    // budget spent thinking, six tokens of visible answer. Before the wire
    // folded thoughtsTokenCount into output_tokens the turn billed for six.
    let value = serde_json::json!({
        "candidates": [{
            "content": {"role": "model", "parts": [{"text": "ok"}]},
            "finishReason": "MAX_TOKENS"
        }],
        "usageMetadata": {
            "promptTokenCount": 27,
            "candidatesTokenCount": 6,
            "thoughtsTokenCount": 194,
            "totalTokenCount": 227
        }
    });
    let parsed = systemprompt_wire::gemini::parse_response(&value, "gemini-2.5-flash")
        .expect("fixture parses")
        .usage;
    let p = ModelPricing {
        input_per_million: 1.0,
        output_per_million: 10.0,
        ..ModelPricing::default()
    };
    // 27 input @ $1/M = 27 microdollars; 200 output (6 visible + 194 thinking)
    // @ $10/M = 2000 microdollars.
    assert_eq!(p.cost_microdollars(&parsed), 2_027);
    assert_eq!(parsed.reasoning_tokens, 194);
}

#[test]
fn reasoning_tokens_are_never_added_to_cost_a_second_time() {
    // OpenAI already counts reasoning inside completion_tokens, so the same
    // pricing arithmetic must charge 106 output tokens, not 206.
    let value = serde_json::json!({
        "id": "resp_r",
        "model": "o4-mini",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "42"}}],
        "usage": {
            "prompt_tokens": 0,
            "completion_tokens": 106,
            "completion_tokens_details": {"reasoning_tokens": 100}
        }
    });
    let parsed = systemprompt_wire::openai_chat::parse_response(&value, "o4-mini")
        .expect("fixture parses")
        .usage;
    let p = ModelPricing {
        output_per_million: 1_000_000.0,
        ..ModelPricing::default()
    };
    assert_eq!(parsed.reasoning_tokens, 100);
    assert_eq!(
        p.cost_microdollars(&usage().output(parsed.output_tokens).build()),
        106_000_000
    );
}

#[test]
fn a_cached_openai_turn_bills_the_cached_slice_once_at_the_cache_rate() {
    let value = serde_json::json!({
        "id": "resp_c",
        "model": "gpt-4.1-mini",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "hi"}}],
        "usage": {
            "prompt_tokens": 1_000,
            "completion_tokens": 0,
            "prompt_tokens_details": {"cached_tokens": 800}
        }
    });
    let parsed = systemprompt_wire::openai_chat::parse_response(&value, "gpt-4.1-mini")
        .expect("fixture parses")
        .usage;
    assert_eq!(
        parsed.input_tokens + parsed.cache_read_tokens,
        1_000,
        "input must be exclusive of the cached slice"
    );

    let p = ModelPricing {
        input_per_million: 10.0,
        cache_read_per_million: Some(1.0),
        ..ModelPricing::default()
    };
    // 200 uncached @ $10/M = 2000; 800 cached @ $1/M = 800.
    assert_eq!(p.cost_microdollars(&parsed), 2_800);

    let double_billed = p.cost_microdollars(
        &usage()
            .input(1_000)
            .cache_read(parsed.cache_read_tokens)
            .build(),
    );
    assert!(
        double_billed > p.cost_microdollars(&parsed),
        "the disjoint mapping over-charges: {double_billed} vs 2800"
    );
}

#[test]
fn an_unknown_provider_has_no_measured_price() {
    assert!(
        resolve(
            "some-new-provider",
            &["mystery-model"],
            &CatalogFixture::new().build()
        )
        .is_err()
    );
}

#[test]
fn tokens_used_is_billable_total_on_every_path() {
    let u = usage()
        .input(10)
        .output(20)
        .cache_read(30)
        .cache_creation(40)
        .reasoning(15)
        .build();
    assert_eq!(u.billable_total(), 100);
}
