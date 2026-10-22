//! Pre-dispatch upstream resolution: model-exposure check, route and provider
//! lookup, API-key secret, and outbound wire adapter.
//!
//! The matched route's deployment chain is selected from the request's scope
//! attribution (`by_scope`); an attributed value the route does not map is
//! refused unless the route opts into `unmapped: shared`.
//! The chain is then planned (`chain_plan`): reordered by its selection
//! strategy and checked against the leading deployment's context window.
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
use super::chain_plan::{PlannedChain, fit_context_window, order_chain};
use super::credentials::{BillingSource, KeyOwner};
use super::route_match::{describe_route_match, enforce_route_requirements};
use super::{DispatchError, GatewayError, PolicyDenied};

pub(super) struct ResolvedUpstream<'a> {
    pub(super) route: Cow<'a, GatewayRoute>,
    pub(super) provider: &'a ProviderEntry,
    pub(super) call: UpstreamCall,
    pub(super) adapter: &'static Arc<dyn OutboundAdapter>,
    pub(super) route_match_descriptor: Option<String>,
    pub(super) deployments: Vec<GatewayRoute>,
    pub(super) scoped: bool,
    pub(super) billing: BillingSource,
    pub(super) owner: KeyOwner,
}

pub(super) struct UpstreamQuery<'r> {
    pub(super) request: &'r CanonicalRequest,
    pub(super) attribution: &'r RequestAttribution,
    pub(super) ai_request_id: &'r AiRequestId,
    pub(super) owner: KeyOwner,
}

pub(super) async fn resolve_upstream<'a>(
    config: &'a GatewayConfig,
    registry: &'a ProviderRegistry,
    query: UpstreamQuery<'_>,
) -> Result<ResolvedUpstream<'a>, DispatchError> {
    let UpstreamQuery {
        request,
        attribution,
        ai_request_id,
        owner,
    } = query;
    ensure_model_exposed(config, registry, request, ai_request_id)?;

    let matched = config.resolve_route(registry, request).ok_or_else(|| {
        DispatchError::pre_audit(GatewayError::NoRoute {
            model: request.model.to_string(),
        })
    })?;

    let declarative = matched.when.as_ref().and_then(|w| {
        let predicates = w.matched_predicates();
        (!predicates.is_empty()).then(|| format!("when:{}", predicates.join(",")))
    });
    let (route, selector) = refine_route(matched, request).await;

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
    let planned = plan_chain(registry, request, &route, &selection, ai_request_id)?;
    let deployments = planned.deployments;
    let route_match_descriptor = join_segments(
        Some(describe_route_match(&route, declarative, selector, scope)),
        planned.descriptor,
    );
    let Some(primary) = deployments.first().cloned() else {
        return Err(DispatchError::pre_audit(GatewayError::NoRoute {
            model: request.model.to_string(),
        }));
    };
    let mut bound = bind_route(
        registry,
        Cow::Owned(primary),
        BindTarget {
            requested_model: request.model.as_str(),
            ai_request_id,
            owner,
        },
        route_match_descriptor,
    )
    .await?;
    bound.deployments = deployments;
    bound.scoped = scoped;
    Ok(bound)
}

fn ensure_model_exposed(
    config: &GatewayConfig,
    registry: &ProviderRegistry,
    request: &CanonicalRequest,
    ai_request_id: &AiRequestId,
) -> Result<(), DispatchError> {
    if config.is_model_exposed(registry, request.model.as_str()) {
        return Ok(());
    }
    tracing::warn!(
        ai_request_id = %ai_request_id,
        model = %request.model,
        "Gateway denied: model not exposed by gateway policy or registry"
    );
    Err(DispatchError::pre_audit(PolicyDenied(format!(
        "model '{}' is not permitted by gateway policy",
        request.model
    ))))
}

async fn refine_route<'a>(
    matched: Cow<'a, GatewayRoute>,
    request: &CanonicalRequest,
) -> (Cow<'a, GatewayRoute>, Option<String>) {
    let engine = RouteSelectorEngine::global();
    if !engine.has_selectors() {
        return (matched, None);
    }
    match engine.refine(matched.as_ref(), request).await {
        Some((refined, name)) => (Cow::Owned(refined), Some(format!("selector:{name}"))),
        None => (matched, None),
    }
}

fn plan_chain(
    registry: &ProviderRegistry,
    request: &CanonicalRequest,
    route: &GatewayRoute,
    selection: &ChainSelection<'_>,
    ai_request_id: &AiRequestId,
) -> Result<PlannedChain, DispatchError> {
    let ordered = order_chain(route.chain_views(selection), request.model.as_str());
    let context_fallbacks = route.context_fallback_views(selection);
    let fitted = fit_context_window(registry, request, ordered.deployments, context_fallbacks)
        .map_err(|exceeded| {
            tracing::warn!(
                ai_request_id = %ai_request_id,
                route = %route.effective_id(),
                model = %exceeded.model,
                estimate = exceeded.estimate,
                limit = exceeded.limit,
                "Gateway denied: request exceeds the model's context window"
            );
            DispatchError::pre_audit(exceeded)
        })?;
    Ok(PlannedChain {
        deployments: fitted.deployments,
        descriptor: join_segments(ordered.descriptor, fitted.descriptor),
    })
}

fn join_segments(first: Option<String>, second: Option<String>) -> Option<String> {
    match (first, second) {
        (Some(a), Some(b)) => Some(format!("{a};{b}")),
        (a, b) => a.or(b),
    }
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
    // Why: a personal subscription and a deployment secret, or two members'
    // personal subscriptions, bill different accounts; a failover hop between
    // them would move a request onto a bill nobody chose.
    if view
        .resolve(registry)
        .is_some_and(|entry| !entry.bills_same_credential(primary.provider))
    {
        return Err(DispatchError::pre_audit(PolicyDenied(format!(
            "deployment '{}' bills a different credential than '{}'",
            view.provider.as_str(),
            primary.provider.name.as_str()
        ))));
    }
    let failover = format!("failover:{}", hop.hops.join("->"));
    let descriptor = Some(match primary.route_match_descriptor.as_deref() {
        Some(existing) => format!("{existing};{failover}"),
        None => failover,
    });
    bind_route(
        registry,
        Cow::Owned(view),
        BindTarget {
            requested_model,
            ai_request_id,
            owner: primary.owner.clone(),
        },
        descriptor,
    )
    .await
    .map(Some)
}

struct BindTarget<'r> {
    requested_model: &'r str,
    ai_request_id: &'r AiRequestId,
    owner: KeyOwner,
}

async fn bind_route<'a>(
    registry: &'a ProviderRegistry,
    route: Cow<'a, GatewayRoute>,
    target: BindTarget<'_>,
    route_match_descriptor: Option<String>,
) -> Result<ResolvedUpstream<'a>, DispatchError> {
    let BindTarget {
        requested_model,
        ai_request_id,
        owner,
    } = target;
    let provider = route.resolve(registry).ok_or_else(|| {
        DispatchError::pre_audit(GatewayError::UndeclaredProvider {
            route: route.effective_id().to_string(),
            provider: route.provider.as_str().to_owned(),
        })
    })?;

    enforce_route_requirements(&route, provider, requested_model, ai_request_id)?;

    let (call, billing) = super::credentials::resolve(provider, &owner).await?;

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
        billing,
        owner,
    })
}
