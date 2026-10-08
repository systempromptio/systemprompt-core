//! Gateway routing patterns and stable route-id synthesis.
//!
//! A [`GatewayRoute`] maps an external `model_pattern` (exact, prefix `foo*`,
//! suffix `*foo`, or catch-all `*`) onto a provider in the registry. When a
//! route omits an explicit id, [`synthesize_route_id`] derives a stable one
//! from `(model_pattern, provider)` so `access_control_rules` can address the
//! route by a name that survives reordering. A model's connectivity is never
//! embedded here — [`GatewayRoute::resolve`] looks the provider up in the
//! registry at use time.
//!
//! A route's deployments are its provider followed by its ordered
//! fallbacks, so an upstream that is unreachable or exhausts the
//! transient-failure retry budget hands the same request to the next.
//! `by_scope` swaps that chain for a per-scope one keyed by the value a request
//! is attributed to in one scope dimension.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod chain;
mod predicates;
mod token_estimate;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ProviderId, RouteId};

use super::route_id::{match_pattern, synthesize_route_id};
use crate::services::ai::ModelPricing;
use crate::services::providers::{ProviderEntry, ProviderRegistry};
use systemprompt_wire::canonical::CanonicalRequest;

pub use chain::{ChainSelection, RouteDeployment, RouteScopeChains, ScopeChain, UnmappedScope};
pub use predicates::{ResponseFormatKind, RouteMatch, RouteRequirements};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatewayRoute {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<RouteId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub model_pattern: String,
    pub provider: ProviderId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_model: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_headers: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricing: Option<ModelPricing>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<RouteMatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<RouteRequirements>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallbacks: Vec<RouteDeployment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by_scope: Option<RouteScopeChains>,
}

impl GatewayRoute {
    pub fn matches(&self, model: &str) -> bool {
        match_pattern(
            &self.model_pattern,
            systemprompt_models::providers::without_context_variant(model),
        )
    }

    pub fn matches_request(&self, request: &CanonicalRequest) -> bool {
        self.matches(request.model.as_str())
            && self
                .when
                .as_ref()
                .is_none_or(|w| w.matches_request(request))
    }

    pub fn effective_upstream_model<'a>(&'a self, requested: &'a str) -> &'a str {
        self.upstream_model
            .as_deref()
            .unwrap_or_else(|| systemprompt_models::providers::without_context_variant(requested))
    }

    pub fn ensure_id(&mut self) -> bool {
        if self.declared_id().is_some() {
            return false;
        }
        self.id = Some(synthesize_route_id(
            &self.model_pattern,
            self.provider.as_str(),
        ));
        true
    }

    pub fn declared_id(&self) -> Option<&RouteId> {
        self.id.as_ref().filter(|id| !id.as_str().trim().is_empty())
    }

    #[must_use]
    pub fn effective_id(&self) -> RouteId {
        self.declared_id()
            .cloned()
            .unwrap_or_else(|| synthesize_route_id(&self.model_pattern, self.provider.as_str()))
    }

    pub fn resolve<'a>(&self, registry: &'a ProviderRegistry) -> Option<&'a ProviderEntry> {
        registry.find_provider(self.provider.as_str())
    }
}
