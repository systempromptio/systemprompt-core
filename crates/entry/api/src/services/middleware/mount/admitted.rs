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
//! [`MountError::Resolve`]. A route's own byte limit (`BodyLimit::Bytes`) of
//! zero or above `MountLayers::body_ceiling` (the profile's
//! `server.max_route_body_bytes`) refuses the mount with
//! [`MountError::RouteBodyLimit`], naming the route; within it, an install
//! route mounts with that limit, while a core route above the server default
//! must still be a listed body-limit exemption. An install route
//! admitted beside a reserved literal is wrapped so it answers `404` for any
//! request that falls under a reserved entry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::router::{
    BasePath, BaseRef, BodyLimit, RouteDef, RouteSpec, full_path, shadows_reserved_literal,
};
use systemprompt_extension::{Admitted, RouteGroup, RouteProtection, StandingTag};
use systemprompt_runtime::Kernel;

use super::recheck::{path_base, recheck};
use super::{MountError, MountLayers, place, protect, protect_layered, reserved_guard};
use crate::services::middleware::rate_limit::LayerReach;
use crate::services::server::routes::Composed;

pub const fn protection_of(spec: &RouteSpec) -> RouteProtection {
    let protection =
        RouteProtection::new(spec.policy, spec.context).with_rate_limit(spec.limits.rate);
    match spec.limits.body {
        BodyLimit::Default => protection,
        BodyLimit::Agent => {
            protection.with_body_limit(systemprompt_models::net::AGENT_REQUEST_BODY_LIMIT_BYTES)
        },
        BodyLimit::Bytes(bytes) => protection.with_body_limit(bytes),
    }
}

pub fn mount_admitted(
    mut tree: Router,
    composed: Composed<'_>,
    layers: &MountLayers,
    serves: &dyn Fn(RouteGroup) -> bool,
    decorate: &dyn Fn(Router) -> Router,
) -> Result<Router, MountError> {
    let Composed { admitted, kernel } = composed;
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
        let routers =
            set.set
                .materialize(&kernel.route_scoped(&set.owner, set.standing, &set.handles))?;
        let mut group = Router::new();
        for (def, router) in set.set.defs().iter().zip(routers) {
            check_route_body_limit(set.set.base(), def, layers)?;
            let protection = protection_of(&def.spec);
            let protected = if set.standing == StandingTag::Install
                && matches!(def.spec.limits.body, BodyLimit::Bytes(_))
            {
                protect_layered(router, &protection, layers, base.path())?
            } else {
                protect(router, &protection, layers, base.path())?
            };
            group = group.merge(
                if set.standing == StandingTag::Install
                    && shadows_reserved_literal(set.set.base(), def)
                {
                    reserved_guard::guarded(protected, LayerReach::Routes)
                } else {
                    protected
                },
            );
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

fn check_route_body_limit(
    base: BaseRef,
    def: &RouteDef,
    layers: &MountLayers,
) -> Result<(), MountError> {
    match def.spec.limits.body {
        BodyLimit::Bytes(declared) if declared == 0 || declared > layers.body_ceiling => {
            Err(MountError::RouteBodyLimit {
                method: def.method.clone(),
                path: full_path(base, def.path.as_str()),
                declared,
                ceiling: layers.body_ceiling,
            })
        },
        BodyLimit::Default | BodyLimit::Agent | BodyLimit::Bytes(_) => Ok(()),
    }
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
        let routers = route.decl.router.materialize(
            &kernel.route_scoped(
                &route.owner,
                admitted
                    .built()
                    .standing_of(&route.owner)
                    .unwrap_or(StandingTag::Install),
                &[],
            ),
        )?;
        for (def, router) in route.decl.router.defs().iter().zip(routers) {
            mounted.push(MountedGatewayRoute {
                router,
                protection: protection_of(&def.spec),
            });
        }
    }
    Ok(mounted)
}
