use systemprompt_api::services::gateway::pricing::{cost_microdollars, resolve};
use systemprompt_identifiers::{ModelId, ProviderId, RouteId, SecretName};
use systemprompt_models::profile::{
    GatewayCatalog, GatewayConfig, GatewayModel, GatewayProvider, GatewayRoute,
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

fn gateway_with(routes: Vec<GatewayRoute>, catalog: Option<GatewayCatalog>) -> GatewayConfig {
    GatewayConfig {
        enabled: true,
        routes,
        catalog,
        default_provider: None,
        auth_scheme: "bearer".to_owned(),
        inference_path_prefix: "/v1".to_owned(),
    }
}

#[test]
fn route_pricing_overrides_static_default() {
    let custom = ModelPricing {
        input_per_million: 1.0,
        output_per_million: 2.0,
        per_image_cents: None,
    };
    let gw = gateway_with(
        vec![route("claude-opus-4-7*", "anthropic", Some(custom))],
        None,
    );
    let p = resolve("anthropic", "claude-opus-4-7-something", Some(&gw));
    assert!((p.input_per_million - 1.0).abs() < f64::EPSILON);
    assert!((p.output_per_million - 2.0).abs() < f64::EPSILON);
}

#[test]
fn catalog_pricing_used_when_no_route_override() {
    let custom = ModelPricing {
        input_per_million: 7.0,
        output_per_million: 9.0,
        per_image_cents: None,
    };
    let catalog = GatewayCatalog {
        providers: vec![GatewayProvider {
            name: ProviderId::new("anthropic"),
            endpoint: "https://api.anthropic.com".to_owned(),
            api_key_secret: SecretName::new("anthropic"),
            extra_headers: Default::default(),
        }],
        models: vec![GatewayModel {
            id: ModelId::new("claude-sonnet-4-rare"),
            provider: ProviderId::new("anthropic"),
            aliases: Vec::new(),
            display_name: None,
            upstream_model: None,
            pricing: Some(custom),
        }],
    };
    let gw = gateway_with(vec![], Some(catalog));
    let p = resolve("anthropic", "claude-sonnet-4-rare", Some(&gw));
    assert!((p.input_per_million - 7.0).abs() < f64::EPSILON);
    assert!((p.output_per_million - 9.0).abs() < f64::EPSILON);
}

#[test]
fn falls_back_to_static_default_for_minimax_m2() {
    let p = resolve("minimax", "MiniMax-M2", None);
    assert!((p.input_per_million - 0.30).abs() < f64::EPSILON);
    assert!((p.output_per_million - 1.20).abs() < f64::EPSILON);
}

#[test]
fn unknown_provider_returns_zero() {
    let p = resolve("never-heard-of-it", "wat", None);
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
fn anthropic_static_defaults_full_matrix() {
    let cases = [
        ("anthropic", "claude-opus-4-7-2026", 15.0, 75.0),
        ("anthropic", "claude-opus-4", 15.0, 75.0),
        ("anthropic", "claude-sonnet-4-foo", 3.0, 15.0),
        ("anthropic", "claude-haiku-4", 1.0, 5.0),
        ("anthropic", "claude-3-5-sonnet-anything", 3.0, 15.0),
        ("anthropic", "claude-3-5-haiku-x", 0.80, 4.0),
        ("anthropic", "claude-3-opus-y", 15.0, 75.0),
        ("anthropic", "claude-3-sonnet-z", 3.0, 15.0),
        ("anthropic", "claude-3-haiku-w", 0.25, 1.25),
    ];
    for (provider, model, inp, out) in cases {
        let p = resolve(provider, model, None);
        assert!(
            (p.input_per_million - inp).abs() < 1e-9,
            "{model}: got {} want {inp}",
            p.input_per_million
        );
        assert!(
            (p.output_per_million - out).abs() < 1e-9,
            "{model}: got {} want {out}",
            p.output_per_million
        );
    }
}

#[test]
fn openai_static_defaults_full_matrix() {
    let cases = [
        ("openai", "gpt-4o-mini-2024", 0.15, 0.60),
        ("openai", "gpt-4o-2024", 2.50, 10.0),
        ("openai", "gpt-4-turbo-x", 10.0, 30.0),
        ("openai", "gpt-4-base", 30.0, 60.0),
        ("openai", "gpt-3.5-turbo-instr", 0.50, 1.50),
        ("openai", "o1-mini-2024", 3.0, 12.0),
        ("openai", "o1-preview-2024", 15.0, 60.0),
        ("openai", "o1-something", 15.0, 60.0),
        ("openai", "o3-mini-2025", 1.10, 4.40),
    ];
    for (provider, model, inp, out) in cases {
        let p = resolve(provider, model, None);
        assert!((p.input_per_million - inp).abs() < 1e-9, "{model}");
        assert!((p.output_per_million - out).abs() < 1e-9, "{model}");
    }
}

#[test]
fn gemini_static_defaults() {
    let cases = [
        ("google", "gemini-2.0-flash-001", 0.10, 0.40),
        ("gemini", "gemini-1.5-flash", 0.075, 0.30),
        ("google", "gemini-1.5-pro", 1.25, 5.0),
        ("google", "gemini-1.0-pro", 0.50, 1.50),
        ("google", "gemini-pro", 0.50, 1.50),
    ];
    for (provider, model, inp, out) in cases {
        let p = resolve(provider, model, None);
        assert!(
            (p.input_per_million - inp).abs() < 1e-9,
            "{provider} {model}"
        );
        assert!(
            (p.output_per_million - out).abs() < 1e-9,
            "{provider} {model}"
        );
    }
}

#[test]
fn minimax_static_defaults() {
    let cases = [
        ("minimax", "minimax-m1-foo", 0.40, 2.20),
        ("minimax", "abab7-chat-preview", 0.40, 2.20),
        ("minimax", "minimax-text-01-x", 0.20, 1.10),
        ("minimax", "abab6.5-chat", 0.20, 1.10),
    ];
    for (provider, model, inp, out) in cases {
        let p = resolve(provider, model, None);
        assert!((p.input_per_million - inp).abs() < 1e-9, "{model}");
        assert!((p.output_per_million - out).abs() < 1e-9, "{model}");
    }
}

#[test]
fn provider_match_is_case_insensitive() {
    let p = resolve("OpenAI", "gpt-4o-mini", None);
    assert!((p.input_per_million - 0.15).abs() < 1e-9);
    let p = resolve("ANTHROPIC", "claude-opus-4", None);
    assert!((p.input_per_million - 15.0).abs() < 1e-9);
}

#[test]
fn unknown_model_in_known_provider_returns_zero() {
    let p = resolve("anthropic", "claude-99-mystery", None);
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
