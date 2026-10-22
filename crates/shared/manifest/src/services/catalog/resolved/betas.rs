//! Following `accepted_betas: inherit:<endpoint>` within one provider.
//!
//! An endpoint either states its list, inherits a sibling's, or states
//! nothing (`None`: the gateway forwards no beta flag it was not told the
//! upstream accepts). An inherit chain must end at a stated list or at an
//! endpoint with none, and must not loop.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

use systemprompt_identifiers::{EndpointId, ProviderId};
use systemprompt_wire::anthropic::AnthropicBeta;

use crate::services::catalog::endpoint::BetaSpec;
use crate::services::catalog::error::{CatalogError, CatalogResult};
use crate::services::catalog::providers::ProviderConfig;

pub(super) type EndpointBetas = BTreeMap<EndpointId, Option<BTreeSet<AnthropicBeta>>>;

pub(super) fn resolve_provider_betas(
    provider_id: &ProviderId,
    provider: &ProviderConfig,
) -> CatalogResult<EndpointBetas> {
    provider
        .endpoints
        .keys()
        .map(|endpoint| {
            resolve_one(provider_id, provider, endpoint).map(|betas| (endpoint.clone(), betas))
        })
        .collect()
}

fn resolve_one(
    provider_id: &ProviderId,
    provider: &ProviderConfig,
    start: &EndpointId,
) -> CatalogResult<Option<BTreeSet<AnthropicBeta>>> {
    let mut visited: BTreeSet<&EndpointId> = BTreeSet::new();
    let mut current = start;
    loop {
        if !visited.insert(current) {
            return Err(CatalogError::CyclicBetaInherit {
                provider: provider_id.clone(),
                endpoint: start.clone(),
            });
        }
        let Some(spec) = provider
            .endpoints
            .get(current)
            .and_then(|e| e.accepted_betas.as_ref())
        else {
            return Ok(None);
        };
        let target = match spec {
            BetaSpec::List(list) => return Ok(Some(list.clone())),
            BetaSpec::Inherit(_) => spec.inherited_endpoint(),
        };
        match target {
            Some(Ok(target)) => {
                let Some((key, _)) = provider.endpoints.get_key_value(target.as_str()) else {
                    return Err(CatalogError::UnknownBetaInherit {
                        provider: provider_id.clone(),
                        endpoint: current.clone(),
                        target,
                    });
                };
                current = key;
            },
            Some(Err(raw)) => {
                return Err(CatalogError::MalformedBetaInherit {
                    provider: provider_id.clone(),
                    endpoint: current.clone(),
                    value: raw.to_owned(),
                });
            },
            None => return Ok(None),
        }
    }
}
