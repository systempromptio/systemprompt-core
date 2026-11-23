//! LLM gateway router and its access log.
//!
//! [`gateway_router`] assembles the bridge-facing surface: the `/messages`,
//! `/responses`, and `/chat/completions` proxy endpoints (each bound to an
//! [`InboundAdapter`](systemprompt_gateway::protocol::InboundAdapter)), the
//! `/auth/bridge/*` credential-exchange routes ([`auth`]), the `/bridge/*`
//! manifest and heartbeat routes, the credential-gated `/otel` ingest
//! (`otel` feature), and `/models`. The router requires the session and user
//! providers; if either is missing, building it fails and so does startup.
//! `log_gateway_request` is the middleware that records every request to the
//! logging repository.
//!
//! The surface is assembled in parts so that the server can give each its own
//! rate-limit budget: `gateway_mount_parts` hands the server the inference
//! traffic, the bridge credential exchange and every admitted capability
//! gateway route ([`registered`]), which it protects handler-verified by the
//! gateway credential with their rate limits and bearer windows and wraps in
//! `with_access_log`, while [`gateway_router`] returns the same routes
//! unlimited for tests. The bridge credential-exchange half is budgeted apart
//! from inference traffic because the two have opposite shapes: inference is
//! high-volume and elastic, sign-in is a handful of requests that must
//! succeed. Sharing one budget lets a saturated gateway refuse every
//! credential exchange, which presents to the user as a rejected token rather
//! than as the rate limit it is. An admitted capability route (the bridge
//! inbox) carries its own budget, so its polling and inference never starve
//! each other.
//!
//! [`gateway_engines`] builds the engines from the resolved kernel: the
//! adapter, scanner, override and route-selector sets are the gateway's built
//! slots and the guards are the kernel-built set of every admitted member's
//! guard factory, so nothing a disabled member contributed runs.
//!
//! The access log layer is outside both limiters so that a refused request
//! still produces a record; an instance 429ing every sign-in shows it in its
//! logs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod auth;
pub mod bridge;
pub mod bridge_admission;
pub mod bridge_data;
pub mod bridge_device;
pub mod bridge_error;
pub mod bridge_heartbeat;
pub mod bridge_manifest;
pub mod bridge_plugin_archive;
pub mod bridge_plugin_file;
pub mod bridge_profile_usage;
pub mod bridge_release;
pub mod bridge_resolved;
pub mod bridge_stream;
pub mod bridge_whoami;
pub mod builtin;
pub mod device_link;
pub mod models;
pub mod provider_credentials;
pub mod registered;
pub mod sessions;

pub mod access_log;
mod routers;

use axum::routing::get;
use axum::{Extension, Router};
use std::sync::Arc;
use systemprompt_extension::Admitted;
use systemprompt_extension::reserved::CoreId;
use systemprompt_gateway::{
    DynGatewaySurface, GatewayEngines, GatewayHostHandles, GatewayRouteState, GatewaySurfaceError,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_runtime::{AppContext, Kernel};
use systemprompt_traits::AppContext as _;

use self::access_log::log_gateway_request;
use self::builtin::confined;
use self::routers::{bridge_auth_routes, bridge_profile_routes, bridge_session_routes};
use crate::kernel::JwtContextExtractor;

fn surface_routes(
    ctx: &AppContext,
    admitted: &Admitted,
    kernel: &Kernel,
) -> Result<Router, GatewaySurfaceError> {
    let owner = ExtensionId::new(CoreId::GatewayHttp.id());
    let (Some(standing), Some(surface)) = (
        admitted.built().standing_of(&owner),
        kernel.state().find::<DynGatewaySurface>(),
    ) else {
        return Ok(Router::new());
    };
    surface.build(GatewayRouteState {
        resolved: kernel.route_scoped(&owner, standing, &[]),
        host: host_handles(ctx),
    })
}

fn host_handles(ctx: &AppContext) -> GatewayHostHandles {
    let profile = ctx.profile();
    GatewayHostHandles {
        services: ctx.services_state().clone(),
        governance: ctx.governance_watch().clone(),
        authz_hook: ctx.authz_hook().clone(),
        analytics: ctx.analytics_provider(),
        logging: ctx.logging().clone(),
        payload_cap_bytes: profile.payload_cap_bytes(),
        quota: profile.quota_config(),
        unapplied_plane: profile.unapplied_plane_mode(),
        brain: profile.brain.clone(),
    }
}

pub fn gateway_repositories(
    ctx: &AppContext,
) -> Result<systemprompt_gateway::GatewayRepositories, GatewaySurfaceError> {
    systemprompt_gateway::surface::repositories::gateway_repositories(
        &ctx.kernel().resolved(),
        &host_handles(ctx),
        Arc::new(crate::repository::context_provider(ctx.db_pool())),
        ctx.db_pool(),
    )
}

fn drive_routes(ctx: &AppContext, jwt_extractor: &Arc<JwtContextExtractor>) -> Router {
    if !ctx.files_config().documents_config().is_enabled() {
        return Router::new();
    }
    crate::routes::documents::drive::router(crate::routes::documents::drive::DriveState::new(
        crate::routes::documents::document_service(ctx),
        Arc::clone(jwt_extractor),
        ctx.token_authority(),
    ))
}

pub(crate) fn gateway_mount_parts(
    ctx: &AppContext,
    admitted: &Admitted,
    kernel: &Kernel,
) -> anyhow::Result<GatewayMountParts> {
    let jwt_extractor = Arc::new(JwtContextExtractor::from_kernel(ctx.kernel().state())?);

    let traffic = Router::new()
        .merge(surface_routes(ctx, admitted, kernel)?)
        .merge(bridge_profile_routes(ctx, &jwt_extractor))
        .merge(bridge_session_routes(ctx, &jwt_extractor));
    let registered = registered::admitted_gateway_routes(admitted, kernel, &jwt_extractor)?;
    let traffic = traffic
        .merge(drive_routes(ctx, &jwt_extractor))
        .merge(provider_credentials::routes(ctx, &jwt_extractor))
        .merge(bridge_release::routes(ctx, &jwt_extractor)?)
        .route("/models", get(models::list).with_state(ctx.clone()))
        .route("/", get(models::root));
    Ok(GatewayMountParts {
        traffic: confined(traffic),
        bridge_auth: confined(bridge_auth_routes(ctx, &jwt_extractor)),
        registered,
    })
}

pub fn gateway_engines(kernel: &Kernel) -> Option<Arc<GatewayEngines>> {
    kernel.state().find::<Arc<GatewayEngines>>().cloned()
}

pub fn gateway_router(
    ctx: &AppContext,
    admitted: &Admitted,
    kernel: &Kernel,
) -> anyhow::Result<Router> {
    gateway_engines(kernel).ok_or_else(|| {
        anyhow::anyhow!("the gateway member is not admitted; no gateway engines are built")
    })?;
    let parts = gateway_mount_parts(ctx, admitted, kernel)?;
    let routes = parts
        .registered
        .into_iter()
        .fold(parts.traffic.merge(parts.bridge_auth), |routes, group| {
            routes.merge(group.router)
        });
    Ok(with_access_log(ctx, routes))
}

#[derive(Debug)]
pub(crate) struct GatewayMountParts {
    pub(crate) traffic: Router,
    pub(crate) bridge_auth: Router,
    pub(crate) registered: Vec<registered::RegisteredGatewayRoutes>,
}

pub(crate) fn with_access_log(ctx: &AppContext, router: Router) -> Router {
    router
        .layer(Extension(ctx.clone()))
        .layer(axum::middleware::from_fn_with_state(
            ctx.logging().clone(),
            log_gateway_request,
        ))
}
