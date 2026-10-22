//! Whether a resolved chain can reach a Gemini upstream, which decides if
//! thought signatures are looked up for an inbound request.

use systemprompt_api::routes::gateway::messages::extract::signatures::route_needs_thought_signatures;
use systemprompt_catalog_fixtures::{CatalogFixture, claude_model, platform};
use systemprompt_gateway::RouteResolution;
use systemprompt_identifiers::ModelId;
use systemprompt_manifest::services::catalog::{EndpointConfig, ModelRequest, ProviderConfig};
use systemprompt_manifest::services::{GatewayRoute, ModelGlobs};
use systemprompt_wire::WireProtocol;

fn resolution(model: &str) -> RouteResolution {
    let mut fixture = CatalogFixture::new();
    for (name, wire) in [
        ("anthropic", WireProtocol::Anthropic),
        ("openai", WireProtocol::OpenAiChat),
        ("gemini", WireProtocol::Gemini),
    ] {
        fixture = fixture
            .credential(name, platform(name))
            .provider(name, ProviderConfig::new(name))
            .endpoint(name, "api", EndpointConfig::new(wire, "https://example.invalid/v1"));
    }
    let catalog = fixture
        .model("claude-x", claude_model(0))
        .model("claude-gf", claude_model(0))
        .model("claude-cf", claude_model(0))
        .model("gemini-x", claude_model(0))
        .deploy("claude-x", ["anthropic/api", "openai/api"])
        .deploy("claude-gf", ["anthropic/api", "gemini/api"])
        .deploy("claude-cf", ["anthropic/api"])
        .deploy("gemini-x", ["gemini/api"])
        .route(GatewayRoute {
            models: ModelGlobs(vec!["claude-cf".to_owned()]),
            context_fallbacks: vec![ModelId::new("gemini-x")],
            ..GatewayRoute::new("cf")
        })
        .build();
    RouteResolution {
        target: catalog.resolve(&ModelRequest::new(model)).expect("resolves"),
    }
}

#[test]
fn an_anthropic_and_openai_chain_needs_no_signatures() {
    assert!(!route_needs_thought_signatures(&resolution("claude-x")));
}

#[test]
fn a_gemini_primary_needs_signatures() {
    assert!(route_needs_thought_signatures(&resolution("gemini-x")));
}

#[test]
fn a_gemini_failover_deployment_needs_signatures() {
    assert!(route_needs_thought_signatures(&resolution("claude-gf")));
}

#[test]
fn a_gemini_context_fallback_needs_signatures() {
    assert!(route_needs_thought_signatures(&resolution("claude-cf")));
}
