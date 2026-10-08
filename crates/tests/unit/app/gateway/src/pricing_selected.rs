use systemprompt_gateway::pricing::resolve_upstream;
use systemprompt_manifest::services::{GatewayRoute, ModelPricing, ProviderEntry};

fn provider(name: &str, input: f64) -> ProviderEntry {
    serde_yaml::from_str(&format!(
        "name: {name}\nwire: anthropic\nsurface: anthropic\nendpoint: https://example.test/v1\napi_key_secret: {name}\nmodels:\n  - id: {name}-claude-sonnet-5\n    hidden: true\n    upstream_model: claude-sonnet-5\n    pricing:\n      input_per_million: {input}\n      output_per_million: 9.0\n      cache_read_per_million: 0.0\n"
    ))
    .expect("provider fixture")
}

fn route(name: &str) -> GatewayRoute {
    serde_yaml::from_str(&format!("model_pattern: claude-*\nprovider: {name}\n"))
        .expect("route fixture")
}

#[test]
fn selected_upstream_pricing_uses_only_its_own_aliased_catalog() {
    let native = provider("native", 7.0);
    let ordinary = provider("ordinary", 2.0);
    for (entry, input) in [(&native, 7.0), (&ordinary, 2.0)] {
        let pricing = resolve_upstream(&route(entry.name.as_str()), entry, "claude-sonnet-5[1m]")
            .expect("selected provider prices its upstream alias");
        assert!((pricing.input_per_million - input).abs() < f64::EPSILON);
    }
}

#[test]
fn selected_upstream_honors_catalog_mapping_and_route_rate_override() {
    let entry = provider("native", 7.0);
    let mut selected = route("native");
    selected.upstream_model = Some("native-claude-sonnet-5".to_owned());
    let mapped = resolve_upstream(&selected, &entry, "public-sonnet").expect("mapped catalog");
    assert!((mapped.input_per_million - 7.0).abs() < f64::EPSILON);
    selected.pricing = Some(ModelPricing {
        input_per_million: 1.0,
        output_per_million: 2.0,
        ..ModelPricing::default()
    });
    let custom = resolve_upstream(&selected, &entry, "public-sonnet").expect("custom rate");
    assert!((custom.input_per_million - 1.0).abs() < f64::EPSILON);
}

#[test]
fn selected_upstream_missing_model_is_an_error_instead_of_another_provider_rate() {
    let entry = provider("native", 7.0);
    let error = resolve_upstream(&route("native"), &entry, "claude-unknown")
        .expect_err("an unknown selected model cannot be billed at another provider's rate");
    assert_eq!(error.provider, "native");
    assert_eq!(error.models, vec!["claude-unknown"]);
}
