//! Walks one requested model id through a `ResolvedCatalog` the way dispatch
//! does and renders it in the 0.72.1 golden schema: exposure, advertisement,
//! the plain-request chain, context fallbacks, per-scope chains, and per hop
//! the endpoint URL, wire, credential, upstream model and price. Provider,
//! endpoint and route ids appear only under `info`.
//!
//! Schema mapping from 0.73: `advertised_to` uses the 0.72 surface tags
//! (`claude` is written `anthropic`); `hidden` is "not advertised"
//! (`clients: []`); every hop is a deployment of the model, so `served` is
//! always true; `by_scope` is synthesised from the scope routes that match
//! the model, with `unmapped: shared` because an unmatched scope falls
//! through to the model's own chain.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};
use systemprompt_identifiers::ScopeDimension;
use systemprompt_manifest::services::catalog::{
    ModelRequest, ResolveError, ResolvedCatalog, ResolvedHop,
};
use systemprompt_models::attribution::{AttributionEntry, AttributionSource, RequestAttribution};
use systemprompt_models::providers::ClientFamily;

pub fn requested_ids(catalog: &ResolvedCatalog) -> Vec<String> {
    let mut ids: Vec<String> = catalog.models().map(|m| m.id.as_str().to_owned()).collect();
    ids.extend(
        catalog
            .routes()
            .iter()
            .filter_map(|r| r.alias.as_ref())
            .map(|a| a.requested.as_str().to_owned()),
    );
    if let Some(default) = catalog.default_model() {
        ids.push(default.as_str().to_owned());
    }
    ids.sort();
    ids.dedup();
    ids
}

const fn surface_tag(family: ClientFamily) -> &'static str {
    match family {
        ClientFamily::Claude => "anthropic",
        ClientFamily::OpenAi => "openai",
        ClientFamily::Gemini => "gemini",
    }
}

pub fn resolve_id(catalog: &ResolvedCatalog, requested: &str) -> Value {
    let mut out = Map::new();
    let model = catalog.model(requested);
    out.insert(
        "hidden".into(),
        json!(model.is_some_and(|m| !m.is_advertised())),
    );
    let advertised: Vec<&str> = ClientFamily::ALL
        .into_iter()
        .filter(|f| model.is_some_and(|m| m.is_advertised_to(*f)))
        .map(surface_tag)
        .collect();
    out.insert("advertised_to".into(), json!(advertised));
    let target = match catalog.resolve(&ModelRequest::new(requested)) {
        Ok(target) => target,
        Err(error) => {
            out.insert("exposed".into(), json!(false));
            let reason = match error {
                ResolveError::UnknownModel { .. } => "no provider serves the model",
                ResolveError::NoEnabledDeployment { .. } => "no enabled deployment",
            };
            out.insert("error".into(), json!(reason));
            return Value::Object(out);
        },
    };
    out.insert("exposed".into(), json!(true));
    let gateway = catalog.gateway_enabled();
    if gateway {
        out.insert("strategy".into(), json!(target.strategy));
    }
    let route_id = gateway.then(|| target.access_route_id.as_str().to_owned());
    out.insert("chain".into(), hops(&target.chain, route_id.as_deref()));
    if !target.context_fallbacks.is_empty() {
        out.insert(
            "context_fallbacks".into(),
            hops(&target.context_fallbacks, route_id.as_deref()),
        );
    }
    if let Some(by_scope) = scope_chains(catalog, requested, target.model.id.as_str()) {
        out.insert("by_scope".into(), by_scope);
    }
    Value::Object(out)
}

fn scope_chains(catalog: &ResolvedCatalog, requested: &str, model: &str) -> Option<Value> {
    let mut dimension: Option<&ScopeDimension> = None;
    let mut chains: BTreeMap<String, Value> = BTreeMap::new();
    for route in catalog.routes() {
        let Some(when) = route.when.as_ref() else {
            continue;
        };
        let targets_model = route.matches_model(model) || route.aliases(requested).is_some();
        let Some((dim, value)) = when.scope.iter().next().filter(|_| targets_model) else {
            continue;
        };
        dimension.get_or_insert(dim);
        let attribution = RequestAttribution {
            entries: vec![AttributionEntry {
                dimension: dim.clone(),
                value: value.clone(),
                source: AttributionSource::Header,
            }],
            api_key_id: None,
        };
        if let Ok(target) =
            catalog.resolve(&ModelRequest::new(requested).with_attribution(&attribution))
        {
            let route_id = target.access_route_id.as_str().to_owned();
            chains.insert(value.clone(), hops(&target.chain, Some(&route_id)));
        }
    }
    dimension
        .map(|dim| json!({ "dimension": dim.as_str(), "unmapped": "shared", "chains": chains }))
}

fn hops(chain: &[ResolvedHop], route_id: Option<&str>) -> Value {
    Value::Array(chain.iter().map(|hop| render_hop(hop, route_id)).collect())
}

fn render_hop(hop: &ResolvedHop, route_id: Option<&str>) -> Value {
    let mut info = json!({ "provider": hop.provider.as_str(), "endpoint": hop.endpoint.as_str() });
    if let Some(route_id) = route_id {
        info["route_id"] = json!(route_id);
    }
    let credential = hop.credential.platform_secret().map_or_else(
        || json!({ "source": "personal", "owner": hop.credential.id.as_str() }),
        |secret| json!({ "source": "platform", "secret": secret.as_str() }),
    );
    json!({
        "url": hop.url,
        "wire": hop.wire,
        "credential": credential,
        "upstream_model": hop.upstream_model,
        "served": true,
        "pricing": hop.pricing,
        "weight": hop.weight,
        "info": info,
    })
}
