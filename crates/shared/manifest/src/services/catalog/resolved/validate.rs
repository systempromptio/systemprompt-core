//! The catalog's validation rules.
//!
//! [`validate_registries`] checks the four registries on their own and
//! against each other: credentials are well-formed, every reference resolves,
//! endpoints are SSRF-guarded, a personal credential belongs to one provider,
//! every model is deployed and every deployment is of a declared model.
//! [`validate_routes`] runs on the projection: the default model, route
//! aliases, route narrowing, governance and, with the gateway enabled, that
//! every hop is priced.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, HashSet};

use systemprompt_identifiers::{CredentialId, ModelId, ProviderId};
use systemprompt_models::providers::has_context_variant;
use systemprompt_wire::hosting::names_a_project_literally;

use super::ResolvedCatalog;
use super::build::CatalogInput;
use crate::services::catalog::credentials::CredentialKind;
use crate::services::catalog::error::{CatalogError, CatalogResult};
use crate::services::catalog::headers;
use crate::services::catalog::providers::ProviderConfig;

pub(super) fn validate_registries(input: &CatalogInput<'_>) -> CatalogResult<()> {
    for (id, credential) in input.credentials.iter() {
        match (credential.kind, credential.secret.as_ref()) {
            (CredentialKind::Platform, None) => {
                return Err(CatalogError::PlatformCredentialWithoutSecret {
                    credential: id.clone(),
                });
            },
            (CredentialKind::Personal, Some(secret)) => {
                return Err(CatalogError::PersonalCredentialWithSecret {
                    credential: id.clone(),
                    secret: secret.as_str().to_owned(),
                });
            },
            _ => {},
        }
    }
    let trusted = systemprompt_models::net::trusted_http_hosts_from_env();
    let mut personal_owner: BTreeMap<&CredentialId, &ProviderId> = BTreeMap::new();
    for (id, provider) in input.providers.iter() {
        let Some(credential) = input.credentials.get(provider.credential.as_str()) else {
            return Err(CatalogError::UnknownCredential {
                provider: id.clone(),
                credential: provider.credential.clone(),
            });
        };
        if credential.kind.is_personal()
            && let Some(first) = personal_owner.insert(&provider.credential, id)
        {
            return Err(CatalogError::PersonalCredentialShared {
                credential: provider.credential.clone(),
                first: first.clone(),
                second: id.clone(),
            });
        }
        validate_provider(id, provider, &trusted)?;
    }
    validate_models_and_deployments(input)
}

fn validate_provider(
    id: &ProviderId,
    provider: &ProviderConfig,
    trusted: &[String],
) -> CatalogResult<()> {
    if provider.endpoints.is_empty() {
        return Err(CatalogError::ProviderWithoutEndpoints {
            provider: id.clone(),
        });
    }
    let reserved = headers::reserved_name(provider.extra_headers.keys())
        .or_else(|| headers::reserved_name(provider.forward_headers.iter()));
    if let Some(header) = reserved {
        return Err(CatalogError::ReservedHeader {
            provider: id.clone(),
            header: header.to_owned(),
        });
    }
    for (endpoint_id, endpoint) in &provider.endpoints {
        if endpoint.url.trim().is_empty() {
            return Err(CatalogError::EmptyEndpointUrl {
                provider: id.clone(),
                endpoint: endpoint_id.clone(),
            });
        }
        systemprompt_models::net::validate_outbound_url_with_trust(&endpoint.url, trusted)
            .map_err(|source| CatalogError::BlockedEndpointUrl {
                provider: id.clone(),
                endpoint: endpoint_id.clone(),
                url: endpoint.url.clone(),
                source,
            })?;
        if names_a_project_literally(&endpoint.url) {
            return Err(CatalogError::LiteralProjectInUrl {
                provider: id.clone(),
                endpoint: endpoint_id.clone(),
                url: endpoint.url.clone(),
            });
        }
        if endpoint
            .price_multiplier
            .is_some_and(|factor| !factor.is_finite() || factor <= 0.0)
        {
            return Err(CatalogError::InvalidPriceMultiplier {
                provider: id.clone(),
                endpoint: endpoint_id.clone(),
            });
        }
    }
    Ok(())
}

fn validate_models_and_deployments(input: &CatalogInput<'_>) -> CatalogResult<()> {
    for (id, _) in input.models.iter() {
        if id.as_str().trim().is_empty() {
            return Err(CatalogError::EmptyModelId);
        }
        if has_context_variant(id.as_str()) {
            return Err(CatalogError::ContextVariantModelId { model: id.clone() });
        }
        if input
            .deployments
            .get(id.as_str())
            .is_none_or(<[_]>::is_empty)
        {
            return Err(CatalogError::ModelWithoutDeployment { model: id.clone() });
        }
    }
    for (model, targets) in input.deployments.iter() {
        if input.models.get(model.as_str()).is_none() {
            return Err(CatalogError::DeploymentForUnknownModel {
                model: model.clone(),
            });
        }
        let mut seen = HashSet::with_capacity(targets.len());
        for target in targets {
            let endpoint = target.endpoint();
            let declared = input
                .providers
                .get(endpoint.provider.as_str())
                .and_then(|p| p.endpoint(endpoint.endpoint.as_str()))
                .is_some();
            if !declared {
                return Err(CatalogError::UnknownEndpoint {
                    model: model.clone(),
                    endpoint: endpoint.to_string(),
                });
            }
            if !seen.insert(endpoint) {
                return Err(CatalogError::DuplicateDeployment {
                    model: model.clone(),
                    endpoint: endpoint.to_string(),
                });
            }
        }
    }
    Ok(())
}

pub(super) fn validate_routes(catalog: &ResolvedCatalog) -> CatalogResult<()> {
    if let Some(model) = catalog.default_model.as_ref() {
        if !catalog.models.contains_key(model) {
            return Err(CatalogError::UnknownDefaultModel {
                model: model.clone(),
            });
        }
        if catalog.enabled_hops(model.as_str()).next().is_none() {
            return Err(CatalogError::DefaultModelNotServable {
                model: model.clone(),
            });
        }
    }
    let mut aliases: HashSet<&ModelId> = HashSet::new();
    for route in &catalog.routes {
        let mut matched: Vec<&ModelId> = Vec::new();
        if let Some(alias) = route.alias.as_ref() {
            if !catalog.models.contains_key(&alias.model) {
                return Err(CatalogError::UnknownAliasTarget {
                    route: route.id.clone(),
                    model: alias.model.clone(),
                });
            }
            if catalog.models.contains_key(&alias.requested) || !aliases.insert(&alias.requested) {
                return Err(CatalogError::AliasShadowsModel {
                    route: route.id.clone(),
                    requested: alias.requested.clone(),
                });
            }
            matched.push(&alias.model);
        }
        if !route.models.is_empty() {
            let before = matched.len();
            matched.extend(
                catalog
                    .models
                    .keys()
                    .filter(|id| route.matches_model(id.as_str())),
            );
            if matched.len() == before {
                return Err(CatalogError::RouteMatchesNoModel {
                    route: route.id.clone(),
                });
            }
        }
        if let Some(fallback) = route
            .context_fallbacks
            .iter()
            .find(|m| !catalog.models.contains_key(*m))
        {
            return Err(CatalogError::UnknownContextFallback {
                route: route.id.clone(),
                model: fallback.clone(),
            });
        }
        for model in matched {
            let chain = catalog.route_chain(route, model)?;
            super::validate_hops::check_route_governance(route, &chain)?;
        }
    }
    if catalog.gateway_enabled {
        for hops in catalog.deployments.values() {
            super::validate_hops::check_pricing(hops)?;
        }
    }
    Ok(())
}
