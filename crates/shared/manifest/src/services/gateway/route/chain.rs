//! Deployment chains of a gateway route.
//!
//! A chain is an ordered list of deployments (a provider entry, optionally
//! under its own upstream model). The route chain is the route's `provider`
//! followed by its `fallbacks`. `by_scope` names one scope dimension and maps
//! the values a request can be attributed to in it onto their own chains: a
//! request with no value for the dimension takes the route chain, a mapped
//! value takes its chain, and an unmapped value is refused unless
//! `unmapped: shared` sends it down the route chain.
//!
//! Every chain is expanded into route *views* by
//! [`GatewayRoute::chain_views`]: the route as each deployment's provider
//! sees it, carrying the route id unchanged so pricing, governance and
//! access-control checks address the same route whichever deployment serves.
//! Each view carries its deployment's `weight` and the chain's `strategy`, so
//! the gateway plans the attempt order from the views alone.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ProviderId, ScopeDimension};
use systemprompt_models::attribution::RequestAttribution;

use super::{GatewayRoute, SelectionStrategy};

/// One deployment in a chain after the primary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RouteDeployment {
    pub provider: ProviderId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<u32>,
}

/// The chain a request attributed to one scope value is served by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScopeChain {
    pub provider: ProviderId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_model: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallbacks: Vec<RouteDeployment>,
    #[serde(default, skip_serializing_if = "SelectionStrategy::is_ordered")]
    pub strategy: SelectionStrategy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context_fallbacks: Vec<RouteDeployment>,
}

/// What a request attributed to a value absent from `chains` is served by.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum UnmappedScope {
    #[default]
    Deny,
    Shared,
}

/// Per-scope chains of a route, keyed by the value attributed in `dimension`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RouteScopeChains {
    pub dimension: ScopeDimension,
    #[serde(default)]
    pub chains: BTreeMap<String, ScopeChain>,
    #[serde(default)]
    pub unmapped: UnmappedScope,
}

/// The chain [`GatewayRoute::chain_for`] selected for one request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainSelection<'a> {
    Route,
    Scope {
        dimension: &'a ScopeDimension,
        value: &'a str,
        chain: &'a ScopeChain,
    },
    Unmapped {
        dimension: &'a ScopeDimension,
        value: &'a str,
    },
}

impl ChainSelection<'_> {
    #[must_use]
    pub fn descriptor(&self) -> Option<String> {
        match self {
            Self::Route => None,
            Self::Scope {
                dimension, value, ..
            }
            | Self::Unmapped { dimension, value } => Some(format!("scope:{dimension}={value}")),
        }
    }
}

impl GatewayRoute {
    #[must_use]
    pub fn chain_for<'a>(&'a self, attribution: &'a RequestAttribution) -> ChainSelection<'a> {
        let Some(scoped) = self.by_scope.as_ref() else {
            return ChainSelection::Route;
        };
        let Some(value) = attribution.value_for(scoped.dimension.as_str()) else {
            return ChainSelection::Route;
        };
        match scoped.chains.get_key_value(value) {
            Some((key, chain)) => ChainSelection::Scope {
                dimension: &scoped.dimension,
                value: key.as_str(),
                chain,
            },
            None if scoped.unmapped == UnmappedScope::Shared => ChainSelection::Route,
            None => ChainSelection::Unmapped {
                dimension: &scoped.dimension,
                value,
            },
        }
    }

    #[must_use]
    pub fn deployment_views(&self, attribution: &RequestAttribution) -> Vec<Self> {
        self.chain_views(&self.chain_for(attribution))
    }

    #[must_use]
    pub fn chain_views(&self, selection: &ChainSelection<'_>) -> Vec<Self> {
        match selection {
            ChainSelection::Unmapped { .. } => Vec::new(),
            ChainSelection::Route => {
                let mut views = vec![Self {
                    id: Some(self.effective_id()),
                    fallbacks: Vec::new(),
                    by_scope: None,
                    context_fallbacks: Vec::new(),
                    ..self.clone()
                }];
                views.extend(
                    self.fallbacks
                        .iter()
                        .map(|d| self.view_of(d, self.strategy)),
                );
                views
            },
            ChainSelection::Scope { chain, .. } => {
                let primary = RouteDeployment {
                    provider: chain.provider.clone(),
                    upstream_model: chain.upstream_model.clone(),
                    weight: chain.weight,
                };
                let mut views = vec![self.view_of(&primary, chain.strategy)];
                views.extend(
                    chain
                        .fallbacks
                        .iter()
                        .map(|d| self.view_of(d, chain.strategy)),
                );
                views
            },
        }
    }

    #[must_use]
    pub fn all_chains(&self) -> Vec<(Option<&str>, Vec<Self>)> {
        let mut out = vec![(None, self.chain_views(&ChainSelection::Route))];
        if let Some(scoped) = self.by_scope.as_ref() {
            for (value, chain) in &scoped.chains {
                let selection = ChainSelection::Scope {
                    dimension: &scoped.dimension,
                    value: value.as_str(),
                    chain,
                };
                out.push((Some(value.as_str()), self.chain_views(&selection)));
            }
        }
        out
    }

    pub(super) fn view_of(
        &self,
        deployment: &RouteDeployment,
        strategy: SelectionStrategy,
    ) -> Self {
        Self {
            id: Some(self.effective_id()),
            provider: deployment.provider.clone(),
            upstream_model: deployment.upstream_model.clone(),
            pricing: None,
            fallbacks: Vec::new(),
            by_scope: None,
            strategy,
            weight: deployment.weight,
            context_fallbacks: Vec::new(),
            ..self.clone()
        }
    }
}
