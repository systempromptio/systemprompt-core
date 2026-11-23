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
pub mod count_tokens;
pub mod messages;
pub mod models;
#[cfg(feature = "otel")]
pub mod otel;
pub mod provider_credentials;
pub mod registered;
pub mod sessions;

pub mod access_log;
mod routers;

use axum::routing::get;
use axum::{Extension, Router};
use std::sync::Arc;
use systemprompt_extension::Admitted;
use systemprompt_gateway::GatewayEngines;
use systemprompt_runtime::{AppContext, Kernel};

use self::access_log::log_gateway_request;
use self::builtin::confined;
#[cfg(feature = "otel")]
use self::routers::otel_routes;
use self::routers::{
    bridge_auth_routes, bridge_profile_routes, bridge_session_routes, inference_routes,
};
use crate::kernel::JwtContextExtractor;

pub(crate) use self::access_log::GatewayLogIdentity;

pub fn gateway_repositories(
    ctx: &AppContext,
) -> anyhow::Result<systemprompt_gateway::GatewayRepositories> {
    let journal = systemprompt_gateway::audit::journal::GatewayJournal::open(
        ctx.app_paths().storage().data(),
        ctx.secrets().encryption_master_key()?,
    )?;
    let profile = ctx.profile();
    let repos = systemprompt_gateway::GatewayRepositories::new(
        ctx.db_pool(),
        journal,
        Arc::new(crate::repository::context_provider(ctx.db_pool())),
        systemprompt_gateway::GatewayHandles {
            background: ctx.background_tasks().clone(),
            cache_epochs: ctx.cache_epochs().clone(),
            logging: ctx.logging().clone(),
            services: ctx.services_state().view(),
            subject_providers: ctx.subject_providers().clone(),
            at_rest: systemprompt_security::at_rest::AtRestCipher::from_secrets(
                &ctx.secrets().get(),
            )?,
            unapplied_plane: profile.unapplied_plane_mode(),
        },
    )
    .with_payload_cap(profile.payload_cap_bytes())
    .with_quota_sharding(profile.quota_config());
    let repos = match ctx
        .kernel()
        .state()
        .get::<Arc<systemprompt_mcp::ArtifactIngest>>()
    {
        Ok(ingest) => repos.with_artifact_ingest(Arc::clone(ingest)),
        Err(_) => repos,
    };
    let repos = match ctx.session_store() {
        Some(sessions) => repos.with_session_store(sessions),
        None => repos,
    };
    let repos = repos.with_brain_recall(profile.brain.recall.clone());
    let brain_sink = ctx
        .kernel()
        .state()
        .find::<systemprompt_extension::DynBrainSink>()
        .filter(|_| profile.brain.enabled)
        .cloned();
    let repos = match brain_sink {
        Some(sink) => {
            repos.with_brain_session(systemprompt_gateway::audit::session::SessionCapture::new(
                sink,
                profile.brain.capture.clone(),
                Arc::clone(ctx.db_pool()),
            ))
        },
        None => repos,
    };
    Ok(repos)
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
    engines: &Arc<GatewayEngines>,
    admitted: &Admitted,
    kernel: &Kernel,
) -> anyhow::Result<GatewayMountParts> {
    let jwt_extractor = Arc::new(JwtContextExtractor::from_app_context(ctx)?);
    let gateway_repos = Arc::new(gateway_repositories(ctx)?);

    let traffic = Router::new()
        .merge(inference_routes(
            ctx,
            &jwt_extractor,
            &gateway_repos,
            engines,
        ))
        .merge(bridge_profile_routes(ctx, &jwt_extractor))
        .merge(bridge_session_routes(ctx, &jwt_extractor));
    let registered = registered::admitted_gateway_routes(admitted, kernel, &jwt_extractor)?;
    #[cfg(feature = "otel")]
    let traffic = traffic.merge(otel_routes(ctx, &jwt_extractor));
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
    let engines = gateway_engines(kernel).ok_or_else(|| {
        anyhow::anyhow!("the gateway member is not admitted; no gateway engines are built")
    })?;
    let parts = gateway_mount_parts(ctx, &engines, admitted, kernel)?;
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
