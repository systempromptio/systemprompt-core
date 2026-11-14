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
//! The surface is assembled in two halves so that the server can give each its
//! own rate-limit budget: `gateway_mount_parts` hands the server the three
//! halves, which it declares handler-verified by the gateway credential with
//! their rate limits and bearer windows and wraps in `with_access_log`,
//! while [`gateway_router`] returns the same routes unlimited for tests. The
//! bridge credential-exchange half is budgeted apart from inference traffic
//! because the two have opposite shapes: inference is high-volume and elastic,
//! sign-in is a handful of requests that must succeed. Sharing one budget let
//! a saturated gateway refuse every credential exchange, which presents to the
//! user as a rejected token rather than as the rate limit it is. The bridge
//! inbox routes take the notifications budget, so inbox polling and inference
//! never starve each other.
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
#[cfg(feature = "notifications")]
mod bridge_notifications;
pub mod bridge_plugin_archive;
pub mod bridge_plugin_file;
pub mod bridge_profile_usage;
pub mod bridge_release;
pub mod bridge_resolved;
pub mod bridge_stream;
pub mod bridge_whoami;
pub mod count_tokens;
pub mod messages;
pub mod models;
#[cfg(feature = "otel")]
pub mod otel;
pub mod provider_credentials;
pub mod sessions;

pub mod access_log;
mod routers;

use axum::routing::get;
use axum::{Extension, Router};
use std::sync::Arc;
use systemprompt_gateway::GatewayEngines;
use systemprompt_runtime::AppContext;

use self::access_log::log_gateway_request;
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
        ctx.context_materializer(),
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
    .with_artifact_ingest(ctx.artifact_ingest_arc())
    .with_session_store(ctx.session_store())
    .with_payload_cap(profile.payload_cap_bytes())
    .with_quota_sharding(profile.quota_config());
    #[cfg(feature = "brain")]
    let repos = repos.with_brain_recall(profile.brain.recall.clone());
    #[cfg(feature = "brain")]
    let repos = match ctx.brain() {
        Some(brain) => {
            repos.with_brain_session(systemprompt_gateway::audit::session::SessionCapture::new(
                Arc::clone(brain.sink()),
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
    #[cfg(feature = "notifications")]
    let notifications = bridge_notifications::routes(ctx, &jwt_extractor);
    #[cfg(not(feature = "notifications"))]
    let notifications = Router::new();
    #[cfg(feature = "otel")]
    let traffic = traffic.merge(otel_routes(ctx, &jwt_extractor));
    Ok(GatewayMountParts {
        traffic: traffic
            .merge(drive_routes(ctx, &jwt_extractor))
            .merge(provider_credentials::routes(ctx, &jwt_extractor))
            .merge(bridge_release::routes(ctx, &jwt_extractor)?)
            .route("/models", get(models::list).with_state(ctx.clone()))
            .route("/", get(models::root)),
        bridge_auth: bridge_auth_routes(ctx, &jwt_extractor),
        notifications,
    })
}

pub fn gateway_engines(
    ctx: &AppContext,
) -> Result<GatewayEngines, systemprompt_gateway::GatewayEnginesError> {
    let extensions: Arc<dyn systemprompt_extension::ExtensionContext> = Arc::new(ctx.clone());
    let engines = GatewayEngines::new(
        Arc::clone(ctx.connector_tokens()),
        &extensions,
        Arc::clone(ctx.secrets()),
    )?;
    engines.refuse_shadowed_caller_wires(&ctx.services_state().view().catalog)?;
    #[cfg(feature = "brain")]
    let engines = match ctx.brain_recall() {
        Some(recall) => engines.with_memory_recall(recall),
        None => engines,
    };
    Ok(engines)
}

pub fn gateway_router(ctx: &AppContext) -> anyhow::Result<Router> {
    let parts = gateway_mount_parts(ctx, &Arc::new(gateway_engines(ctx)?))?;
    Ok(with_access_log(
        ctx,
        parts
            .traffic
            .merge(parts.bridge_auth)
            .merge(parts.notifications),
    ))
}

#[derive(Debug)]
pub(crate) struct GatewayMountParts {
    pub(crate) traffic: Router,
    pub(crate) bridge_auth: Router,
    pub(crate) notifications: Router,
}

pub(crate) fn with_access_log(ctx: &AppContext, router: Router) -> Router {
    router
        .layer(Extension(ctx.clone()))
        .layer(axum::middleware::from_fn_with_state(
            ctx.logging().clone(),
            log_gateway_request,
        ))
}
