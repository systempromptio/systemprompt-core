//! Replaying 0.72.1 dispatch over a legacy tree.
//!
//! For one requested id this is what the old gateway did with a plain request
//! (no tools, no scope attribution): the model must be exposed, the first
//! matching route wins (the `default_provider` catch-all last, unless the
//! model is hidden and `hidden_requires_route` holds), and the route's chain
//! is its primary plus `fallbacks`, each hop resolving its upstream name on its
//! own entry and its price from the route override or the entry's served
//! model. With no gateway the first entry declaring the model serves it.
//! [`super::models`] turns these hops into deployments, so a migrated catalog
//! dispatches every id the way 0.72.1 did.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ModelId;
use systemprompt_manifest::services::ModelPricing;
use systemprompt_manifest::services::gateway::SelectionStrategy;
use systemprompt_wire::canonical::CanonicalRequest;

use super::legacy::{LegacyDeployment, LegacyGateway, LegacyProvider, LegacyRoute};

pub(super) const DEFAULT_ROUTE_PATTERN: &str = "*";

#[derive(Debug, Clone)]
pub struct LegacyTree {
    pub providers: Vec<LegacyProvider>,
    pub gateway: Option<LegacyGateway>,
}

/// One hop of a replayed chain.
#[derive(Debug, Clone, PartialEq)]
pub struct LegacyHop {
    pub provider: String,
    pub upstream_model: String,
    pub pricing: Option<ModelPricing>,
    pub weight: Option<u32>,
}

/// What one requested id dispatched to, and the route that decided it.
#[derive(Debug, Clone)]
pub struct LegacyResolution {
    pub route_index: Option<usize>,
    pub chain: Vec<LegacyHop>,
    pub strategy: SelectionStrategy,
}

impl LegacyTree {
    pub fn provider(&self, name: &str) -> Option<&LegacyProvider> {
        self.providers.iter().find(|p| p.name == name)
    }

    fn contains_model(&self, requested: &str) -> bool {
        self.providers
            .iter()
            .any(|p| p.find_model(requested).is_some())
    }

    fn is_hidden(&self, requested: &str) -> bool {
        self.providers
            .iter()
            .filter_map(|p| p.find_model(requested))
            .any(|m| m.hidden)
    }

    fn default_route(&self, gateway: &LegacyGateway) -> Option<LegacyRoute> {
        let provider = gateway.default_provider.as_deref()?;
        self.provider(provider)?;
        Some(LegacyRoute {
            id: None,
            name: None,
            description: None,
            model_pattern: DEFAULT_ROUTE_PATTERN.to_owned(),
            provider: provider.to_owned(),
            upstream_model: None,
            extra_headers: std::collections::HashMap::new(),
            pricing: None,
            when: None,
            requires: None,
            fallbacks: Vec::new(),
            by_scope: None,
            strategy: SelectionStrategy::Ordered,
            weight: None,
            context_fallbacks: Vec::new(),
        })
    }

    #[must_use]
    pub fn catch_all_route_id(&self) -> Option<String> {
        let gateway = self.gateway.as_ref()?;
        gateway
            .catalog_route_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
            .or_else(|| self.default_route(gateway).map(|r| r.effective_id()))
    }

    fn exposed(&self, gateway: &LegacyGateway, requested: &str) -> bool {
        let hidden_gate = gateway.hidden_requires_route && self.is_hidden(requested);
        gateway.routes.iter().any(|r| r.matches(requested))
            || (!hidden_gate && self.contains_model(requested))
    }

    #[must_use]
    pub fn resolve(&self, requested: &str) -> Option<LegacyResolution> {
        let Some(gateway) = self.gateway.as_ref() else {
            let provider = self
                .providers
                .iter()
                .find(|p| p.find_model(requested).is_some())?;
            let primary = LegacyDeployment {
                provider: provider.name.clone(),
                upstream_model: None,
                weight: None,
            };
            let hop = self.hop(&primary, None, requested)?;
            return Some(LegacyResolution {
                route_index: None,
                chain: vec![hop],
                strategy: SelectionStrategy::Ordered,
            });
        };
        if !self.exposed(gateway, requested) {
            return None;
        }
        let hidden_gate = gateway.hidden_requires_route && self.is_hidden(requested);
        let plain = CanonicalRequest::new(ModelId::new(requested), Vec::new(), 1);
        let declared = gateway.routes.iter().enumerate().find(|(_, r)| {
            r.matches(requested)
                && r.when
                    .as_ref()
                    .is_none_or(|w| w.matches(None, Some(&plain)))
        });
        let (route_index, route) = match declared {
            Some((index, route)) => (Some(index), route.clone()),
            None if hidden_gate => return None,
            None => (None, self.default_route(gateway)?),
        };
        let mut chain = Vec::new();
        let primary = LegacyDeployment {
            provider: route.provider.clone(),
            upstream_model: route.upstream_model.clone(),
            weight: route.weight,
        };
        chain.push(self.hop(&primary, route.pricing, requested)?);
        chain.extend(
            route
                .fallbacks
                .iter()
                .filter_map(|d| self.fallback(d, requested)),
        );
        Some(LegacyResolution {
            route_index,
            chain,
            strategy: route.strategy,
        })
    }

    pub fn fallback(&self, deployment: &LegacyDeployment, requested: &str) -> Option<LegacyHop> {
        self.hop(deployment, None, requested)
    }

    fn hop(
        &self,
        deployment: &LegacyDeployment,
        route_pricing: Option<ModelPricing>,
        requested: &str,
    ) -> Option<LegacyHop> {
        let entry = self.provider(&deployment.provider)?;
        let upstream_override = deployment.upstream_model.as_deref();
        let pricing = route_pricing.or_else(|| {
            entry
                .find_served_model(upstream_override.unwrap_or(requested))
                .map(|m| m.pricing)
        });
        Some(LegacyHop {
            provider: entry.name.clone(),
            upstream_model: entry.upstream_model_for(upstream_override, requested),
            pricing,
            weight: deployment.weight,
        })
    }
}
