use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ProviderId, RouteId};

use super::catalog::GatewayProvider;
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

    pub fn resolve<'a>(&self, providers: &'a [GatewayProvider]) -> Option<&'a GatewayProvider> {
        providers.iter().find(|p| p.name == self.provider)
    }
}

/// Slugify a model pattern for use in a stable id.
///
/// Mirrors the template's historical implementation in
/// `extensions/web/admin/.../gateway.rs`: `*` becomes `star`,
/// non-alphanumeric runs collapse to a single `-`, leading/trailing `-`
/// are trimmed, and an empty result becomes `route`.
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

// Format: <slug>-<6 hex chars> where the hex digest is the first 6 chars of
// DefaultHasher over (model_pattern, provider). The collision check in
// GatewayConfig::validate() guards against the vanishingly unlikely case of
// two operator-authored patterns colliding on the 6-hex tail.
#[must_use]
pub fn synthesize_route_id(model_pattern: &str, provider: &str) -> RouteId {
    let mut hasher = DefaultHasher::new();
    model_pattern.hash(&mut hasher);
    provider.hash(&mut hasher);
    let h = hasher.finish();
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
