//! The enumerating mount over an admission handoff.
//!
//! [`mount_admitted`] re-checks the whole handoff first ([`recheck`]), then
//! mounts every admitted route set its role serves: the set's state is
//! resolved through the kernel with exactly the handles its base was
//! conferred, each declared route is protected with its own spec, and the
//! routes of every set at one base are placed there together, after the
//! server's `decorate` hook has layered the request extensions every
//! composition route reads (the governance watch, the services refresh). A
//! set whose
//! state needs a handle its base was not conferred fails with
//! [`MountError::Resolve`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::router::{BasePath, BodyLimit, RouteSpec};
use systemprompt_extension::{Admitted, RouteGroup, RouteProtection};
use systemprompt_runtime::Kernel;

use super::recheck::{path_base, recheck};
use super::{MountError, MountLayers, place, protect};

pub fn protection_of(spec: &RouteSpec) -> RouteProtection {
    let protection =
        RouteProtection::new(spec.policy, spec.context).with_rate_limit(spec.limits.rate);
    match spec.limits.body {
        BodyLimit::Default => protection,
        BodyLimit::Agent => {
            protection.with_body_limit(systemprompt_models::net::AGENT_REQUEST_BODY_LIMIT_BYTES)
        },
    }
}

pub fn mount_admitted(
    mut tree: Router,
    admitted: &Admitted,
    kernel: &Kernel,
    layers: &MountLayers,
    serves: &dyn Fn(RouteGroup) -> bool,
    decorate: &dyn Fn(Router) -> Router,
) -> Result<Router, MountError> {
    recheck(admitted)?;
    let mut by_base: Vec<(BasePath, Router)> = Vec::new();
    for set in admitted.routes() {
        let base = path_base(&set.set)?;
        if !serves(set.set.group()) {
            tracing::info!(
                owner = %set.owner,
                base_path = base.path(),
                group = ?set.set.group(),
                reason = "role_does_not_serve_group",
                "Route set not mounted"
            );
            continue;
        }
        let routers = set.set.materialize(&kernel.route_scoped(&set.handles))?;
        let mut group = Router::new();
        for (def, router) in set.set.defs().iter().zip(routers) {
            group = group.merge(protect(
                router,
                &protection_of(&def.spec),
                layers,
                base.path(),
            )?);
        }
        match by_base.iter_mut().find(|(at, _)| *at == base) {
            Some((_, existing)) => *existing = std::mem::take(existing).merge(group),
            None => by_base.push((base, group)),
        }
    }
    for (base, router) in by_base {
        tree = place(tree, base.path(), decorate(router));
    }
    Ok(tree)
}

/// One admitted gateway route, materialised and ready to be placed under the
/// gateway's verifier.
#[derive(Debug)]
pub struct MountedGatewayRoute {
    pub router: Router,
    pub protection: RouteProtection,
}

pub fn gateway_routes(
    admitted: &Admitted,
    kernel: &Kernel,
) -> Result<Vec<MountedGatewayRoute>, MountError> {
    recheck(admitted)?;
    let mut mounted = Vec::new();
    for route in admitted.gateway_routes() {
        let routers = route.decl.router.materialize(&kernel.route_scoped(&[]))?;
        for (def, router) in route.decl.router.defs().iter().zip(routers) {
            mounted.push(MountedGatewayRoute {
                router,
                protection: protection_of(&def.spec),
            });
        }
    }
    Ok(mounted)
}
