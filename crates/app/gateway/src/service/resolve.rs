//! Pre-dispatch upstream resolution: model-exposure check, route and provider
//! lookup, API-key secret, and outbound wire adapter.
//!
//! The matched route's deployment chain is selected from the request's scope
//! attribution (`by_scope`); an attributed value the route does not map is
//! refused unless the route opts into `unmapped: shared`.
//! `resolve_deployment_upstream` binds the same request to a later deployment
//! of that chain when an earlier one has failed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;
use std::sync::Arc;

use crate::policies::RouteSelectorEngine;
use systemprompt_ai::UpstreamCall;
use systemprompt_identifiers::AiRequestId;
use systemprompt_manifest::services::{
    ChainSelection, GatewayConfig, GatewayRoute, ProviderEntry, ProviderRegistry,
};
use systemprompt_models::attribution::RequestAttribution;

use super::super::protocol::canonical::CanonicalRequest;
use super::super::protocol::outbound::OutboundAdapter;
use super::super::registry::GatewayUpstreamRegistry;
use super::{DispatchError, GatewayError, PolicyDenied};

pub(super) struct ResolvedUpstream<'a> {
    pub(super) route: Cow<'a, GatewayRoute>,
    pub(super) provider: &'a ProviderEntry,
    pub(super) call: UpstreamCall,
    pub(super) adapter: &'static Arc<dyn OutboundAdapter>,
    pub(super) route_match_descriptor: Option<String>,
    pub(super) deployments: Vec<GatewayRoute>,
    pub(super) scoped: bool,
}

pub(super) async fn resolve_upstream<'a>(
    config: &'a GatewayConfig,
    registry: &'a ProviderRegistry,
    request: &CanonicalRequest,
    attribution: &RequestAttribution,
    ai_request_id: &AiRequestId,
) -> Result<ResolvedUpstream<'a>, DispatchError> {
    if !config.is_model_exposed(registry, request.model.as_str()) {
        tracing::warn!(
            ai_request_id = %ai_request_id,
            model = %request.model,
            "Gateway denied: model not exposed by gateway policy or registry"
        );
        return Err(DispatchError::pre_audit(PolicyDenied(format!(
            "model '{}' is not permitted by gateway policy",
            request.model
        ))));
    }

    let matched = config.resolve_route(registry, request).ok_or_else(|| {
        DispatchError::pre_audit(GatewayError::NoRoute {
            model: request.model.to_string(),
        })
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

    let selection = route.chain_for(attribution);
    if let ChainSelection::Unmapped { dimension, value } = selection {
        tracing::warn!(
            ai_request_id = %ai_request_id,
            route = %route.effective_id(),
            dimension = %dimension,
            value = %value,
            "Gateway denied: scope value has no deployment chain on the route"
        );
        return Err(DispatchError::pre_audit(PolicyDenied(format!(
            "scope {dimension}='{value}' has no deployment on route '{}'",
            route.effective_id()
        ))));
    }
    let scope = selection.descriptor();
    let scoped = matches!(selection, ChainSelection::Scope { .. });
    let deployments = route.chain_views(&selection);
    let route_match_descriptor = describe_route_match(&route, declarative, selector, scope);
    let Some(primary) = deployments.first().cloned() else {
        return Err(DispatchError::pre_audit(GatewayError::NoRoute {
            model: request.model.to_string(),
        }));
    };
    let mut bound = bind_route(
        registry,
        Cow::Owned(primary),
        request.model.as_str(),
        ai_request_id,
        route_match_descriptor,
    )
    .await?;
    bound.deployments = deployments;
    bound.scoped = scoped;
    Ok(bound)
}

pub(super) struct DeploymentHop<'h> {
    pub(super) index: usize,
    pub(super) hops: &'h [String],
}

pub(super) async fn resolve_deployment_upstream<'a>(
    registry: &'a ProviderRegistry,
    primary: &ResolvedUpstream<'a>,
    hop: DeploymentHop<'_>,
    requested_model: &str,
    ai_request_id: &AiRequestId,
) -> Result<Option<ResolvedUpstream<'a>>, DispatchError> {
    let Some(view) = primary.deployments.get(hop.index).cloned() else {
        return Ok(None);
    };
    let failover = format!("failover:{}", hop.hops.join("->"));
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
        DispatchError::pre_audit(GatewayError::UndeclaredProvider {
            route: route.effective_id().to_string(),
            provider: route.provider.as_str().to_owned(),
        })
    })?;

    enforce_route_requirements(&route, provider, requested_model, ai_request_id)?;

    let call = super::credentials::resolve(provider).await?;

    let adapter = GatewayUpstreamRegistry::global()
        .get(provider.wire.as_tag())
        .ok_or_else(|| {
            DispatchError::pre_audit(GatewayError::NoAdapter {
                wire: provider.wire.as_tag().to_owned(),
            })
        })?;

    Ok(ResolvedUpstream {
        route,
        provider,
        call,
        adapter,
        route_match_descriptor,
        deployments: Vec::new(),
        scoped: false,
    })
}

pub fn describe_route_match(
    route: &GatewayRoute,
    declarative: Option<String>,
    selector: Option<String>,
    scope: Option<String>,
) -> Option<String> {
    let governance = route.requires.as_ref().and_then(|r| {
        let declared = r.declared();
        (!declared.is_empty()).then(|| format!("requires:{}", declared.join(",")))
    });

    let any = declarative.is_some() || selector.is_some() || governance.is_some();
    (any || scope.is_some()).then(|| {
        [declarative, selector, governance, scope]
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
        route = %route.effective_id(),
        model = %upstream,
        requirements = %unmet.join(","),
        "Gateway denied: route governance requirements unmet by resolved provider/model"
    );
    Err(DispatchError::pre_audit(PolicyDenied(format!(
        "route '{}' requires [{}] which provider '{}' does not satisfy for model '{}'",
        route.effective_id(),
        unmet.join(","),
        route.provider.as_str(),
        upstream
    ))))
}
