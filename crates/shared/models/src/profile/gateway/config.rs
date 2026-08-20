//! Gateway configuration: on-disk spec and resolved runtime form.
//!
//! [`GatewayConfigSpec`] is the serde shape accepted under `gateway:` in a
//! profile; [`GatewayConfig`] is its runtime projection. Routes carry no
//! embedded provider catalog — every route resolves its provider against
//! `profile.providers` ([`ProviderRegistry`]) at use time.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ProviderId, RouteId};

use super::super::providers::ProviderRegistry;
use super::error::{GatewayProfileError, GatewayResult};
use super::override_rule::SystemPromptRule;
use super::route::GatewayRoute;
use crate::wire::canonical::CanonicalRequest;

pub(crate) const DEFAULT_ROUTE_PATTERN: &str = "*";

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatewayConfigSpec {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub routes: Vec<GatewayRoute>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_provider: Option<ProviderId>,
    #[serde(default)]
    pub allow_unlisted_models: bool,
    #[serde(default = "default_auth_scheme")]
    pub auth_scheme: String,
    #[serde(default = "default_inference_path_prefix")]
    pub inference_path_prefix: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub system_prompt_overrides: Vec<SystemPromptRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_releases: Option<BridgeReleasesSpec>,
}

/// Release feed for the desktop bridge self-updater.
///
/// The bridge cannot reach these assets itself — the repository is private —
/// so the gateway resolves and proxies them. Keeping the resolution here is
/// also what makes staged rollouts a config change rather than a client
/// release.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BridgeReleasesSpec {
    pub repo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_env: Option<String>,
    #[serde(default = "default_tag_prefix")]
    pub tag_prefix: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_version: Option<String>,
    #[serde(default)]
    pub assets: std::collections::BTreeMap<String, String>,
}

fn default_tag_prefix() -> String {
    "bridge-v".to_owned()
}

impl Default for GatewayConfigSpec {
    fn default() -> Self {
        Self {
            enabled: false,
            routes: Vec::new(),
            default_provider: None,
            allow_unlisted_models: false,
            auth_scheme: default_auth_scheme(),
            inference_path_prefix: default_inference_path_prefix(),
            system_prompt_overrides: Vec::new(),
            bridge_releases: None,
        }
    }
}

fn default_auth_scheme() -> String {
    "bearer".to_owned()
}

fn default_inference_path_prefix() -> String {
    "/v1".to_owned()
}

impl GatewayConfigSpec {
    #[must_use]
    pub fn resolve(self) -> GatewayConfig {
        let Self {
            enabled,
            routes,
            default_provider,
            allow_unlisted_models,
            auth_scheme,
            inference_path_prefix,
            system_prompt_overrides,
            bridge_releases,
        } = self;

        GatewayConfig {
            enabled,
            routes,
            default_provider,
            allow_unlisted_models,
            auth_scheme,
            inference_path_prefix,
            system_prompt_overrides,
            bridge_releases,
        }
    }
}

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

    pub fn validate(&self, registry: &ProviderRegistry) -> GatewayResult<()> {
        let mut route_ids: std::collections::HashSet<&str> =
            std::collections::HashSet::with_capacity(self.routes.len());
        for route in &self.routes {
            if !route_ids.insert(route.id.as_str()) {
                return Err(GatewayProfileError::DuplicateRouteId {
                    id: route.id.as_str().to_owned(),
                });
            }
        }
        if let Some(provider) = self.default_provider.as_ref()
            && registry.find_provider(provider.as_str()).is_none()
        {
            return Err(GatewayProfileError::DefaultProviderNotInRegistry {
                provider: provider.as_str().to_owned(),
            });
        }
        for route in &self.routes {
            if registry.find_provider(route.provider.as_str()).is_none() {
                return Err(GatewayProfileError::RouteProviderNotInRegistry {
                    route: route.model_pattern.clone(),
                    provider: route.provider.as_str().to_owned(),
                });
            }
            if let Some(when) = route.when.as_ref() {
                when.validate()?;
            }
            self.validate_route_pricing(registry, route)?;
        }
        for rule in &self.system_prompt_overrides {
            rule.validate()?;
            if let Some(provider) = rule.provider.as_ref()
                && registry.find_provider(provider.as_str()).is_none()
            {
                return Err(GatewayProfileError::OverrideProviderNotInRegistry {
                    provider: provider.as_str().to_owned(),
                });
            }
        }
        Ok(())
    }

    fn validate_route_pricing(
        &self,
        registry: &ProviderRegistry,
        route: &GatewayRoute,
    ) -> GatewayResult<()> {
        if !self.enabled {
            return Ok(());
        }
        let route_id = route.id.as_str().to_owned();
        if let Some(pricing) = route.pricing {
            return if pricing.is_billable() {
                Ok(())
            } else {
                Err(GatewayProfileError::RouteModelUnpriced {
                    route: route_id,
                    model: route.model_pattern.clone(),
                })
            };
        }
        let Some(entry) = route.resolve(registry) else {
            return Ok(());
        };
        if let Some(upstream) = route.upstream_model.as_deref() {
            return match entry.find_model(upstream) {
                Some(model) if model.pricing.is_billable() => Ok(()),
                Some(model) => Err(GatewayProfileError::RouteModelUnpriced {
                    route: route_id,
                    model: model.id.as_str().to_owned(),
                }),
                None => Err(GatewayProfileError::RouteReachesNoPricedModel {
                    route: route_id,
                    pattern: route.model_pattern.clone(),
                    provider: route.provider.as_str().to_owned(),
                }),
            };
        }
        let mut reached = 0usize;
        for model in entry.models.iter().filter(|m| route.matches(m.id.as_str())) {
            reached += 1;
            if !model.pricing.is_billable() {
                return Err(GatewayProfileError::RouteModelUnpriced {
                    route: route_id,
                    model: model.id.as_str().to_owned(),
                });
            }
        }
        if reached == 0 {
            return Err(GatewayProfileError::RouteReachesNoPricedModel {
                route: route_id,
                pattern: route.model_pattern.clone(),
                provider: route.provider.as_str().to_owned(),
            });
        }
        Ok(())
    }

    #[must_use]
    pub fn to_spec(&self) -> GatewayConfigSpec {
        GatewayConfigSpec {
            enabled: self.enabled,
            routes: self.routes.clone(),
            default_provider: self.default_provider.clone(),
            allow_unlisted_models: self.allow_unlisted_models,
            auth_scheme: self.auth_scheme.clone(),
            inference_path_prefix: self.inference_path_prefix.clone(),
            system_prompt_overrides: self.system_prompt_overrides.clone(),
            bridge_releases: self.bridge_releases.clone(),
        }
    }
}
