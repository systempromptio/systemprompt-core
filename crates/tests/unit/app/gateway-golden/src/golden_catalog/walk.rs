//! Walks one requested model id through the 0.72.1 catalog the way dispatch
//! does: exposure, route match, deployment chain views, served model, pricing,
//! advertisement and credential source. Output is behaviour only; provider
//! names and route ids appear solely under `info`.

use serde_json::{Map, Value, json};
use systemprompt_gateway::pricing::resolve_upstream;
use systemprompt_identifiers::ModelId;
use systemprompt_manifest::services::{
    ChainSelection, GatewayConfig, GatewayRoute, ProviderEntry, ProviderRegistry,
};
use systemprompt_models::providers::ApiSurface;
use systemprompt_wire::canonical::CanonicalRequest;

const CLIENT_SURFACES: [ApiSurface; 3] = [
    ApiSurface::Anthropic,
    ApiSurface::OpenAi,
    ApiSurface::Gemini,
];

pub fn requested_ids(registry: &ProviderRegistry, gateway: Option<&GatewayConfig>) -> Vec<String> {
    let mut ids: Vec<String> = registry
        .providers
        .iter()
        .flat_map(|p| p.models.iter())
        .flat_map(|m| std::iter::once(&m.id).chain(m.aliases.iter()))
        .map(|id| id.as_str().to_owned())
        .collect();
    if let Some(default) = gateway.and_then(|g| g.default_model.clone()) {
        ids.push(default);
    }
    ids.sort();
    ids.dedup();
    ids
}

pub fn resolve_id(
    registry: &ProviderRegistry,
    gateway: Option<&GatewayConfig>,
    requested: &str,
) -> Value {
    let mut out = Map::new();
    out.insert("hidden".into(), json!(registry.is_hidden_model(requested)));
    out.insert(
        "advertised_to".into(),
        json!(advertised_to(registry, requested)),
    );
    match gateway {
        Some(gw) => resolve_via_gateway(registry, gw, requested, &mut out),
        None => resolve_via_registry(registry, requested, &mut out),
    }
    Value::Object(out)
}

fn advertised_to(registry: &ProviderRegistry, requested: &str) -> Vec<&'static str> {
    CLIENT_SURFACES
        .iter()
        .filter(|s| {
            registry
                .advertised_model_ids(&[**s])
                .iter()
                .any(|id| id == requested)
        })
        .map(|s| s.as_tag())
        .collect()
}

fn resolve_via_registry(
    registry: &ProviderRegistry,
    requested: &str,
    out: &mut Map<String, Value>,
) {
    out.insert("exposed".into(), json!(registry.contains_model(requested)));
    let Some(provider) = registry
        .providers
        .iter()
        .find(|p| p.find_model(requested).is_some())
    else {
        out.insert("error".into(), json!("no provider serves the model"));
        return;
    };
    let route = GatewayRoute {
        id: None,
        name: None,
        description: None,
        model_pattern: requested.to_owned(),
        provider: provider.name.clone(),
        upstream_model: None,
        extra_headers: std::collections::HashMap::new(),
        pricing: None,
        when: None,
        requires: None,
        fallbacks: Vec::new(),
        by_scope: None,
        strategy: systemprompt_manifest::services::SelectionStrategy::Ordered,
        weight: None,
        context_fallbacks: Vec::new(),
    };
    out.insert(
        "chain".into(),
        json!([hop(registry, &route, requested, false)]),
    );
}

fn resolve_via_gateway(
    registry: &ProviderRegistry,
    gw: &GatewayConfig,
    requested: &str,
    out: &mut Map<String, Value>,
) {
    out.insert(
        "exposed".into(),
        json!(gw.is_model_exposed(registry, requested)),
    );
    let request = CanonicalRequest::new(ModelId::new(requested), Vec::new(), 1);
    let Some(route) = gw.resolve_route(registry, &request) else {
        out.insert("error".into(), json!("no route"));
        return;
    };
    out.insert("strategy".into(), json!(route.strategy));
    let views = route.chain_views(&ChainSelection::Route);
    out.insert("chain".into(), hops(registry, &views, requested, true));
    let ctx = route.context_fallback_views(&ChainSelection::Route);
    if !ctx.is_empty() {
        out.insert(
            "context_fallbacks".into(),
            hops(registry, &ctx, requested, true),
        );
    }
    let scoped: Map<String, Value> = route
        .all_chains()
        .into_iter()
        .filter_map(|(value, views)| {
            value.map(|v| (v.to_owned(), hops(registry, &views, requested, true)))
        })
        .collect();
    if let Some(by_scope) = route.by_scope.as_ref() {
        out.insert(
            "by_scope".into(),
            json!({ "dimension": by_scope.dimension.as_str(), "unmapped": by_scope.unmapped, "chains": scoped }),
        );
    }
}

fn hops(registry: &ProviderRegistry, views: &[GatewayRoute], requested: &str, gw: bool) -> Value {
    Value::Array(
        views
            .iter()
            .map(|v| hop(registry, v, requested, gw))
            .collect(),
    )
}

fn hop(registry: &ProviderRegistry, view: &GatewayRoute, requested: &str, gw: bool) -> Value {
    let Some(provider) = view.resolve(registry) else {
        return json!({ "error": "undeclared provider", "info": { "provider": view.provider.as_str() } });
    };
    let mut info = json!({ "provider": provider.name.as_str() });
    if gw {
        info["route_id"] = json!(view.effective_id().as_str());
    }
    let pricing = resolve_upstream(view, provider, requested)
        .map_or_else(|e| json!({ "error": e.to_string() }), |p| json!(p));
    json!({
        "url": provider.endpoint,
        "wire": provider.wire,
        "credential": credential(provider),
        "upstream_model": provider.upstream_model_for(view.upstream_model.as_deref(), requested),
        "served": provider.find_served_model(view.upstream_model.as_deref().unwrap_or(requested)).is_some()
            || provider.find_served_model(requested).is_some(),
        "pricing": pricing,
        "weight": view.weight,
        "info": info,
    })
}

fn credential(provider: &ProviderEntry) -> Value {
    if provider.is_user_credential() {
        json!({ "source": "personal", "owner": provider.credential_owner() })
    } else {
        json!({ "source": "platform", "secret": provider.platform_secret().map(|s| s.as_str()) })
    }
}
