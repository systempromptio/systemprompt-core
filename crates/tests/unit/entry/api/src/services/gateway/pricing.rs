use systemprompt_api::services::gateway::pricing::resolve;
use systemprompt_test_fixtures::usage;
use systemprompt_identifiers::{ModelId, ProviderId, RouteId, SecretName};
use systemprompt_models::services::{
    ApiSurface, GatewayConfig, GatewayRoute, ModelPricing, ProviderEntry, ProviderModel,
    ProviderRegistry, WireProtocol,
};

fn route(pattern: &str, provider: &str, pricing: Option<ModelPricing>) -> GatewayRoute {
    GatewayRoute {
        id: RouteId::new(format!("{pattern}-{provider}")),
        model_pattern: pattern.to_owned(),
        provider: ProviderId::new(provider),
        upstream_model: None,
        extra_headers: Default::default(),
        pricing,
        when: None,
        requires: None,
    }
}

fn gateway_with(routes: Vec<GatewayRoute>) -> GatewayConfig {
    GatewayConfig {
        enabled: true,
        routes,
        default_provider: None,
        default_model: None,
        allow_unlisted_models: false,
        auth_scheme: "bearer".to_owned(),
        inference_path_prefix: "/v1".to_owned(),
        system_prompt_overrides: Vec::new(),
        bridge_releases: None,
    }
}

#[test]
fn route_pricing_takes_precedence() {
    let custom = ModelPricing {
        input_per_million: 1.0,
        output_per_million: 2.0,
        ..ModelPricing::default()
    };
    let gw = gateway_with(vec![route("claude-opus-4-7*", "anthropic", Some(custom))]);
    let registry = ProviderRegistry::default();
    let p = resolve(
        "anthropic",
        &["claude-opus-4-7-something"],
        Some(&gw),
        &registry,
    );
    assert!((p.input_per_million - 1.0).abs() < f64::EPSILON);
    assert!((p.output_per_million - 2.0).abs() < f64::EPSILON);
}

#[test]
fn registry_pricing_used_when_no_route_override() {
    let custom = ModelPricing {
        input_per_million: 7.0,
        output_per_million: 9.0,
        ..ModelPricing::default()
    };
    let registry = ProviderRegistry {
        providers: vec![ProviderEntry {
            name: ProviderId::new("anthropic"),
            wire: WireProtocol::Anthropic,
            surface: ApiSurface::Anthropic,
            endpoint: "https://api.anthropic.com".to_owned(),
            api_key_secret: SecretName::new("anthropic"),
            governance: Default::default(),
            extra_headers: Default::default(),
            models: vec![ProviderModel {
                id: ModelId::new("claude-sonnet-4-rare"),
                aliases: Vec::new(),
                governance: None,
                upstream_model: None,
                pricing: custom,
                capabilities: Default::default(),
                limits: Default::default(),
            }],
        }],
    };
    let gw = gateway_with(vec![]);
    let p = resolve("anthropic", &["claude-sonnet-4-rare"], Some(&gw), &registry);
    assert!((p.input_per_million - 7.0).abs() < f64::EPSILON);
    assert!((p.output_per_million - 9.0).abs() < f64::EPSILON);
}

#[test]
fn resolve_falls_back_to_configured_model_when_served_alias_unknown() {
    let custom = ModelPricing {
        input_per_million: 0.25,
        output_per_million: 2.0,
        ..ModelPricing::default()
    };
    let registry = ProviderRegistry {
        providers: vec![ProviderEntry {
            name: ProviderId::new("openai"),
            wire: WireProtocol::OpenAiResponses,
            surface: ApiSurface::OpenAi,
            endpoint: "https://api.openai.com/v1".to_owned(),
            api_key_secret: SecretName::new("openai"),
            governance: Default::default(),
            extra_headers: Default::default(),
            models: vec![ProviderModel {
                id: ModelId::new("gpt-5-mini"),
                aliases: Vec::new(),
                governance: None,
                upstream_model: None,
                pricing: custom,
                capabilities: Default::default(),
                limits: Default::default(),
            }],
        }],
    };
    // First candidate is the dated alias the provider echoes (no catalog entry);
    // the configured upstream model resolves.
    let p = resolve(
        "openai",
        &["gpt-5-mini-2025-08-07", "gpt-5-mini"],
        None,
        &registry,
    );
    assert!((p.input_per_million - 0.25).abs() < f64::EPSILON);
    assert!((p.output_per_million - 2.0).abs() < f64::EPSILON);
}

#[test]
fn resolve_reads_pricing_from_seeded_registry() {
    let registry = ProviderRegistry::default_seed().expect("embedded default catalog parses");
    let p = resolve("anthropic", &["claude-haiku-4-5-20251001"], None, &registry);
    assert!((p.input_per_million - 1.0).abs() < 1e-9);
    assert!((p.output_per_million - 5.0).abs() < 1e-9);
}

#[test]
fn empty_registry_and_no_route_returns_zero() {
    let p = resolve(
        "anthropic",
        &["claude-3-haiku-20240307"],
        None,
        &ProviderRegistry::default(),
    );
    assert_eq!(p.input_per_million, 0.0);
    assert_eq!(p.output_per_million, 0.0);
}

#[test]
fn unknown_provider_returns_zero() {
    let p = resolve(
        "never-heard-of-it",
        &["wat"],
        None,
        &ProviderRegistry::default(),
    );
    assert!((p.input_per_million - 0.0).abs() < f64::EPSILON);
    assert!((p.output_per_million - 0.0).abs() < f64::EPSILON);
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
fn unknown_model_in_known_provider_returns_zero() {
    let p = resolve(
        "anthropic",
        &["claude-99-mystery"],
        None,
        &ProviderRegistry::default(),
    );
    assert_eq!(p.input_per_million, 0.0);
    assert_eq!(p.output_per_million, 0.0);
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

/// The Claude Code shape: a large cached system prompt means cache reads
/// dominate the token mix, so pricing on `input + output` alone reports a
/// fraction of the real bill.
#[test]
fn cost_microdollars_prices_cache_tokens_at_their_own_rates() {
    let p = ModelPricing {
        input_per_million: 5.0,
        output_per_million: 25.0,
        cache_read_per_million: 0.5,
        cache_write_per_million: 6.25,
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
    let parsed =
        systemprompt_models::wire::gemini::parse_response(&value, "gemini-2.5-flash").usage;
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
    let parsed = systemprompt_models::wire::openai_chat::parse_response(&value, "o4-mini").usage;
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

/// The double-billing defect: `OpenAI` reports `cached_tokens` as a subset of
/// `prompt_tokens`, so a canonical usage whose `input_tokens` still contains
/// the cached slice charges it at the input rate and again at the cache rate.
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
    let parsed =
        systemprompt_models::wire::openai_chat::parse_response(&value, "gpt-4.1-mini").usage;
    assert_eq!(
        parsed.input_tokens + parsed.cache_read_tokens,
        1_000,
        "input must be exclusive of the cached slice"
    );

    let p = ModelPricing {
        input_per_million: 10.0,
        cache_read_per_million: 1.0,
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
fn an_unknown_provider_costs_nothing_rather_than_a_fabricated_rate() {
    let p = resolve(
        "some-new-provider",
        &["mystery-model"],
        None,
        &ProviderRegistry::default(),
    );
    let u = usage().input(1_000_000).output(1_000_000).build();
    assert_eq!(
        p.cost_microdollars(&u),
        0,
        "an unpriced provider must bill zero, never an invented $1/$1 rate"
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
