//! Gateway routing patterns and stable route-id synthesis.
//!
//! A [`GatewayRoute`] maps an external `model_pattern` (exact, prefix `foo*`,
//! suffix `*foo`, or catch-all `*`) onto a provider in the registry. When a
//! route omits an explicit id, [`synthesize_route_id`] derives a stable one
//! from `(model_pattern, provider)` so `access_control_rules` can address the
//! route by a name that survives reordering. A model's connectivity is never
//! embedded here — [`GatewayRoute::resolve`] looks the provider up in the
//! registry at use time.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ProviderId, RouteId};

use super::super::providers::{ProviderEntry, ProviderRegistry};
use crate::gateway_hash::fnv1a_segments;
use crate::services::ai::ModelPricing;

fn default_route_id() -> RouteId {
    RouteId::new("")
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatewayRoute {
    #[serde(default = "default_route_id")]
    pub id: RouteId,
    pub model_pattern: String,
    pub provider: ProviderId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_model: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_headers: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricing: Option<ModelPricing>,
}

impl GatewayRoute {
    pub fn matches(&self, model: &str) -> bool {
        match_pattern(&self.model_pattern, model)
    }

    pub fn effective_upstream_model<'a>(&'a self, requested: &'a str) -> &'a str {
        self.upstream_model.as_deref().unwrap_or(requested)
    }

    pub fn ensure_id(&mut self) {
        if self.id.as_str().trim().is_empty() {
            self.id = synthesize_route_id(&self.model_pattern, self.provider.as_str());
        }
    }

    pub fn resolve<'a>(&self, registry: &'a ProviderRegistry) -> Option<&'a ProviderEntry> {
        registry.find_provider(self.provider.as_str())
    }
}

#[must_use]
pub fn slugify_pattern(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut last_dash = false;
    for ch in pattern.chars() {
        if ch == '*' {
            out.push_str("star");
            last_dash = false;
        } else if ch.is_ascii_alphanumeric() {
            for lc in ch.to_lowercase() {
                out.push(lc);
            }
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    while out.starts_with('-') {
        out.remove(0);
    }
    if out.is_empty() {
        out.push_str("route");
    }
    out
}

// Format: <slug>-<6 hex chars> where the hex digest is the first 6 chars of an
// FNV-1a 64 hash over the labelled (model_pattern, provider) segments. FNV-1a
// is used deliberately over `std::hash::DefaultHasher`: the route id is
// persisted in `access_control_entities`/`_rules` and the resolver is
// fail-closed, so the id must be stable *by contract*. DefaultHasher's
// algorithm is explicitly allowed to change between Rust releases; a toolchain
// bump would silently re-key every route and resurrect `unknown to access
// control` denials. FNV-1a never moves. The collision check in
// GatewayConfig::validate() guards against the vanishingly unlikely case of two
// operator-authored patterns colliding on the 6-hex tail.
#[must_use]
pub fn synthesize_route_id(model_pattern: &str, provider: &str) -> RouteId {
    let h = fnv1a_segments(&[
        ("model_pattern", model_pattern.as_bytes()),
        ("provider", provider.as_bytes()),
    ]);
    let hash6: String = format!("{h:016x}").chars().take(6).collect();
    RouteId::new(format!("{}-{}", slugify_pattern(model_pattern), hash6))
}

fn match_pattern(pattern: &str, model: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return model.starts_with(prefix);
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return model.ends_with(suffix);
    }
    pattern == model
}
