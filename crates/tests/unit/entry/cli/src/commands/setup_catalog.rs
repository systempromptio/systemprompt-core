//! Tests for `admin::setup::catalog` — the provider registry and gateway route
//! generation the setup wizard emits from the supplied AI keys.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::setup::SecretsData;
use systemprompt_cli::admin::setup::catalog::{build_registry, build_routes};
use systemprompt_models::profile::{GatewayConfig, GatewayRoute};

fn secrets_with(openai: bool, anthropic: bool, gemini: bool) -> SecretsData {
    SecretsData {
        openai: openai.then(|| "sk-openai".to_owned()),
        anthropic: anthropic.then(|| "sk-anthropic".to_owned()),
        gemini: gemini.then(|| "sk-gemini".to_owned()),
        ..SecretsData::default()
    }
}

fn route_for<'a>(routes: &'a [GatewayRoute], provider: &str) -> &'a GatewayRoute {
    routes
        .iter()
        .find(|r| r.provider.as_str() == provider)
        .unwrap_or_else(|| panic!("no route for provider {provider}"))
}

#[test]
fn openai_route_pins_a_concrete_upstream_model() {
    let routes = build_routes(&secrets_with(true, false, false));
    let openai = route_for(&routes, "openai");
    assert_eq!(openai.model_pattern, "gpt-*");
    assert_eq!(
        openai.upstream_model.as_deref(),
        Some("gpt-5-mini"),
        "the openai default must rewrite Codex aliases to a concrete model, not pass through"
    );
}

#[test]
fn passthrough_providers_keep_none_upstream() {
    let routes = build_routes(&secrets_with(false, true, true));
    assert_eq!(route_for(&routes, "anthropic").upstream_model, None);
    assert_eq!(route_for(&routes, "gemini").upstream_model, None);
}

#[test]
fn routes_are_only_emitted_for_present_keys() {
    let routes = build_routes(&secrets_with(true, false, false));
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].provider.as_str(), "openai");
}

#[test]
fn generated_registry_and_gateway_validate_together() {
    let secrets = secrets_with(true, true, true);
    let registry = build_registry(&secrets);
    registry.validate().expect("seeded registry must validate");

    let config = GatewayConfig {
        enabled: true,
        routes: build_routes(&secrets),
        ..GatewayConfig::default()
    };
    config
        .validate(&registry)
        .expect("every generated route must reference a provider in the registry");
}
