use std::collections::HashMap;

use async_trait::async_trait;
use systemprompt_ai::{
    RouteSelector, RouteSelectorEngine, RouteSelectorError, register_route_selector,
};
use systemprompt_identifiers::{ProviderId, RouteId};
use systemprompt_models::profile::GatewayRoute;
use systemprompt_models::wire::canonical::CanonicalRequest;

fn route(pattern: &str, provider: &str) -> GatewayRoute {
    let mut r = GatewayRoute {
        id: RouteId::new(""),
        model_pattern: pattern.to_owned(),
        provider: ProviderId::new(provider),
        upstream_model: None,
        extra_headers: HashMap::new(),
        pricing: None,
        when: None,
    };
    r.ensure_id();
    r
}

fn req(model: &str) -> CanonicalRequest {
    CanonicalRequest {
        model: model.to_owned(),
        system: None,
        messages: Vec::new(),
        max_tokens: 0,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: Vec::new(),
        tools: Vec::new(),
        tool_choice: None,
        stream: false,
        thinking: None,
        metadata: None,
        response_format: None,
        reasoning_effort: None,
        search: None,
        code_execution: false,
        presence_penalty: None,
        frequency_penalty: None,
    }
}

struct RerouteToGemini;

impl RerouteToGemini {
    fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RouteSelector for RerouteToGemini {
    fn name(&self) -> &'static str {
        "reroute-to-gemini"
    }

    async fn refine(
        &self,
        matched: &GatewayRoute,
        request: &CanonicalRequest,
    ) -> Result<Option<GatewayRoute>, RouteSelectorError> {
        if request.model == "reroute-me" {
            let mut refined = matched.clone();
            refined.provider = ProviderId::new("gemini");
            refined.ensure_id();
            return Ok(Some(refined));
        }
        Ok(None)
    }
}

register_route_selector!(RerouteToGemini::new, name = "reroute-to-gemini");

#[tokio::test]
async fn engine_collects_the_registered_selector() {
    assert!(
        RouteSelectorEngine::global().has_selectors(),
        "the inventory-registered selector must be visible to the engine"
    );
}

#[tokio::test]
async fn selector_passes_through_unmatched_requests() {
    let matched = route("claude-*", "anthropic");
    let refined = RouteSelectorEngine::global()
        .refine(&matched, &req("claude-opus-4-8"))
        .await;
    assert!(refined.is_none(), "a selector that returns None re-routes nothing");
}

#[tokio::test]
async fn selector_reroutes_and_reports_its_name() {
    let matched = route("claude-*", "anthropic");
    let (refined, name) = RouteSelectorEngine::global()
        .refine(&matched, &req("reroute-me"))
        .await
        .expect("selector must re-route the matching request");
    assert_eq!(refined.provider.as_str(), "gemini");
    assert_eq!(name, "reroute-to-gemini");
}

#[tokio::test]
async fn selector_trait_refine_is_invoked_directly() {
    let matched = route("claude-*", "anthropic");
    let out = RerouteToGemini::new()
        .refine(&matched, &req("reroute-me"))
        .await
        .expect("refine must not error");
    assert_eq!(out.expect("re-routed").provider.as_str(), "gemini");
}
