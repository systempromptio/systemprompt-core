//! Cross-checks of gateway routes against the provider registry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::profile::gateway::config::GatewayConfig;
use crate::profile::gateway::error::{GatewayProfileError, GatewayResult};
use crate::profile::gateway::route::GatewayRoute;
use crate::profile::providers::ProviderRegistry;

impl GatewayConfig {
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
            validate_route_governance(registry, route)?;
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
}

fn validate_route_governance(
    registry: &ProviderRegistry,
    route: &GatewayRoute,
) -> GatewayResult<()> {
    let Some(requires) = route.requires.as_ref() else {
        return Ok(());
    };
    if requires.declared().is_empty() {
        return Ok(());
    }
    let Some(entry) = route.resolve(registry) else {
        return Ok(());
    };
    let check = |model_id: &str| -> GatewayResult<()> {
        let unmet = requires.unmet(entry.effective_governance(model_id));
        if unmet.is_empty() {
            Ok(())
        } else {
            Err(GatewayProfileError::RouteGovernanceUnsatisfied {
                route: route.id.as_str().to_owned(),
                model: model_id.to_owned(),
                requirements: unmet.join(","),
            })
        }
    };
    if let Some(upstream) = route.upstream_model.as_deref() {
        return check(upstream);
    }
    for model in entry.models.iter().filter(|m| route.matches(m.id.as_str())) {
        check(model.id.as_str())?;
    }
    Ok(())
}
