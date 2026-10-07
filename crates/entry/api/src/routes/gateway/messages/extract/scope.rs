//! Scope attribution for one gateway request.
//!
//! Every subject dimension a tenant registers through
//! `register_subject_attribute_provider!` is resolved per request, in order:
//!
//! 1. an `x-systemprompt-scope-<dimension>` header, whose value must be one of
//!    the provider's values for the caller (`403` otherwise — an unknown value
//!    and a value the caller does not hold answer the same, so the header
//!    cannot probe for existence);
//! 2. the API key's bound value for that dimension, verified the same way;
//! 3. the provider's first value (a tenant orders the primary first).
//!
//! A header naming a dimension no provider registers is `400`, and a dimension
//! listed in `gateway.require_scopes` that resolves to nothing is `400
//! scope_required`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderMap, StatusCode};
use systemprompt_identifiers::headers::SCOPE_PREFIX;
use systemprompt_identifiers::{ApiKeyId, ScopeDimension, UserId};
use systemprompt_models::attribution::{
    AttributionEntry, AttributionSource, RequestAttribution, ScopeBinding,
};
use systemprompt_security::authz::{SharedSubjectAttributeProvider, SubjectProviderSet};

use super::RejectionPartial;
use crate::routes::gateway::messages::RequestContext;
use crate::routes::gateway::messages::auth::AuthedPrincipal;
use crate::routes::gateway::messages::error::RejectionError;
use systemprompt_manifest::services::gateway::GatewayConfig;

/// The `x-systemprompt-scope-*` headers of one request, parsed before the
/// body is consumed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScopeHeaders(Vec<ScopeBinding>);

impl ScopeHeaders {
    pub fn capture(headers: &HeaderMap) -> Result<Self, RejectionError> {
        let mut requested: Vec<ScopeBinding> = Vec::new();
        for (name, value) in headers {
            let Some(raw) = name.as_str().strip_prefix(SCOPE_PREFIX) else {
                continue;
            };
            let dimension = ScopeDimension::try_new(raw)
                .map_err(|error| RejectionError::invalid(StatusCode::BAD_REQUEST, error))?;
            let value = value
                .to_str()
                .ok()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .ok_or_else(|| {
                    RejectionError::client(
                        StatusCode::BAD_REQUEST,
                        format!("{}{raw} must be a non-empty ASCII value", SCOPE_PREFIX),
                    )
                })?;
            if requested.iter().any(|b| b.dimension == dimension) {
                return Err(RejectionError::client(
                    StatusCode::BAD_REQUEST,
                    format!("{}{raw} appears more than once", SCOPE_PREFIX),
                ));
            }
            requested.push(ScopeBinding {
                dimension,
                value: value.to_owned(),
            });
        }
        Ok(Self(requested))
    }

    fn value_for(&self, dimension: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|b| b.dimension.as_str() == dimension)
            .map(|b| b.value.as_str())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ScopeResolution<'a> {
    pub providers: &'a SubjectProviderSet,
    pub user_id: &'a UserId,
    pub headers: &'a ScopeHeaders,
    pub key_bindings: &'a [ScopeBinding],
    pub api_key_id: Option<&'a ApiKeyId>,
    pub require: &'a [ScopeDimension],
}

pub async fn resolve_scope_attribution(
    input: ScopeResolution<'_>,
    partial: &mut RejectionPartial,
) -> Result<RequestAttribution, RejectionError> {
    partial.attribution.api_key_id = input.api_key_id.cloned();
    ensure_registered(&input)?;
    for provider in input.providers.iter() {
        let dimension = provider_dimension(provider)?;
        let claimed = claimed_value(&input, &dimension);
        let held = provider.values_for(input.user_id).await.map_err(|error| {
            RejectionError::server(
                StatusCode::INTERNAL_SERVER_ERROR,
                "scope attribution lookup failed",
            )
            .with_cause(error)
        })?;
        match claimed {
            Some((value, source)) => {
                let holds = held.iter().any(|h| h == value);
                partial.attribution.entries.push(AttributionEntry {
                    dimension: dimension.clone(),
                    value: value.to_owned(),
                    source,
                });
                if !holds {
                    return Err(RejectionError::client(
                        StatusCode::FORBIDDEN,
                        format!("not a member of {dimension} '{value}'"),
                    ));
                }
            },
            None => {
                if let Some(value) = held.into_iter().next() {
                    partial.attribution.entries.push(AttributionEntry {
                        dimension,
                        value,
                        source: AttributionSource::Default,
                    });
                }
            },
        }
    }
    ensure_required(input.require, &partial.attribution)?;
    Ok(partial.attribution.clone())
}

fn ensure_registered(input: &ScopeResolution<'_>) -> Result<(), RejectionError> {
    let unregistered = |b: &&ScopeBinding| input.providers.find(b.dimension.as_str()).is_none();
    if let Some(unknown) = input.headers.0.iter().find(unregistered) {
        return Err(RejectionError::client(
            StatusCode::BAD_REQUEST,
            format!(
                "unknown scope dimension '{}': no subject attribute provider registers it",
                unknown.dimension
            ),
        ));
    }
    if let Some(orphan) = input.key_bindings.iter().find(unregistered) {
        return Err(RejectionError::client(
            StatusCode::FORBIDDEN,
            format!(
                "API key is bound to scope dimension '{}', which no provider registers",
                orphan.dimension
            ),
        ));
    }
    Ok(())
}

fn provider_dimension(
    provider: &SharedSubjectAttributeProvider,
) -> Result<ScopeDimension, RejectionError> {
    ScopeDimension::try_new(provider.dimension().rule_type.as_str()).map_err(|error| {
        RejectionError::server(
            StatusCode::INTERNAL_SERVER_ERROR,
            "subject dimension is not a valid scope dimension",
        )
        .with_cause(error)
    })
}

fn claimed_value<'a>(
    input: &ScopeResolution<'a>,
    dimension: &ScopeDimension,
) -> Option<(&'a str, AttributionSource)> {
    input
        .headers
        .value_for(dimension.as_str())
        .map(|v| (v, AttributionSource::Header))
        .or_else(|| {
            input
                .key_bindings
                .iter()
                .find(|b| &b.dimension == dimension)
                .map(|b| (b.value.as_str(), AttributionSource::ApiKey))
        })
}

fn ensure_required(
    require: &[ScopeDimension],
    attribution: &RequestAttribution,
) -> Result<(), RejectionError> {
    if let Some(missing) = require
        .iter()
        .find(|d| attribution.value_for(d.as_str()).is_none())
    {
        return Err(RejectionError::client(
            StatusCode::BAD_REQUEST,
            format!(
                "scope_required: set {SCOPE_PREFIX}{missing} or hold a default {missing} value"
            ),
        ));
    }
    Ok(())
}

pub(super) async fn attribute_scopes(
    rc: &RequestContext<'_>,
    gateway_config: &GatewayConfig,
    principal: &AuthedPrincipal,
    scope_headers: &ScopeHeaders,
    partial: &mut RejectionPartial,
) -> Result<RequestAttribution, RejectionError> {
    let scope = ScopeResolution {
        providers: &rc.repos.subject_providers,
        user_id: principal.user_id(),
        headers: scope_headers,
        key_bindings: &[],
        api_key_id: principal.api_key().map(|key| &key.api_key_id),
        require: &gateway_config.require_scopes,
    };
    resolve_scope_attribution(scope, partial).await
}
