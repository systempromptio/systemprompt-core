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
        catalog_path: None,
        catalog,
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
