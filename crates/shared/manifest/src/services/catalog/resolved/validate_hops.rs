//! Per-hop checks: a route's governance demands, that every hop the gateway
//! can dispatch is priced with a declared cache-read rate, and that
//! `ai.default_image_model` is a deployed image-generation model.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::ResolvedCatalog;
use super::types::ResolvedHop;
use crate::services::catalog::error::{CatalogError, CatalogResult};
use crate::services::gateway::GatewayRoute;

pub(super) fn check_route_governance(
    route: &GatewayRoute,
    chain: &[ResolvedHop],
) -> CatalogResult<()> {
    let Some(requires) = route.requires.as_ref() else {
        return Ok(());
    };
    for hop in chain {
        let unmet = requires.unmet(hop.governance);
        if !unmet.is_empty() {
            return Err(CatalogError::RouteGovernanceUnsatisfied {
                route: route.id.clone(),
                model: hop.model.clone(),
                endpoint: hop.endpoint_ref().to_string(),
                requirements: unmet.join(","),
            });
        }
    }
    Ok(())
}

pub(super) fn check_pricing(hops: &[ResolvedHop]) -> CatalogResult<()> {
    for hop in hops {
        if !hop.pricing.is_billable() {
            return Err(CatalogError::UnpricedDeployment {
                model: hop.model.clone(),
                endpoint: hop.endpoint_ref().to_string(),
            });
        }
        let token_priced =
            hop.pricing.input_per_million > 0.0 || hop.pricing.output_per_million > 0.0;
        if token_priced && !hop.pricing.declares_cache_rate() {
            return Err(CatalogError::CacheRateUndeclared {
                model: hop.model.clone(),
                endpoint: hop.endpoint_ref().to_string(),
            });
        }
    }
    Ok(())
}

pub(super) fn check_default_image_model(catalog: &ResolvedCatalog) -> CatalogResult<()> {
    let Some(model) = catalog.default_image_model.as_ref() else {
        return Ok(());
    };
    let Some(resolved) = catalog.models.get(model) else {
        return Err(CatalogError::UnknownDefaultImageModel {
            model: model.clone(),
        });
    };
    if resolved.capabilities.image_generation {
        Ok(())
    } else {
        Err(CatalogError::DefaultImageModelNotImageCapable {
            model: model.clone(),
        })
    }
}
