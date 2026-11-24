//! LLM gateway mount: the layering around the routes the `gateway-http` and
//! `identity-http` members build.
//!
//! `gateway_mount_parts` calls the `gateway-http` member's
//! [`GatewaySurface`](systemprompt_gateway::GatewaySurface) with that
//! member's route-scoped state and hands the server its parts: the inference
//! and bridge traffic, the principal-bound routes (placed behind
//! `registered::verified`, the kernel's gateway verifier), the public
//! session mint, the bridge credential exchange `identity-http` builds, and
//! every admitted capability gateway route ([`registered`]). The server
//! protects each part handler-verified by the gateway credential with its own
//! rate limit and bearer window and wraps everything in `with_access_log`;
//! [`gateway_router`] returns the same routes unlimited for tests. A member
//! that is not admitted contributes nothing, so its routes answer 404.
//!
//! The bridge credential-exchange half is budgeted apart from inference
//! traffic because the two have opposite shapes: inference is high-volume and
//! elastic, sign-in is a handful of requests that must succeed. Sharing one
//! budget lets a saturated gateway refuse every credential exchange, which
//! presents to the user as a rejected token rather than as the rate limit it
//! is. An admitted capability route (the bridge inbox) carries its own budget,
//! so its polling and inference never starve each other.
//!
//! The bridge Drive routes are mounted whenever the `documents` member is
//! admitted and the document store is enabled; they hold the sealed
//! `DynDriveTokens` view the kernel reads, never the token authority, and do
//! not depend on `gateway-http`'s standing. The view is read by type, so the
//! mount checks that the `documents` member provided it and refuses boot with
//! [`GatewayBindingError::ForeignDriveTokens`] when any other member did.
//!
//! The gateway repositories are built from the unscoped kernel resolution
//! only for the boot paths inside this crate (settlement recovery, the brain
//! recall sink); no other crate reaches that builder.
//!
//! [`gateway_engines`] builds the engines from the resolved kernel: the
//! adapter, scanner, override and route-selector sets are the gateway's built
//! slots and the guards are the kernel-built set of every admitted member's
//! guard factory, so nothing a disabled member contributed runs.
//!
//! The access log layer is outside every limiter so that a refused request
//! still produces a record; an instance 429ing every sign-in shows it in its
//! logs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod access_log;
pub mod builtin;
pub mod registered;

use axum::Router;
use std::sync::Arc;
use systemprompt_documents::extension::DOCUMENTS_EXTENSION;
use systemprompt_extension::Admitted;
use systemprompt_extension::reserved::CoreId;
use systemprompt_gateway::surface::ManagedConsumer;
use systemprompt_gateway::{
    DynGatewaySurface, GatewayEngines, GatewayHostHandles, GatewayRouteState, GatewaySurfaceError,
    GatewaySurfaceRoutes,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_logging::LoggingRuntime;
use systemprompt_models::auth::DynRequestAuthenticator;
use systemprompt_provider_contracts::DependencyProvider;
use systemprompt_runtime::{AppContext, Kernel};
use systemprompt_traits::{AppContext as _, DynDriveTokens};

use self::access_log::log_gateway_request;
use self::builtin::confined;

fn surface_routes(
    ctx: &AppContext,
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
        host: host_handles(ctx),
    })
}

pub fn host_handles(ctx: &AppContext) -> GatewayHostHandles {
    let profile = ctx.profile();
    GatewayHostHandles {
        services: ctx.services_state().clone(),
        governance: ctx.governance_watch().clone(),
        authz_hook: Arc::clone(ctx.authz_hook()),
        analytics: ctx.analytics_provider(),
        logging: ctx.logging().clone(),
        payload_cap_bytes: profile.payload_cap_bytes(),
        quota: profile.quota_config(),
        unapplied_plane: profile.unapplied_plane_mode(),
        brain: profile.brain.clone(),
        tenant_id: profile
            .cloud
            .as_ref()
            .and_then(|cloud| cloud.tenant_id.clone()),
    }
}

pub(crate) fn gateway_repositories(
    ctx: &AppContext,
) -> Result<systemprompt_gateway::GatewayRepositories, GatewaySurfaceError> {
    systemprompt_gateway::surface::repositories::gateway_repositories(
        &ctx.kernel().resolved(),
        &host_handles(ctx),
        ctx.kernel()
            .resolved()
            .get::<systemprompt_traits::DynContextMaterializer>()?,
        ctx.db_pool(),
    )
}

fn drive_routes(ctx: &AppContext, kernel: &Kernel) -> Result<Router, GatewayBindingError> {
    let (Some(service), Ok(tokens)) = (
        crate::routes::documents::document_service(ctx),
        kernel.resolved().get::<DynDriveTokens>(),
    ) else {
        return Ok(Router::new());
    };
    provided_by::<DynDriveTokens>(kernel, DOCUMENTS_EXTENSION)
        .map_err(|provider| GatewayBindingError::ForeignDriveTokens { provider })?;
    Ok(crate::routes::documents::drive::router(
        crate::routes::documents::drive::DriveState::new(service, tokens),
    ))
}

/// A value the gateway mount reads by type from the unscoped kernel state was
/// provided by a member other than its owner.
#[derive(Debug, thiserror::Error)]
pub enum GatewayBindingError {
    #[error("the Drive token view is provided by {provider}, not the documents member")]
    ForeignDriveTokens { provider: DependencyProvider },
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
    let routes = surface_routes(ctx, admitted, kernel)?;
    let registered = registered::admitted_gateway_routes(admitted, kernel, &authenticator)?;
    let principal_bound = registered::verified(routes.principal_bound, &authenticator);
    let drive = drive_routes(ctx, kernel)?;
    let traffic = routes.traffic.merge(principal_bound).merge(drive);
    Ok(GatewayMountParts {
        traffic: confined(traffic),
        bridge_auth: confined(bridge_auth),
        public_sessions: routes.public_sessions,
        managed_consumer: routes.managed_consumer,
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
    let bridge_auth = crate::services::server::routes::identity_bridge_auth(ctx, admitted, kernel)?
        .unwrap_or_default();
    let parts = gateway_mount_parts(ctx, admitted, kernel, bridge_auth)?;
    let routes = parts
        .registered
        .into_iter()
        .fold(parts.traffic.merge(parts.bridge_auth), |routes, group| {
            routes.merge(group.router)
        });
    Ok(with_access_log(ctx.logging(), routes))
}

pub(crate) struct GatewayMountParts {
    pub(crate) traffic: Router,
    pub(crate) bridge_auth: Router,
    pub(crate) public_sessions: Router,
    pub(crate) managed_consumer: Option<ManagedConsumer>,
    pub(crate) registered: Vec<registered::RegisteredGatewayRoutes>,
}

impl std::fmt::Debug for GatewayMountParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayMountParts").finish_non_exhaustive()
    }
}

pub(crate) fn with_access_log(logging: &LoggingRuntime, router: Router) -> Router {
    router.layer(axum::middleware::from_fn_with_state(
        logging.clone(),
        log_gateway_request,
    ))
}
