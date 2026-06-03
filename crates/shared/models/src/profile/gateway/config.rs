//! Gateway configuration: on-disk spec and resolved runtime form.
//!
//! [`GatewayConfigSpec`] is the serde shape accepted under `gateway:` in a
//! profile; [`GatewayConfig`] is its runtime projection. Routes carry no
//! embedded provider catalog — every route resolves its provider against
//! `profile.providers` ([`ProviderRegistry`]) at use time.

use std::borrow::Cow;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ProviderId, RouteId};

use super::super::providers::ProviderRegistry;
use super::error::{GatewayProfileError, GatewayResult};
use super::route::GatewayRoute;

pub(crate) const DEFAULT_ROUTE_PATTERN: &str = "*";

/// On-disk gateway configuration: the exact shape accepted under
/// `gateway:` in a profile YAML document.
///
/// Project to the runtime [`GatewayConfig`] via [`Self::resolve`].
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatewayConfigSpec {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub routes: Vec<GatewayRoute>,
    /// Provider that absorbs any model not matched by an explicit `route`.
    /// When set, the gateway stops being a closed allowlist: an unmatched
    /// model is forwarded to this provider instead of denied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_provider: Option<ProviderId>,
    #[serde(default = "default_auth_scheme")]
    pub auth_scheme: String,
    #[serde(default = "default_inference_path_prefix")]
    pub inference_path_prefix: String,
}

impl Default for GatewayConfigSpec {
    fn default() -> Self {
        Self {
            enabled: false,
            routes: Vec::new(),
            default_provider: None,
            auth_scheme: default_auth_scheme(),
            inference_path_prefix: default_inference_path_prefix(),
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
    /// A pure field map: the gateway owns no catalog file, so resolution
    /// performs no I/O.
    #[must_use]
    pub fn resolve(self) -> GatewayConfig {
        let Self {
            enabled,
            routes,
            default_provider,
            auth_scheme,
            inference_path_prefix,
        } = self;

        GatewayConfig {
            enabled,
            routes,
            default_provider,
            auth_scheme,
            inference_path_prefix,
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
    pub auth_scheme: String,
    pub inference_path_prefix: String,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            routes: Vec::new(),
            default_provider: None,
            auth_scheme: default_auth_scheme(),
            inference_path_prefix: default_inference_path_prefix(),
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
        model: &str,
    ) -> Option<Cow<'a, GatewayRoute>> {
        self.candidate_routes(registry)
            .find(|route| route.matches(model))
    }

    #[must_use]
    pub fn dispatchable_route_ids(&self, registry: &ProviderRegistry) -> Vec<RouteId> {
        let mut ids: Vec<RouteId> = Vec::new();
        for route in self.candidate_routes(registry) {
            let mut route = route.into_owned();
            route.ensure_id();
            if !ids.contains(&route.id) {
                ids.push(route.id);
            }
        }
        ids
    }

    /// A catch-all route to [`Self::default_provider`], gated on the provider
    /// existing in `registry`. `upstream_model` is left `None` so the requested
    /// model name passes through unchanged; per-model upstream rewrites live in
    /// the registry and are applied downstream.
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
        };
        route.ensure_id();
        Some(route)
    }

    #[must_use]
    pub fn is_model_exposed(&self, registry: &ProviderRegistry, model: &str) -> bool {
        self.default_provider.is_some()
            || self.find_route(model).is_some()
            || registry.contains_model(model)
    }

    /// Validate the gateway's references into `registry`: route-id uniqueness,
    /// and that `default_provider` (if set) and every route provider resolve to
    /// a registry entry. The registry validates its own models separately.
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
        if let Some(provider) = self.default_provider.as_ref() {
            if registry.find_provider(provider.as_str()).is_none() {
                return Err(GatewayProfileError::DefaultProviderNotInRegistry {
                    provider: provider.as_str().to_owned(),
                });
            }
        }
        for route in &self.routes {
            if registry.find_provider(route.provider.as_str()).is_none() {
                return Err(GatewayProfileError::RouteProviderNotInRegistry {
                    route: route.model_pattern.clone(),
                    provider: route.provider.as_str().to_owned(),
                });
            }
        }
        Ok(())
    }

    /// Round-trips a resolved config back to its on-disk spec for persisting a
    /// profile to YAML.
    #[must_use]
    pub fn to_spec(&self) -> GatewayConfigSpec {
        GatewayConfigSpec {
            enabled: self.enabled,
            routes: self.routes.clone(),
            default_provider: self.default_provider.clone(),
            auth_scheme: self.auth_scheme.clone(),
            inference_path_prefix: self.inference_path_prefix.clone(),
        }
    }
}
