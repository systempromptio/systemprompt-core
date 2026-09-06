//! Cross-checks of gateway routes against the provider registry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::ai::ModelPricing;
use crate::services::gateway::config::GatewayConfig;
use crate::services::gateway::error::{GatewayProfileError, GatewayResult};
use crate::services::gateway::route::GatewayRoute;
use crate::services::providers::ProviderRegistry;

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
        let provider = route.provider.as_str().to_owned();
        if let Some(pricing) = route.pricing {
            if !pricing.is_billable() {
                return Err(GatewayProfileError::RouteModelUnpriced {
                    route: route_id,
                    model: route.model_pattern.clone(),
                });
            }
            return check_cache_rate(&pricing, &route_id, &provider, &route.model_pattern);
        }
        let Some(entry) = route.resolve(registry) else {
            return Ok(());
        };
        if let Some(upstream) = route.upstream_model.as_deref() {
            return match entry.find_model(upstream) {
                Some(model) if model.pricing.is_billable() => {
                    check_cache_rate(&model.pricing, &route_id, &provider, model.id.as_str())
                },
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
            check_cache_rate(&model.pricing, &route_id, &provider, model.id.as_str())?;
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

// Why: every outbound wire the gateway speaks -- anthropic, openai-chat,
// openai-responses, gemini -- can report cached prompt tokens, and the
// canonical usage type keeps them out of `input_tokens`. A model with no
// declared cache rate therefore bills its cached slice at nothing, invisibly.
// An explicit 0.0 is a statement that the provider does not bill cache reads.
// A model billed only per image is exempt: it charges no token classes at all,
// so there is no cached slice for a missing rate to hide.
fn check_cache_rate(
    pricing: &ModelPricing,
    route: &str,
    provider: &str,
    model: &str,
) -> GatewayResult<()> {
    if pricing.input_per_million <= 0.0 && pricing.output_per_million <= 0.0 {
        return Ok(());
    }
    if pricing.declares_cache_rate() {
        Ok(())
    } else {
        Err(GatewayProfileError::RouteModelCacheRateUndeclared {
            route: route.to_owned(),
            provider: provider.to_owned(),
            model: model.to_owned(),
        })
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
