use std::collections::HashMap;
use systemprompt_models::profile::{
    GatewayConfig, GatewayRoute, slugify_pattern, synthesize_route_id,
};

fn route(pattern: &str) -> GatewayRoute {
    GatewayRoute {
        id: String::new(),
        model_pattern: pattern.to_string(),
        provider: "test".to_string(),
        endpoint: "https://example.com".to_string(),
        api_key_secret: "secret".to_string(),
        upstream_model: None,
        extra_headers: HashMap::new(),
    }
}

#[test]
fn exact_pattern_matches() {
    assert!(route("claude-sonnet-4-6").matches("claude-sonnet-4-6"));
    assert!(!route("claude-sonnet-4-6").matches("claude-opus-4-7"));
}

#[test]
fn prefix_wildcard_matches() {
    assert!(route("claude-*").matches("claude-sonnet-4-6"));
    assert!(!route("claude-*").matches("moonshot-v1-8k"));
}

#[test]
fn catch_all_matches() {
    assert!(route("*").matches("any-model-name"));
}

#[test]
fn route_finds_matching_model() {
    let config = GatewayConfig {
        enabled: true,
        routes: vec![GatewayRoute {
            id: String::new(),
            model_pattern: "kimi-*".to_string(),
            provider: "moonshot".to_string(),
            endpoint: "https://api.moonshot.ai/v1".to_string(),
            api_key_secret: "moonshot".to_string(),
            upstream_model: Some("moonshot-v1-32k".to_string()),
            extra_headers: HashMap::new(),
        }],
        ..GatewayConfig::default()
    };
    let matched = config.find_route("kimi-latest").expect("route must match");
    assert_eq!(matched.provider, "moonshot");
    assert_eq!(
        matched.effective_upstream_model("kimi-latest"),
        "moonshot-v1-32k"
    );
}

#[test]
fn slugify_replaces_star_and_non_alnum() {
    assert_eq!(slugify_pattern("claude-*"), "claude-star");
    assert_eq!(slugify_pattern("foo/bar baz!"), "foo-bar-baz");
    assert_eq!(slugify_pattern("---"), "route");
    assert_eq!(slugify_pattern(""), "route");
    assert_eq!(slugify_pattern("Claude_3.7"), "claude-3-7");
}

#[test]
fn synthesize_route_id_is_stable_and_input_dependent() {
    let a = synthesize_route_id("claude-*", "anthropic", "https://api.anthropic.com");
    let b = synthesize_route_id("claude-*", "anthropic", "https://api.anthropic.com");
    assert_eq!(a, b, "synthesize_route_id must be deterministic");
    assert!(a.starts_with("claude-star-"));

    let c = synthesize_route_id("claude-*", "anthropic", "https://other.example");
    assert_ne!(a, c, "endpoint change must produce a different id");

    let d = synthesize_route_id("gpt-*", "anthropic", "https://api.anthropic.com");
    assert_ne!(a, d, "model_pattern change must produce a different id");
}

#[test]
fn ensure_id_backfills_empty_id() {
    let mut r = route("claude-*");
    assert!(r.id.is_empty());
    r.ensure_id();
    assert!(!r.id.is_empty());
    let preserved = r.id.clone();
    r.ensure_id();
    assert_eq!(r.id, preserved, "ensure_id must be idempotent");
}
