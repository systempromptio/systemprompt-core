//! Validation of a route's deployment chains.
//!
//! Every deployment of every chain is validated as the route that deployment
//! will actually serve, so a failover or a per-scope chain can never land on
//! a model the primary route's pricing and governance checks would have
//! refused at boot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use super::validate::validate_route_governance;
use crate::services::gateway::config::GatewayConfig;
use crate::services::gateway::error::{GatewayProfileError, GatewayResult};
use crate::services::gateway::route::GatewayRoute;
use crate::services::providers::ProviderRegistry;

impl GatewayConfig {
    pub(super) fn validate_route_chains(
        &self,
        registry: &ProviderRegistry,
        route: &GatewayRoute,
    ) -> GatewayResult<()> {
        let route_id = route.effective_id().to_string();
        if let Some(scoped) = route.by_scope.as_ref() {
            if scoped.chains.is_empty() {
                return Err(GatewayProfileError::RouteScopeChainsEmpty { route: route_id });
            }
            if scoped.chains.keys().any(|key| key.trim().is_empty()) {
                return Err(GatewayProfileError::RouteScopeKeyEmpty { route: route_id });
            }
        }
        for (scope, views) in route.all_chains() {
            let mut seen = HashSet::with_capacity(views.len());
            for view in &views {
                let provider = view.provider.as_str().to_owned();
                if !seen.insert(view.provider.clone()) {
                    return Err(GatewayProfileError::RouteDeploymentDuplicate {
                        route: route_id,
                        chain: scope.map_or_else(|| "route".to_owned(), str::to_owned),
                        provider,
                    });
                }
                if view.resolve(registry).is_none() {
                    return Err(match scope {
                        None => GatewayProfileError::RouteDeploymentProviderNotInRegistry {
                            route: route_id,
                            provider,
                        },
                        Some(scope) => GatewayProfileError::RouteScopeChainProviderNotInRegistry {
                            route: route_id,
                            scope: scope.to_owned(),
                            provider,
                        },
                    });
                }
                self.validate_route_pricing(registry, view)?;
                validate_route_governance(registry, view)?;
            }
        }
        Ok(())
    }
}
