//! Whether a route's deployments can reach a Gemini upstream, which decides if
//! thought signatures are looked up for an inbound request.

use systemprompt_api::routes::gateway::messages::extract::signatures::route_needs_thought_signatures;
use systemprompt_identifiers::{ProviderId, SecretName};
use systemprompt_manifest::services::gateway::GatewayRoute;
use systemprompt_manifest::services::providers::surface_for;
use systemprompt_manifest::services::{ProviderEntry, ProviderRegistry};
use systemprompt_wire::WireProtocol;

fn provider(name: &str, wire: WireProtocol) -> ProviderEntry {
    ProviderEntry {
        name: ProviderId::new(name),
        display_name: None,
        description: None,
        wire,
        surface: surface_for(wire),
        endpoint: "https://example.invalid/v1".to_owned(),
        api_key_secret: Some(SecretName::new(name)),
        credential: Default::default(),
        credential_from: None,
        user_agent: None,
        forward_headers: Vec::new(),
        governance: Default::default(),
        extra_headers: Default::default(),
        accepted_betas: None,
        models: Vec::new(),
    }
}

fn registry() -> ProviderRegistry {
    ProviderRegistry {
        providers: vec![
            provider("anthropic", WireProtocol::Anthropic),
            provider("openai", WireProtocol::OpenAiChat),
            provider("gemini", WireProtocol::Gemini),
        ],
    }
}

fn route(value: serde_json::Value) -> GatewayRoute {
    serde_json::from_value(value).expect("route")
}

#[test]
fn an_anthropic_only_route_needs_no_signatures() {
    let r = route(serde_json::json!({
        "model_pattern": "claude-*",
        "provider": "anthropic",
        "fallbacks": [{"provider": "openai"}]
    }));
    assert!(!route_needs_thought_signatures(&registry(), &r));
}

#[test]
fn a_gemini_primary_needs_signatures() {
    let r = route(serde_json::json!({"model_pattern": "gemini-*", "provider": "gemini"}));
    assert!(route_needs_thought_signatures(&registry(), &r));
}

#[test]
fn a_gemini_fallback_needs_signatures() {
    let r = route(serde_json::json!({
        "model_pattern": "claude-*",
        "provider": "anthropic",
        "fallbacks": [{"provider": "gemini"}]
    }));
    assert!(route_needs_thought_signatures(&registry(), &r));
}

#[test]
fn a_gemini_context_fallback_needs_signatures() {
    let r = route(serde_json::json!({
        "model_pattern": "claude-*",
        "provider": "anthropic",
        "context_fallbacks": [{"provider": "gemini"}]
    }));
    assert!(route_needs_thought_signatures(&registry(), &r));
}

#[test]
fn a_gemini_scope_chain_needs_signatures() {
    let r = route(serde_json::json!({
        "model_pattern": "claude-*",
        "provider": "anthropic",
        "by_scope": {
            "dimension": "project",
            "chains": {"apollo": {"provider": "openai", "fallbacks": [{"provider": "gemini"}]}}
        }
    }));
    assert!(route_needs_thought_signatures(&registry(), &r));
}

#[test]
fn an_unknown_provider_needs_no_signatures() {
    let r = route(serde_json::json!({"model_pattern": "x-*", "provider": "missing"}));
    assert!(!route_needs_thought_signatures(&registry(), &r));
}
