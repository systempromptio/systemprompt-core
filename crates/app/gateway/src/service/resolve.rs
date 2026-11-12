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
    ResolveError, ResolvedCatalog, ResolvedHop, ResolvedModel,
};
use systemprompt_models::attribution::RequestAttribution;
use systemprompt_wire::WireProtocol;

use super::super::protocol::canonical::CanonicalRequest;
use super::super::protocol::outbound::OutboundAdapter;
use super::super::registry::GatewayEngines;
use super::chain_plan::{ChainHealth, SentBody, join_segments, plan_chain};
use super::credentials::KeyOwner;
use super::hop_policy::servable_hops;
use super::route_match::{describe_route_match, enforce_route_requirements, refine_chain};
use super::{DispatchError, GatewayError, HopPolicy, PolicyDenied};
use crate::routing::{RouteSource, resolve_route};

pub(super) struct ResolvedUpstream<'a> {
    pub(super) catalog: &'a ResolvedCatalog,
    pub(super) hop: ResolvedHop,
    pub(super) model: ResolvedModel,
    pub(super) route: Option<GatewayRoute>,
    pub(super) route_id: Option<RouteId>,
    pub(super) call: UpstreamCall,
    pub(super) adapter: Arc<dyn OutboundAdapter>,
    pub(super) http: crate::protocol::outbound::UpstreamHttp,
    pub(super) route_match_descriptor: Option<String>,
    pub(super) deployments: Vec<ResolvedHop>,
    pub(super) billing: BillingSource,
    pub(super) owner: KeyOwner,
    pub(super) policy: HopPolicy,
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
    pub(super) sent: SentBody,
    pub(super) format: &'static str,
    pub(super) spoken: Option<WireProtocol>,
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
        sent,
        format,
        spoken,
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
    let health = ChainHealth {
        breakers: &engines.breakers,
        load: &engines.load,
        owner: &owner.user_id,
    };
    let planned = plan_chain(catalog, health, (request, sent), &target, ai_request_id)?;
    let route_match_descriptor = join_segments(
        Some(describe_route_match(
            target.route.as_ref(),
            route.as_ref(),
            declarative,
            selector,
        )),
        planned.descriptor,
    );
    let deployments = servable_hops(planned.deployments, spoken).map_err(|wire| {
        DispatchError::pre_audit(GatewayError::CallerFormatMismatch {
            model: request.model.to_string(),
            inbound: format,
            wire: wire.as_tag().to_owned(),
        })
    })?;
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
            route_id: target.route.clone(),
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
    if !primary.hop.may_fail_over_to(&next) {
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

struct BindTarget<'r> {
    model: ResolvedModel,
    route: Option<GatewayRoute>,
    route_id: Option<RouteId>,
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
    enforce_route_requirements(route.as_ref(), &hop, ai_request_id)?;

    let (call, billing) = super::credentials::resolve(catalog, &hop, &owner, engines).await?;

    let policy = HopPolicy::of(&hop);
    let adapter = engines
        .upstream_registry
        .adapter_for(hop.wire, policy)
        .map(Arc::clone)
        .ok_or_else(|| {
            DispatchError::pre_audit(GatewayError::NoAdapter {
                wire: hop.wire.as_tag().to_owned(),
            })
        })?;

    Ok(ResolvedUpstream {
        catalog,
        policy,
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
