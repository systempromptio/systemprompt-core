//! Pre-dispatch upstream resolution: model-exposure check, route and provider
//! lookup, API-key secret, and outbound wire adapter.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;
use std::sync::Arc;

use anyhow::anyhow;
use systemprompt_ai::RouteSelectorEngine;
use systemprompt_identifiers::AiRequestId;
use systemprompt_models::profile::{GatewayConfig, GatewayRoute, ProviderEntry, ProviderRegistry};

use super::super::protocol::canonical::CanonicalRequest;
use super::super::protocol::outbound::OutboundAdapter;
use super::super::registry::GatewayUpstreamRegistry;
use super::{DispatchError, PolicyDenied};

pub(super) struct ResolvedUpstream<'a> {
    pub(super) route: Cow<'a, GatewayRoute>,
    pub(super) provider: &'a ProviderEntry,
    pub(super) api_key: &'static str,
    pub(super) adapter: &'static Arc<dyn OutboundAdapter>,
    pub(super) route_match_descriptor: Option<String>,
}

pub(super) async fn resolve_upstream<'a>(
    config: &'a GatewayConfig,
    registry: &'a ProviderRegistry,
    request: &CanonicalRequest,
    ai_request_id: &AiRequestId,
) -> Result<ResolvedUpstream<'a>, DispatchError> {
    if !config.is_model_exposed(registry, &request.model) {
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

    let provider = route.resolve(registry).ok_or_else(|| {
        DispatchError::PreAudit(anyhow!(
            "Gateway route '{}' provider '{}' is not declared in profile.providers",
            route.id.as_str(),
            route.provider.as_str()
        ))
    })?;

    enforce_route_requirements(&route, provider, &request.model, ai_request_id)?;

    let secrets = systemprompt_config::SecretsBootstrap::get()
        .map_err(|e| DispatchError::PreAudit(anyhow!("Secrets not available: {e}")))?;

    let api_key = secrets
        .get(provider.api_key_secret.as_str())
        .ok_or_else(|| {
            DispatchError::PreAudit(anyhow!(
                "Gateway API key secret '{}' not configured",
                provider.api_key_secret.as_str()
            ))
        })?;

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
        api_key,
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
    let upstream = route.effective_upstream_model(requested_model);
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
