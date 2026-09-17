//! Pre-dispatch upstream resolution: model-exposure check, route and provider
//! lookup, API-key secret, and outbound wire adapter.
//! [`resolve_fallback_upstream`] binds the same request to a route's
//! `fallback_provider` when the primary upstream has failed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;
use std::sync::Arc;

use anyhow::anyhow;
use systemprompt_ai::RouteSelectorEngine;
use systemprompt_identifiers::AiRequestId;
use systemprompt_models::services::{GatewayConfig, GatewayRoute, ProviderEntry, ProviderRegistry};

use super::super::protocol::canonical::CanonicalRequest;
use super::super::protocol::outbound::OutboundAdapter;
use super::super::registry::GatewayUpstreamRegistry;
use super::{DispatchError, PolicyDenied};

pub(super) struct ResolvedUpstream<'a> {
    pub(super) route: Cow<'a, GatewayRoute>,
    pub(super) provider: &'a ProviderEntry,
    pub(super) endpoint: String,
    pub(super) api_key: String,
    pub(super) api_key_is_bearer: bool,
    pub(super) adapter: &'static Arc<dyn OutboundAdapter>,
    pub(super) route_match_descriptor: Option<String>,
}

pub(super) async fn resolve_upstream<'a>(
    config: &'a GatewayConfig,
    registry: &'a ProviderRegistry,
    request: &CanonicalRequest,
    ai_request_id: &AiRequestId,
) -> Result<ResolvedUpstream<'a>, DispatchError> {
    if !config.is_model_exposed(registry, request.model.as_str()) {
        tracing::warn!(
            ai_request_id = %ai_request_id,
            model = %request.model,
            "Gateway denied: model not exposed by gateway policy or registry"
        );
        return Err(DispatchError::PreAudit(
            PolicyDenied(format!(
                "model '{}' is not permitted by gateway policy",
                request.model
            ))
            .into(),
        ));
    }

    let matched = config.resolve_route(registry, request).ok_or_else(|| {
        DispatchError::PreAudit(anyhow!(
            "No gateway route matches model '{}'",
            request.model
        ))
    })?;

    let declarative = matched.when.as_ref().and_then(|w| {
        let predicates = w.matched_predicates();
        (!predicates.is_empty()).then(|| format!("when:{}", predicates.join(",")))
    });
    let engine = RouteSelectorEngine::global();
    let (route, selector) = if engine.has_selectors() {
        match engine.refine(matched.as_ref(), request).await {
            Some((refined, name)) => (Cow::Owned(refined), Some(format!("selector:{name}"))),
            None => (matched, None),
        }
    } else {
        (matched, None)
    };

    let route_match_descriptor = describe_route_match(&route, declarative, selector);
    bind_route(
        registry,
        route,
        request.model.as_str(),
        ai_request_id,
        route_match_descriptor,
    )
    .await
}

pub(super) async fn resolve_fallback_upstream<'a>(
    registry: &'a ProviderRegistry,
    primary: &ResolvedUpstream<'a>,
    requested_model: &str,
    ai_request_id: &AiRequestId,
) -> Result<Option<ResolvedUpstream<'a>>, DispatchError> {
    let Some(view) = primary.route.fallback_view() else {
        return Ok(None);
    };
    let failover = format!(
        "failover:{}->{}",
        primary.provider.name.as_str(),
        view.provider.as_str()
    );
    let descriptor = Some(match primary.route_match_descriptor.as_deref() {
        Some(existing) => format!("{existing};{failover}"),
        None => failover,
    });
    bind_route(
        registry,
        Cow::Owned(view),
        requested_model,
        ai_request_id,
        descriptor,
    )
    .await
    .map(Some)
}

async fn bind_route<'a>(
    registry: &'a ProviderRegistry,
    route: Cow<'a, GatewayRoute>,
    requested_model: &str,
    ai_request_id: &AiRequestId,
    route_match_descriptor: Option<String>,
) -> Result<ResolvedUpstream<'a>, DispatchError> {
    let provider = route.resolve(registry).ok_or_else(|| {
        DispatchError::PreAudit(anyhow!(
            "Gateway route '{}' provider '{}' is not declared in services providers",
            route.id.as_str(),
            route.provider.as_str()
        ))
    })?;

    enforce_route_requirements(&route, provider, requested_model, ai_request_id)?;

    let credential = super::credentials::resolve(provider).await?;
    let endpoint =
        systemprompt_security::credential::fill_endpoint(&provider.endpoint, &credential.scope)
            .map_err(|e| DispatchError::PreAudit(anyhow::Error::new(e)))?;

    let adapter = GatewayUpstreamRegistry::global()
        .get(provider.wire.as_tag())
        .ok_or_else(|| {
            DispatchError::PreAudit(anyhow!(
                "Gateway has no outbound adapter for wire protocol '{}'",
                provider.wire.as_tag()
            ))
        })?;

    Ok(ResolvedUpstream {
        route,
        provider,
        endpoint,
        api_key: credential.value,
        api_key_is_bearer: credential.is_bearer,
        adapter,
        route_match_descriptor,
    })
}

pub fn describe_route_match(
    route: &GatewayRoute,
    declarative: Option<String>,
    selector: Option<String>,
) -> Option<String> {
    let governance = route.requires.as_ref().and_then(|r| {
        let declared = r.declared();
        (!declared.is_empty()).then(|| format!("requires:{}", declared.join(",")))
    });

    (declarative.is_some() || selector.is_some() || governance.is_some()).then(|| {
        [declarative, selector, governance]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(";")
    })
}

pub fn enforce_route_requirements(
    route: &GatewayRoute,
    provider: &ProviderEntry,
    requested_model: &str,
    ai_request_id: &AiRequestId,
) -> Result<(), DispatchError> {
    let Some(requires) = route.requires.as_ref() else {
        return Ok(());
    };
    let upstream = provider.upstream_model_for(route.upstream_model.as_deref(), requested_model);
    let unmet = requires.unmet(provider.effective_governance(upstream));
    if unmet.is_empty() {
        return Ok(());
    }

    tracing::warn!(
        ai_request_id = %ai_request_id,
        route = %route.id.as_str(),
        model = %upstream,
        requirements = %unmet.join(","),
        "Gateway denied: route governance requirements unmet by resolved provider/model"
    );
    Err(DispatchError::PreAudit(
        PolicyDenied(format!(
            "route '{}' requires [{}] which provider '{}' does not satisfy for model '{}'",
            route.id.as_str(),
            unmet.join(","),
            route.provider.as_str(),
            upstream
        ))
        .into(),
    ))
}
