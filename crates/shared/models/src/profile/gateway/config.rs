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
    /// Authorizes the synthetic catch-all route, but a model is only
    /// *dispatched* to it when [`Self::allow_unlisted_models`] is also set; see
    /// [`GatewayConfig::is_model_exposed`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_provider: Option<ProviderId>,
    /// Closed allowlist when `false` (the default): a model matching no route
    /// and absent from the registry is denied (`403`) rather than silently
    /// billed against `default_provider`. Set `true` only to let the default
    /// provider absorb arbitrary model strings.
    #[serde(default)]
    pub allow_unlisted_models: bool,
    #[serde(default = "default_auth_scheme")]
    pub auth_scheme: String,
    #[serde(default = "default_inference_path_prefix")]
    pub inference_path_prefix: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub system_prompt_overrides: Vec<SystemPromptRule>,
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
        } = self;

        GatewayConfig {
            enabled,
            routes,
            default_provider,
            allow_unlisted_models,
            auth_scheme,
            inference_path_prefix,
            system_prompt_overrides,
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

    /// Selects the first candidate route whose model glob **and** request-shape
    /// predicates match. A route without a `when` block matches on model name
    /// alone, so omitting predicates preserves the prior model-only behaviour.
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

    /// `upstream_model` is left `None` **by design**: the `*` catch-all route
    /// forwards the requested model name to `default_provider` verbatim.
    /// Per-model rewrites live in the registry, applied downstream. Which
    /// names may reach this route at all is governed by the closed allowlist
    /// in [`Self::is_model_exposed`]; a profile that opts into
    /// `allow_unlisted_models` accepts that unlisted names are not validated
    /// at the gateway — the upstream provider's error is the rejection signal.
    /// Shipped profiles keep the opt-in off.
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

    /// Closed-allowlist posture: a model matching no explicit route and not a
    /// registered provider model is dispatchable only when `default_provider`
    /// is set **and** [`Self::allow_unlisted_models`] opts in. Otherwise it
    /// is denied before dispatch rather than silently billed.
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
        }
    }
}
