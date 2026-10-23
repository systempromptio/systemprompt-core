//! How a resolved route is described on the audit row, narrowed by route
//! selectors, and checked against its declared governance requirements.
//!
//! A selector sees the matched policy route, or the catalog route (an empty
//! route under the access route id) when none matched. A refined route may
//! only narrow the model's chain to a subset of its deployments and set the
//! strategy; it never adds a deployment.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{AiRequestId, RouteId};
use systemprompt_manifest::services::GatewayRoute;
use systemprompt_manifest::services::catalog::{ResolvedCatalog, ResolvedHop, ResolvedTarget};

use super::super::protocol::canonical::CanonicalRequest;
use super::{DispatchError, PolicyDenied};
use crate::registry::GatewayEngines;

pub fn describe_route_match(
    route_id: &RouteId,
    route: Option<&GatewayRoute>,
    declarative: Option<String>,
    selector: Option<String>,
) -> String {
    let governance = route.and_then(|r| r.requires.as_ref()).and_then(|r| {
        let declared = r.declared();
        (!declared.is_empty()).then(|| format!("requires:{}", declared.join(",")))
    });

    let identity = Some(format!("route:{route_id}"));
    [identity, declarative, selector, governance]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(";")
}

pub(super) async fn refine_chain(
    engines: &GatewayEngines,
    catalog: &ResolvedCatalog,
    target: &mut ResolvedTarget,
    route: Option<&GatewayRoute>,
    request: &CanonicalRequest,
) -> Option<String> {
    let engine = &engines.route_selector;
    if !engine.has_selectors() {
        return None;
    }
    let catalog_route = GatewayRoute::new(target.access_route_id.as_str());
    let matched = route.unwrap_or(&catalog_route);
    let (refined, name) = engine.refine(matched, request).await?;
    let hops = catalog.deployments(target.model.id.as_str());
    let chain: Vec<ResolvedHop> = if refined.deployments.is_empty() {
        target.chain.clone()
    } else {
        refined
            .deployments
            .iter()
            .filter_map(|pinned| {
                hops.iter()
                    .find(|hop| hop.endpoint_ref() == *pinned.endpoint())
                    .filter(|hop| catalog.is_enabled(hop))
                    .map(|hop| ResolvedHop {
                        weight: pinned.weight(),
                        ..hop.clone()
                    })
            })
            .collect()
    };
    if chain.is_empty() {
        tracing::warn!(
            selector = name,
            route = %refined.id,
            "route selector narrowed the chain to nothing; keeping the resolved chain"
        );
        return None;
    }
    target.chain = chain;
    target.strategy = refined.strategy;
    Some(format!("selector:{name}"))
}

pub fn enforce_route_requirements(
    route: Option<&GatewayRoute>,
    route_id: &RouteId,
    hop: &ResolvedHop,
    ai_request_id: &AiRequestId,
) -> Result<(), DispatchError> {
    let Some(requires) = route.and_then(|r| r.requires.as_ref()) else {
        return Ok(());
    };
    let unmet = requires.unmet(hop.governance);
    if unmet.is_empty() {
        return Ok(());
    }

    tracing::warn!(
        ai_request_id = %ai_request_id,
        route = %route_id,
        model = %hop.upstream_model,
        requirements = %unmet.join(","),
        "Gateway denied: route governance requirements unmet by resolved deployment"
    );
    Err(DispatchError::pre_audit(PolicyDenied(format!(
        "route '{route_id}' requires [{}] which deployment '{}' does not satisfy for model '{}'",
        unmet.join(","),
        hop.endpoint_ref(),
        hop.upstream_model
    ))))
}
