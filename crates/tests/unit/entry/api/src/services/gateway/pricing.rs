use systemprompt_api::services::gateway::pricing::{cost_microdollars, resolve};
use systemprompt_identifiers::{ModelId, ProviderId, RouteId, SecretName};
use systemprompt_models::profile::{
    ApiSurface, GatewayConfig, GatewayRoute, ProviderEntry, ProviderModel, ProviderRegistry,
    WireProtocol,
};
use systemprompt_models::services::ModelPricing;

fn route(pattern: &str, provider: &str, pricing: Option<ModelPricing>) -> GatewayRoute {
    GatewayRoute {
        id: RouteId::new(format!("{pattern}-{provider}")),
        model_pattern: pattern.to_owned(),
        provider: ProviderId::new(provider),
        upstream_model: None,
        extra_headers: Default::default(),
        pricing,
    }
}

fn gateway_with(routes: Vec<GatewayRoute>) -> GatewayConfig {
    GatewayConfig {
        enabled: true,
        routes,
        default_provider: None,
        allow_unlisted_models: false,
        auth_scheme: "bearer".to_owned(),
        inference_path_prefix: "/v1".to_owned(),
    }
}

#[test]
fn route_pricing_takes_precedence() {
    let custom = ModelPricing {
        input_per_million: 1.0,
        output_per_million: 2.0,
        per_image_cents: None,
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
        per_image_cents: None,
    };
    let registry = ProviderRegistry {
        providers: vec![ProviderEntry {
            name: ProviderId::new("anthropic"),
            wire: WireProtocol::Anthropic,
            surface: ApiSurface::Anthropic,
            endpoint: "https://api.anthropic.com".to_owned(),
            api_key_secret: SecretName::new("anthropic"),
            extra_headers: Default::default(),
            models: vec![ProviderModel {
                id: ModelId::new("claude-sonnet-4-rare"),
                aliases: Vec::new(),
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
        per_image_cents: None,
    };
    let registry = ProviderRegistry {
        providers: vec![ProviderEntry {
            name: ProviderId::new("openai"),
            wire: WireProtocol::OpenAiResponses,
            surface: ApiSurface::OpenAi,
            endpoint: "https://api.openai.com/v1".to_owned(),
            api_key_secret: SecretName::new("openai"),
            extra_headers: Default::default(),
            models: vec![ProviderModel {
                id: ModelId::new("gpt-5-mini"),
                aliases: Vec::new(),
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
        per_image_cents: None,
    };
    // 1M input * $1 + 1M output * $2 = $3 = 3_000_000 microdollars.
    assert_eq!(cost_microdollars(p, 1_000_000, 1_000_000), 3_000_000);
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
        per_image_cents: None,
    };
    assert_eq!(cost_microdollars(p, 0, 0), 0);
}

#[test]
fn cost_microdollars_rounds_to_nearest() {
    // 1 input @ $1/1M = $1e-6 → 1 microdollar.
    let p = ModelPricing {
        input_per_million: 1.0,
        output_per_million: 0.0,
        per_image_cents: None,
    };
    assert_eq!(cost_microdollars(p, 1, 0), 1);
    assert_eq!(cost_microdollars(p, 500_000, 0), 500_000);
}
