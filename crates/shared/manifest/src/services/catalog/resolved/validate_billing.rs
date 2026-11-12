//! Member-paid credential checks: a caller credential's provider carries the
//! client's request unchanged, and every deployment a request for one model
//! can reach bills one account.
//!
//! A model's enabled deployments are what a chain, a pinned route, a route
//! selector or an alias can dispatch, so the rule is checked per model and
//! holds for every subset. A route's `context_fallbacks` are checked against
//! each model the route serves.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ModelId, ProviderId};

use super::ResolvedCatalog;
use super::types::ResolvedHop;
use crate::services::catalog::billing_error::BillingRuleError;
use crate::services::catalog::error::{CatalogError, CatalogResult};
use crate::services::catalog::providers::ProviderConfig;
use crate::services::catalog::spoken_wire::is_spoken_by_a_client;
use crate::services::gateway::GatewayRoute;

pub(super) fn check_caller_provider(
    id: &ProviderId,
    provider: &ProviderConfig,
) -> CatalogResult<()> {
    if provider.user_agent.is_some()
        || !provider.extra_headers.is_empty()
        || !provider.forward_headers.is_empty()
    {
        return Err(CatalogError::CallerProviderSetsHeaders {
            credential: provider.credential.clone(),
            provider: id.clone(),
        });
    }
    for (endpoint_id, endpoint) in &provider.endpoints {
        if endpoint.effective_hosting() != systemprompt_wire::Hosting::FirstParty {
            return Err(CatalogError::CallerCredentialNotFirstParty {
                credential: provider.credential.clone(),
                provider: id.clone(),
                endpoint: endpoint_id.clone(),
            });
        }
        if !is_spoken_by_a_client(endpoint.wire) {
            return Err(BillingRuleError::CallerWireRewritesRequest {
                credential: provider.credential.clone(),
                provider: id.clone(),
                endpoint: endpoint_id.clone(),
                wire: endpoint.wire,
            }
            .into());
        }
        if endpoint.accepted_betas.is_some() {
            return Err(CatalogError::CallerCredentialFiltersBetas {
                credential: provider.credential.clone(),
                provider: id.clone(),
                endpoint: endpoint_id.clone(),
            });
        }
    }
    Ok(())
}

pub(super) fn check_model_chains(catalog: &ResolvedCatalog) -> Result<(), BillingRuleError> {
    for model in catalog.models.keys() {
        let mut hops = catalog.enabled_hops(model.as_str());
        let Some(first) = hops.next() else {
            continue;
        };
        if let Some(second) = hops.find(|hop| !first.may_fail_over_to(hop)) {
            return Err(BillingRuleError::MixedChain {
                model: model.clone(),
                first: first.endpoint_ref().to_string(),
                first_credential: first.credential.id.clone(),
                second: second.endpoint_ref().to_string(),
                second_credential: second.credential.id.clone(),
            });
        }
    }
    Ok(())
}

pub(super) fn check_context_fallbacks(
    catalog: &ResolvedCatalog,
    route: &GatewayRoute,
    served: &[&ModelId],
) -> Result<(), BillingRuleError> {
    for model in served {
        let Some(primary) = catalog.enabled_hops(model.as_str()).next() else {
            continue;
        };
        for fallback in &route.context_fallbacks {
            let mixed = catalog
                .enabled_hops(fallback.as_str())
                .find(|hop: &&ResolvedHop| !primary.may_fail_over_to(hop));
            if let Some(hop) = mixed {
                return Err(BillingRuleError::MixedContextFallback {
                    route: route.id.clone(),
                    model: (*model).clone(),
                    credential: primary.credential.id.clone(),
                    fallback: fallback.clone(),
                    fallback_credential: hop.credential.id.clone(),
                });
            }
        }
    }
    Ok(())
}
