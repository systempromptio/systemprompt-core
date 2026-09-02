//! Tests for the `admin config gateway` services-file mutators: enable state,
//! route upsert/remove, default provider, and registry validation.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::config::gateway::{
    RouteAddArgs, add_route, clear_default_provider, remove_route, set_default_provider,
    set_enabled, spec_mut, validate_gateway,
};
use systemprompt_cli::admin::config::services_io::GatewayFile;
use systemprompt_models::services::{GatewayConfigSpec, GatewayState, ProviderRegistry};

fn file() -> GatewayFile {
    GatewayFile { gateway: None }
}

fn registry() -> ProviderRegistry {
    ProviderRegistry::default_seed().unwrap()
}

fn route_args(pattern: &str, provider: &str) -> RouteAddArgs {
    RouteAddArgs {
        model_pattern: pattern.to_string(),
        provider: provider.to_string(),
        upstream_model: None,
    }
}

fn spec(file: &GatewayFile) -> &GatewayConfigSpec {
    match file.gateway.as_ref().unwrap() {
        GatewayState::Spec(s) => s,
        GatewayState::Resolved(_) => panic!("gateway unexpectedly resolved"),
    }
}

#[test]
fn set_enabled_creates_spec_and_toggles_flag() {
    let mut f = file();
    let msg = set_enabled(&mut f, true).unwrap();
    assert_eq!(msg, "Gateway enabled = true");
    assert!(spec(&f).enabled);

    let msg = set_enabled(&mut f, false).unwrap();
    assert_eq!(msg, "Gateway enabled = false");
    assert!(!spec(&f).enabled);
}

#[test]
fn spec_mut_rejects_a_resolved_gateway() {
    let mut f = file();
    f.gateway = Some(GatewayState::Resolved(
        GatewayConfigSpec::default().resolve(),
    ));
    let err = spec_mut(&mut f).unwrap_err().to_string();
    assert!(err.contains("resolved state"), "unexpected error: {err}");
}

#[test]
fn add_route_mints_an_id_and_upserts_by_pattern() {
    let mut f = file();
    let msg = add_route(&mut f, &route_args("claude-*", "anthropic")).unwrap();
    assert_eq!(msg, "Route claude-* -> anthropic added");
    let first_id = spec(&f).routes[0].id.clone();
    assert!(!first_id.as_str().is_empty());

    add_route(&mut f, &route_args("claude-*", "openai")).unwrap();
    let routes = &spec(&f).routes;
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].provider.as_str(), "openai");
    assert_ne!(routes[0].id, first_id);
}

#[test]
fn remove_route_deletes_matching_pattern_and_errors_when_absent() {
    let mut f = file();
    add_route(&mut f, &route_args("claude-*", "anthropic")).unwrap();
    add_route(&mut f, &route_args("gpt-*", "openai")).unwrap();

    let msg = remove_route(&mut f, "claude-*").unwrap();
    assert_eq!(msg, "Route claude-* removed");
    assert_eq!(spec(&f).routes.len(), 1);
    assert_eq!(spec(&f).routes[0].model_pattern, "gpt-*");

    let err = remove_route(&mut f, "claude-*").unwrap_err().to_string();
    assert!(err.contains("No route found"), "unexpected error: {err}");
}

#[test]
fn default_provider_set_and_clear_round_trip() {
    let mut f = file();
    let msg = set_default_provider(&mut f, "anthropic").unwrap();
    assert_eq!(msg, "Gateway default provider set to anthropic");
    assert_eq!(
        spec(&f).default_provider.as_ref().unwrap().as_str(),
        "anthropic"
    );

    let msg = clear_default_provider(&mut f).unwrap();
    assert_eq!(msg, "Gateway default provider cleared");
    assert!(spec(&f).default_provider.is_none());
}

#[test]
fn validate_gateway_passes_without_gateway_and_with_registry_providers() {
    let mut f = file();
    validate_gateway(&f, &registry()).unwrap();

    add_route(&mut f, &route_args("claude-*", "anthropic")).unwrap();
    set_default_provider(&mut f, "openai").unwrap();
    validate_gateway(&f, &registry()).unwrap();
}

#[test]
fn validate_gateway_rejects_route_provider_missing_from_registry() {
    let mut f = file();
    add_route(&mut f, &route_args("claude-*", "no-such-provider")).unwrap();
    let err = validate_gateway(&f, &registry()).unwrap_err().to_string();
    assert!(
        err.contains("gateway validation failed"),
        "unexpected error: {err}"
    );
    assert!(err.contains("no-such-provider"), "unexpected error: {err}");
}

#[test]
fn validate_gateway_rejects_unknown_default_provider() {
    let mut f = file();
    set_default_provider(&mut f, "ghost").unwrap();
    let err = validate_gateway(&f, &registry()).unwrap_err().to_string();
    assert!(err.contains("ghost"), "unexpected error: {err}");
}

#[test]
fn gateway_file_round_trips_through_yaml() {
    let mut f = file();
    set_enabled(&mut f, true).unwrap();
    add_route(&mut f, &route_args("claude-*", "anthropic")).unwrap();
    let yaml = serde_yaml::to_string(&f).unwrap();
    assert!(yaml.starts_with("gateway:"), "unexpected yaml: {yaml}");
    let back: GatewayFile = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(spec(&back).routes.len(), 1);
    assert!(spec(&back).enabled);
}
