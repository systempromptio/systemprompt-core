//! LLM gateway router and its access log.
//!
//! [`gateway_router`] assembles the bridge-facing surface: the `/messages`,
//! `/responses`, and `/chat/completions` proxy endpoints (each bound to an
//! [`InboundAdapter`](crate::services::gateway::protocol::InboundAdapter)), the
//! `/auth/bridge/*` credential-exchange routes ([`auth`]), the `/bridge/*`
//! manifest and heartbeat routes, the credential-gated `/otel` ingest
//! ([`otel`]), and `/models`. The router requires the session and user
//! providers; if either is missing, building it fails and so does startup.
//! `log_gateway_request` is the middleware that records every request to the
//! logging repository.
//!
//! The surface is assembled in two halves so that the server can give each its
//! own rate-limit budget: [`gateway_mount_router`] is what the server mounts,
//! while [`gateway_router`] returns the same routes unlimited for tests. The
//! bridge credential-exchange half is budgeted apart from inference traffic
//! because the two have opposite shapes: inference is high-volume and elastic,
//! sign-in is a handful of requests that must succeed. Sharing one budget let
//! a saturated gateway refuse every credential exchange, which presents to the
//! user as a rejected token rather than as the rate limit it is.
//!
//! The access log layer is outside both limiters so that a refused request
//! still produces a record. It sat inside the limiter until 2026-09-22, which
//! is why an instance that was 429ing every sign-in showed nothing at all in
//! its logs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod auth;
pub mod bridge;
pub mod bridge_data;
pub mod bridge_device;
pub mod bridge_error;
pub mod bridge_heartbeat;
pub mod bridge_manifest;
pub mod bridge_plugin_file;
pub mod bridge_profile_usage;
pub mod bridge_release;
pub mod bridge_resolved;
pub mod bridge_stream;
pub mod bridge_whoami;
pub mod messages;
pub mod models;
pub mod otel;
pub mod sessions;

pub mod access_log;
mod routers;

use axum::routing::get;
use axum::{Extension, Router};
use std::sync::Arc;
use systemprompt_runtime::AppContext;
use systemprompt_traits::AppContext as _;

use self::access_log::log_gateway_request;
use self::routers::{
    bridge_auth_routes, bridge_profile_routes, bridge_release_routes, bridge_session_routes,
    inference_routes, otel_routes,
};
use crate::services::middleware::{
    JtiRevocationChecker, JwtContextExtractor, RateLimitState, RouterExt,
};

pub(crate) use self::access_log::{GatewayLogIdentity, TerminalOutcome, log_gateway_terminal};

fn build_jwt_extractor(ctx: &AppContext) -> anyhow::Result<Arc<JwtContextExtractor>> {
    let sessions = ctx
        .session_provider()
        .ok_or_else(|| anyhow::anyhow!("gateway requires a session provider"))?;
    let user_provider = ctx
        .user_provider()
        .ok_or_else(|| anyhow::anyhow!("gateway requires a user provider"))?;
    let jti_revocation =
        JtiRevocationChecker::from_repository(ctx.oauth_repositories().oauth.clone());
    Ok(Arc::new(JwtContextExtractor::new(
        sessions,
        user_provider,
        jti_revocation,
        ctx.config().jwt_issuer.clone(),
    )))
}

pub fn gateway_repositories(
    ctx: &AppContext,
) -> anyhow::Result<crate::services::gateway::GatewayRepositories> {
    let journal = crate::services::gateway::audit::journal::GatewayJournal::open(
        ctx.app_paths().storage().data(),
        systemprompt_config::SecretsBootstrap::get()?,
    )?;
    let payload_cap_bytes = systemprompt_config::ProfileBootstrap::get()?.payload_cap_bytes();
    Ok(crate::services::gateway::GatewayRepositories::new(
        ctx.db_pool(),
        journal,
        ctx.context_materializer(),
    )
    .with_artifact_ingest(ctx.artifact_ingest_arc())
    .with_session_store(ctx.session_store())
    .with_payload_cap(payload_cap_bytes))
}

struct GatewayParts {
    traffic: Router,
    bridge_auth: Router,
}

fn gateway_parts(ctx: &AppContext) -> anyhow::Result<GatewayParts> {
    let jwt_extractor = build_jwt_extractor(ctx)?;
    let gateway_repos = Arc::new(gateway_repositories(ctx)?);

    Ok(GatewayParts {
        traffic: Router::new()
            .merge(inference_routes(ctx, &jwt_extractor, &gateway_repos))
            .merge(bridge_profile_routes(ctx, &jwt_extractor))
            .merge(bridge_session_routes(ctx, &jwt_extractor))
            .merge(bridge_release_routes(&jwt_extractor))
            .merge(otel_routes(ctx, &jwt_extractor))
            .route("/models", get(models::list))
            .route("/", get(models::root)),
        bridge_auth: bridge_auth_routes(ctx, &jwt_extractor),
    })
}

pub fn gateway_router(ctx: &AppContext) -> anyhow::Result<Router> {
    let parts = gateway_parts(ctx)?;
    Ok(common_layers(ctx, parts.traffic.merge(parts.bridge_auth)))
}

pub fn gateway_mount_router(ctx: &AppContext, limits: &RateLimitState) -> anyhow::Result<Router> {
    let parts = gateway_parts(ctx)?;
    let rate_config = &ctx.config().rate_limits;

    let traffic =
        parts
            .traffic
            .with_rate_limit(limits, rate_config.gateway_per_second, "gateway")?;
    let bridge_auth = parts.bridge_auth.with_rate_limit(
        limits,
        rate_config.bridge_auth_per_second,
        "bridge_auth",
    )?;

    Ok(common_layers(ctx, traffic.merge(bridge_auth)))
}

fn common_layers(ctx: &AppContext, router: Router) -> Router {
    router
        .layer(Extension(ctx.clone()))
        .layer(axum::middleware::from_fn(log_gateway_request))
}
