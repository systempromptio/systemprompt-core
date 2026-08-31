//! Runtime projection of the gateway configuration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;
use std::collections::HashMap;

use systemprompt_identifiers::{ProviderId, RouteId};

use crate::profile::gateway::config::{
    BridgeReleasesSpec, DEFAULT_ROUTE_PATTERN, GatewayConfigSpec, default_auth_scheme,
    default_inference_path_prefix,
};
use crate::profile::gateway::override_rule::SystemPromptRule;
use crate::profile::gateway::route::GatewayRoute;
use crate::profile::providers::ProviderRegistry;
use crate::wire::canonical::CanonicalRequest;

/// Runtime gateway configuration: the post-resolution shape every non-loader
/// caller sees.
///
/// Not `Deserialize`: the only legal construction paths are
/// [`GatewayConfigSpec::resolve`] for the production loader and direct
/// struct-literal construction in tests.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    pub enabled: bool,
    pub routes: Vec<GatewayRoute>,
    pub default_provider: Option<ProviderId>,
    pub default_model: Option<String>,
    pub allow_unlisted_models: bool,
    pub auth_scheme: String,
    pub inference_path_prefix: String,
    pub system_prompt_overrides: Vec<SystemPromptRule>,
    pub bridge_releases: Option<BridgeReleasesSpec>,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            routes: Vec::new(),
            default_provider: None,
            default_model: None,
            allow_unlisted_models: false,
            auth_scheme: default_auth_scheme(),
            inference_path_prefix: default_inference_path_prefix(),
            system_prompt_overrides: Vec::new(),
            bridge_releases: None,
        }
    }
}

impl GatewayConfig {
    pub fn find_route(&self, model: &str) -> Option<&GatewayRoute> {
        self.routes.iter().find(|route| route.matches(model))
    }

    pub fn candidate_routes<'a>(
        &'a self,
        registry: &ProviderRegistry,
    ) -> impl Iterator<Item = Cow<'a, GatewayRoute>> {
        self.routes
            .iter()
            .map(Cow::Borrowed)
            .chain(self.synthesize_default_route(registry).map(Cow::Owned))
    }

    #[must_use]
    pub fn resolve_route<'a>(
        &'a self,
        registry: &ProviderRegistry,
        request: &CanonicalRequest,
    ) -> Option<Cow<'a, GatewayRoute>> {
        self.candidate_routes(registry)
            .find(|route| route.matches_request(request))
    }

    #[must_use]
    pub fn dispatchable_route_ids(&self, registry: &ProviderRegistry) -> Vec<RouteId> {
        let mut ids: Vec<RouteId> = Vec::new();
        let mut seen: std::collections::HashSet<RouteId> = std::collections::HashSet::new();
        for route in self.candidate_routes(registry) {
            let mut route = route.into_owned();
            route.ensure_id();
            if seen.insert(route.id.clone()) {
                ids.push(route.id);
            }
        }
        ids
    }

    fn synthesize_default_route(&self, registry: &ProviderRegistry) -> Option<GatewayRoute> {
        let provider = self.default_provider.as_ref()?;
        registry.find_provider(provider.as_str())?;
        let mut route = GatewayRoute {
            id: RouteId::new(""),
            model_pattern: DEFAULT_ROUTE_PATTERN.to_owned(),
            provider: provider.clone(),
            upstream_model: None,
            extra_headers: HashMap::new(),
            pricing: None,
            when: None,
            requires: None,
        };
        route.ensure_id();
        Some(route)
    }

    #[must_use]
    pub fn is_model_exposed(&self, registry: &ProviderRegistry, model: &str) -> bool {
        if self.find_route(model).is_some() || registry.contains_model(model) {
            return true;
        }
        if self.default_provider.is_some() && self.allow_unlisted_models {
            tracing::warn!(
                model,
                "gateway forwarding an unlisted model to default_provider \
                 (allow_unlisted_models=true): open allowlist posture"
            );
            return true;
        }
        false
    }

    #[must_use]
    pub fn to_spec(&self) -> GatewayConfigSpec {
        GatewayConfigSpec {
            enabled: self.enabled,
            routes: self.routes.clone(),
            default_provider: self.default_provider.clone(),
            default_model: self.default_model.clone(),
            allow_unlisted_models: self.allow_unlisted_models,
            auth_scheme: self.auth_scheme.clone(),
            inference_path_prefix: self.inference_path_prefix.clone(),
            system_prompt_overrides: self.system_prompt_overrides.clone(),
            bridge_releases: self.bridge_releases.clone(),
        }
    }
}
