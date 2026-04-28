use std::collections::HashMap;
use systemprompt_models::profile::{GatewayConfig, GatewayRoute};

fn route(pattern: &str) -> GatewayRoute {
    GatewayRoute {
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
