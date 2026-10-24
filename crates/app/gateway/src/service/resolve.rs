//! Pre-dispatch upstream resolution: the catalog resolution, the planned
//! chain, the credential and the outbound wire adapter.
//!
//! `resolve_upstream` resolves the request through the catalog
//! (`crate::routing::resolve_route`): an unknown model is refused as not
//! permitted, a model with no enabled deployment has no route. Route
//! selectors may then narrow the chain, which is planned (`chain_plan`):
//! reordered by its selection strategy and checked against the leading
//! deployment's context window. `resolve_deployment_upstream` binds the
//! same request to a later hop of that chain when an earlier one has failed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_ai::UpstreamCall;
use systemprompt_ai::models::BillingSource;
use systemprompt_identifiers::{AiRequestId, RouteId};
use systemprompt_manifest::services::GatewayRoute;
use systemprompt_manifest::services::catalog::{
    ResolveError, ResolvedCatalog, ResolvedHop, ResolvedModel, ResolvedTarget,
};
use systemprompt_models::attribution::RequestAttribution;

use super::super::protocol::canonical::CanonicalRequest;
use super::super::protocol::outbound::OutboundAdapter;
use super::super::registry::GatewayEngines;
use super::chain_plan::{PlannedChain, fit_context_window, order_chain};
use super::credentials::KeyOwner;
use super::route_match::{describe_route_match, enforce_route_requirements, refine_chain};
use super::{DispatchError, GatewayError, PolicyDenied};
use crate::routing::{RouteSource, resolve_route};

pub(super) struct ResolvedUpstream<'a> {
    pub(super) catalog: &'a ResolvedCatalog,
    pub(super) hop: ResolvedHop,
    pub(super) model: ResolvedModel,
    pub(super) route: Option<GatewayRoute>,
    pub(super) route_id: RouteId,
    pub(super) call: UpstreamCall,
    pub(super) adapter: Arc<dyn OutboundAdapter>,
    pub(super) http: reqwest::Client,
    pub(super) route_match_descriptor: Option<String>,
    pub(super) deployments: Vec<ResolvedHop>,
    pub(super) billing: BillingSource,
    pub(super) owner: KeyOwner,
}

impl ResolvedUpstream<'_> {
    pub(super) fn model_limits(&self) -> systemprompt_wire::ModelLimits {
        self.catalog
            .model(self.hop.model.as_str())
            .map_or(self.model.limits, |model| model.limits)
    }
}

pub(super) struct UpstreamQuery<'r> {
    pub(super) request: &'r CanonicalRequest,
    pub(super) attribution: &'r RequestAttribution,
    pub(super) ai_request_id: &'r AiRequestId,
    pub(super) owner: KeyOwner,
}

pub(super) async fn resolve_upstream<'a>(
    catalog: &'a ResolvedCatalog,
    engines: &GatewayEngines,
    query: UpstreamQuery<'_>,
) -> Result<ResolvedUpstream<'a>, DispatchError> {
    let UpstreamQuery {
        request,
        attribution,
        ai_request_id,
        owner,
    } = query;
    let resolution = resolve_route(RouteSource::Catalog(catalog), request, attribution)
        .map_err(|error| refusal(&error, request, ai_request_id))?;
    let mut target = resolution.target;
    let route = target
        .route
        .as_ref()
        .and_then(|id| catalog.route(id.as_str()))
        .cloned();
    let declarative = route.as_ref().and_then(|r| r.when.as_ref()).and_then(|w| {
        let predicates = w.matched_predicates();
        (!predicates.is_empty()).then(|| format!("when:{}", predicates.join(",")))
    });
    let selector = refine_chain(engines, catalog, &mut target, route.as_ref(), request).await;
    let planned = plan_chain(catalog, engines, request, &target, ai_request_id)?;
    let route_match_descriptor = join_segments(
        Some(describe_route_match(
            &target.access_route_id,
            route.as_ref(),
            declarative,
            selector,
        )),
        planned.descriptor,
    );
    let deployments = planned.deployments;
    let Some(primary) = deployments.first().cloned() else {
        return Err(DispatchError::pre_audit(GatewayError::NoRoute {
            model: request.model.to_string(),
        }));
    };
    let mut bound = bind_hop(
        catalog,
        engines,
        primary,
        BindTarget {
            model: target.model,
            route,
            route_id: target.access_route_id,
            ai_request_id,
            owner,
        },
        route_match_descriptor,
    )
    .await?;
    bound.deployments = deployments;
    Ok(bound)
}

fn refusal(
    error: &ResolveError,
    request: &CanonicalRequest,
    ai_request_id: &AiRequestId,
) -> DispatchError {
    match error {
        ResolveError::UnknownModel { .. } => {
            tracing::warn!(
                ai_request_id = %ai_request_id,
                model = %request.model,
                "Gateway denied: model is not in the catalog"
            );
            DispatchError::pre_audit(PolicyDenied(format!(
                "model '{}' is not permitted by gateway policy",
                request.model
            )))
        },
        ResolveError::NoEnabledDeployment { .. } => {
            DispatchError::pre_audit(GatewayError::NoRoute {
                model: request.model.to_string(),
            })
        },
    }
}

fn plan_chain(
    catalog: &ResolvedCatalog,
    engines: &GatewayEngines,
    request: &CanonicalRequest,
    target: &ResolvedTarget,
    ai_request_id: &AiRequestId,
) -> Result<PlannedChain, DispatchError> {
    let ordered = order_chain(
        target.chain.clone(),
        target.strategy,
        &target.access_route_id,
        &engines.breakers,
        &engines.load,
    );
    let fitted = fit_context_window(
        catalog,
        request,
        ordered.deployments,
        target.context_fallbacks.clone(),
    )
    .map_err(|exceeded| {
        tracing::warn!(
            ai_request_id = %ai_request_id,
            route = %target.access_route_id,
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
    pub(super) engines: &'h GatewayEngines,
}

pub(super) async fn resolve_deployment_upstream<'a>(
    primary: &ResolvedUpstream<'a>,
    hop: DeploymentHop<'_>,
    ai_request_id: &AiRequestId,
) -> Result<Option<ResolvedUpstream<'a>>, DispatchError> {
    let Some(next) = primary.deployments.get(hop.index).cloned() else {
        return Ok(None);
    };
    if !may_fail_over(&primary.hop, &next) {
        return Err(DispatchError::pre_audit(PolicyDenied(format!(
            "deployment '{}' bills credential '{}', not '{}'",
            next.endpoint_ref(),
            next.credential.id,
            primary.hop.credential.id
        ))));
    }
    let failover = format!("failover:{}", hop.hops.join("->"));
    let descriptor = Some(match primary.route_match_descriptor.as_deref() {
        Some(existing) => format!("{existing};{failover}"),
        None => failover,
    });
    bind_hop(
        primary.catalog,
        hop.engines,
        next,
        BindTarget {
            model: primary.model.clone(),
            route: primary.route.clone(),
            route_id: primary.route_id.clone(),
            ai_request_id,
            owner: primary.owner.clone(),
        },
        descriptor,
    )
    .await
    .map(Some)
}

// Why: a personal subscription and a deployment secret, or two members'
// personal subscriptions, bill different accounts; a failover hop between them
// would move a request onto a bill nobody chose. Two platform credentials are
// both the deployment's own bill, so failing over between them is allowed.
#[must_use]
pub fn may_fail_over(from: &ResolvedHop, to: &ResolvedHop) -> bool {
    match (from.credential.is_personal(), to.credential.is_personal()) {
        (false, false) => true,
        (true, true) => from.bills_same_credential(to),
        _ => false,
    }
}

struct BindTarget<'r> {
    model: ResolvedModel,
    route: Option<GatewayRoute>,
    route_id: RouteId,
    ai_request_id: &'r AiRequestId,
    owner: KeyOwner,
}

async fn bind_hop<'a>(
    catalog: &'a ResolvedCatalog,
    engines: &GatewayEngines,
    hop: ResolvedHop,
    target: BindTarget<'_>,
    route_match_descriptor: Option<String>,
) -> Result<ResolvedUpstream<'a>, DispatchError> {
    let BindTarget {
        model,
        route,
        route_id,
        ai_request_id,
        owner,
    } = target;
    enforce_route_requirements(route.as_ref(), &route_id, &hop, ai_request_id)?;

    let (call, billing) = super::credentials::resolve(catalog, &hop, &owner).await?;

    let adapter = engines
        .upstream_registry
        .get(hop.wire.as_tag())
        .map(Arc::clone)
        .ok_or_else(|| {
            DispatchError::pre_audit(GatewayError::NoAdapter {
                wire: hop.wire.as_tag().to_owned(),
            })
        })?;

    Ok(ResolvedUpstream {
        catalog,
        hop,
        model,
        route,
        route_id,
        call,
        adapter,
        http: engines.http.clone(),
        route_match_descriptor,
        deployments: Vec::new(),
        billing,
        owner,
    })
}
