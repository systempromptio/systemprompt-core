use systemprompt_identifiers::ProviderId;
use systemprompt_manifest::services::{GatewayConfig, ProviderModel, ProviderRegistry};
use systemprompt_wire::WireProtocol;

use crate::profile_gateway::{priced_model, priced_provider, req, route_to, token_rates};

fn hidden(id: &str) -> ProviderModel {
    let mut model = priced_model(id, token_rates(15.0, 75.0));
    model.hidden = true;
    model
}

fn registry() -> ProviderRegistry {
    ProviderRegistry {
        providers: vec![priced_provider(
            "anthropic",
            WireProtocol::Anthropic,
            vec![
                priced_model("claude-haiku", token_rates(1.0, 5.0)),
                hidden("anthropic-claude-opus"),
            ],
        )],
    }
}

fn config(patterns: &[&str], hidden_requires_route: bool) -> GatewayConfig {
    GatewayConfig {
        enabled: true,
        routes: patterns
            .iter()
            .map(|pattern| route_to(pattern, "anthropic"))
            .collect(),
        default_provider: Some(ProviderId::new("anthropic")),
        hidden_requires_route,
        ..GatewayConfig::default()
    }
}

#[test]
fn hidden_model_is_not_exposed_without_a_declared_route() {
    let config = config(&["claude-*"], true);
    assert!(!config.is_model_exposed(&registry(), "anthropic-claude-opus"));
}

#[test]
fn synthesized_default_route_does_not_serve_a_hidden_model() {
    let config = config(&["claude-*"], true);
    assert!(
        config
            .resolve_route(&registry(), &req("anthropic-claude-opus"))
            .is_none()
    );
}

#[test]
fn a_declared_route_naming_the_hidden_model_serves_it() {
    let config = config(&["claude-*", "anthropic-claude-*"], true);
    let registry = registry();
    assert!(config.is_model_exposed(&registry, "anthropic-claude-opus"));
    let route = config
        .resolve_route(&registry, &req("anthropic-claude-opus"))
        .expect("declared route matches");
    assert_eq!(route.model_pattern, "anthropic-claude-*");
}

#[test]
fn visible_catalog_models_stay_exposed_through_the_default_route() {
    let config = config(&[], true);
    let registry = registry();
    assert!(config.is_model_exposed(&registry, "claude-haiku"));
    assert!(
        config
            .resolve_route(&registry, &req("claude-haiku"))
            .is_some()
    );
}

#[test]
fn opting_out_restores_catalog_exposure_of_hidden_models() {
    let config = config(&["claude-*"], false);
    let registry = registry();
    assert!(config.is_model_exposed(&registry, "anthropic-claude-opus"));
    assert!(
        config
            .resolve_route(&registry, &req("anthropic-claude-opus"))
            .is_some()
    );
}

#[test]
fn hidden_models_never_appear_in_the_advertised_catalog() {
    let ids = registry().advertised_model_ids(&[]);
    assert!(ids.iter().any(|id| id == "claude-haiku"));
    assert!(!ids.iter().any(|id| id == "anthropic-claude-opus"));
}

#[test]
fn the_spec_defaults_to_requiring_a_route_for_hidden_models() {
    let spec: systemprompt_manifest::services::GatewayConfigSpec =
        serde_yaml::from_str("enabled: true\n").unwrap();
    assert!(spec.resolve().hidden_requires_route);
}
