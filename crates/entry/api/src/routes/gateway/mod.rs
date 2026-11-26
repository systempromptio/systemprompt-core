//! LLM gateway mount: the layering around the routes the `gateway-http` and
//! `identity-http` members build.
//!
//! `gateway_mount_parts` calls the `gateway-http` member's
//! [`GatewaySurface`](systemprompt_extension::router::GatewaySurface) with
//! that member's route-scoped state and hands the server its parts: the
//! inference and bridge traffic, the principal-bound routes (placed behind
//! `registered::verified`, the kernel's gateway verifier), the public session
//! mint, the bridge credential exchange `identity-http` builds, and every
//! admitted capability gateway route ([`registered`]). The server protects
//! each part handler-verified by the gateway credential with its own rate
//! limit and bearer window and wraps everything in the access log the
//! `gateway` member's operations layer on; [`gateway_router`] returns the
//! same routes unlimited for tests. A member that is not admitted contributes
//! nothing, so its routes answer 404.
//!
//! The bridge credential-exchange half is budgeted apart from inference
//! traffic because the two have opposite shapes: inference is high-volume and
//! elastic, sign-in is a handful of requests that must succeed. Sharing one
//! budget lets a saturated gateway refuse every credential exchange, which
//! presents to the user as a rejected token rather than as the rate limit it
//! is. An admitted capability route (the bridge inbox) carries its own budget,
//! so its polling and inference never starve each other.
//!
//! The bridge Drive routes are the `files-http` member's
//! [`DriveSurface`](systemprompt_extension::router::DriveSurface), built
//! with that member's route-scoped state: they hold the `DynDriveTokens` view
//! the `documents` member confers on `files-http`, never the token
//! authority, and do not depend on `gateway-http`'s standing. They are
//! mounted when `files-http` is admitted and the document store is enabled.
//!
//! [`gateway_operations`] reads the `gateway` member's sealed
//! [`GatewayOperations`](systemprompt_extension::router::GatewayOperations);
//! their presence is the one test of whether the gateway is composed.
//!
//! The access log layer is outside every limiter so that a refused request
//! still produces a record; an instance 429ing every sign-in shows it in its
//! logs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod builtin;
pub mod registered;

use axum::Router;
use systemprompt_extension::Admitted;
use systemprompt_extension::reserved::CoreId;
use systemprompt_extension::router::{
    DynConsumerCatalog, DynDriveSurface, DynGatewayOperations, DynGatewaySurface,
    FilesSurfaceError, GatewayRouteState, GatewaySurfaceError, GatewaySurfaceRoutes,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::auth::DynRequestAuthenticator;
use systemprompt_provider_contracts::DependencyProvider;
use systemprompt_runtime::{AppContext, Kernel};

use self::builtin::confined;

fn surface_routes(
    admitted: &Admitted,
    kernel: &Kernel,
) -> Result<GatewaySurfaceRoutes, GatewaySurfaceError> {
    let owner = ExtensionId::new(CoreId::GatewayHttp.id());
    let (Some(standing), Some(surface)) = (
        admitted.built().standing_of(&owner),
        kernel.state().find::<DynGatewaySurface>(),
    ) else {
        return Ok(GatewaySurfaceRoutes::default());
    };
    surface.build(GatewayRouteState {
        resolved: kernel.route_scoped(&owner, standing, &[]),
    })
}

fn drive_routes(admitted: &Admitted, kernel: &Kernel) -> Result<Router, FilesSurfaceError> {
    let owner = ExtensionId::new(CoreId::FilesHttp.id());
    let (Some(standing), Some(surface)) = (
        admitted.built().standing_of(&owner),
        kernel.state().find::<DynDriveSurface>(),
    ) else {
        return Ok(Router::new());
    };
    Ok(surface
        .build(&kernel.route_scoped(&owner, standing, &[]))?
        .unwrap_or_default())
}

pub(crate) fn provided_by<T: Send + Sync + 'static>(
    kernel: &Kernel,
    owner: &str,
) -> Result<(), DependencyProvider> {
    match kernel.state().provider_of::<T>() {
        Some(DependencyProvider::Extension(id)) if id.as_str() == owner => Ok(()),
        Some(other) => Err(other.clone()),
        None => Err(DependencyProvider::Host),
    }
}

pub(crate) fn gateway_mount_parts(
    ctx: &AppContext,
    admitted: &Admitted,
    kernel: &Kernel,
    bridge_auth: Router,
) -> anyhow::Result<GatewayMountParts> {
    let authenticator = ctx
        .kernel()
        .state()
        .get::<DynRequestAuthenticator>()
        .cloned()
        .map_err(|_missing| {
            systemprompt_extension::LoaderError::missing_provider::<DynRequestAuthenticator>(
                ExtensionId::new("users"),
            )
        })?;
    let routes = surface_routes(admitted, kernel)?;
    let registered = registered::admitted_gateway_routes(admitted, kernel, &authenticator)?;
    let principal_bound = registered::verified(routes.principal_bound, &authenticator);
    let drive = drive_routes(admitted, kernel)?;
    let traffic = routes.traffic.merge(principal_bound).merge(drive);
    Ok(GatewayMountParts {
        traffic: confined(traffic),
        bridge_auth: confined(bridge_auth),
        public_sessions: routes.public_sessions,
        consumer_catalog: routes.consumer_catalog,
        registered,
    })
}

pub fn gateway_operations(kernel: &Kernel) -> Option<DynGatewayOperations> {
    kernel.state().find::<DynGatewayOperations>().cloned()
}

pub fn gateway_router(
    ctx: &AppContext,
    admitted: &Admitted,
    kernel: &Kernel,
) -> anyhow::Result<Router> {
    let gateway = gateway_operations(kernel).ok_or_else(|| {
        anyhow::anyhow!("the gateway member is not admitted; no gateway route is built")
    })?;
    let bridge_auth = crate::services::server::routes::identity_bridge_auth(ctx, admitted, kernel)?
        .unwrap_or_default();
    let parts = gateway_mount_parts(ctx, admitted, kernel, bridge_auth)?;
    let routes = parts
        .registered
        .into_iter()
        .fold(parts.traffic.merge(parts.bridge_auth), |routes, group| {
            routes.merge(group.router)
        });
    Ok(gateway.access_log(&ctx.kernel().resolved(), routes)?)
}

pub(crate) struct GatewayMountParts {
    pub(crate) traffic: Router,
    pub(crate) bridge_auth: Router,
    pub(crate) public_sessions: Router,
    pub(crate) consumer_catalog: Option<DynConsumerCatalog>,
    pub(crate) registered: Vec<registered::RegisteredGatewayRoutes>,
}

impl std::fmt::Debug for GatewayMountParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayMountParts").finish_non_exhaustive()
    }
}
