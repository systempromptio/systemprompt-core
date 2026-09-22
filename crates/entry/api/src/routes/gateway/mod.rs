//! LLM gateway router and its access log.
//!
//! [`gateway_router`] assembles the bridge-facing surface: the `/messages`,
//! `/responses`, and `/chat/completions` proxy endpoints (each bound to an
//! [`InboundAdapter`](crate::services::gateway::protocol::InboundAdapter)), the
//! `/auth/bridge/*` credential-exchange routes ([`auth`]), the `/bridge/*`
//! manifest and heartbeat routes, the credential-gated `/otel` ingest
//! ([`otel`]), and `/models`. The router is gated on the availability of the
//! analytics, user, and JTI-revocation providers; if any is missing it returns
//! `None` and the gateway stays unmounted. `log_gateway_request` is the
//! middleware that records every request to the logging repository.
//!
//! The surface is assembled in two halves so that the server can give each its
//! own rate-limit budget: [`gateway_mount_router`] is what the server mounts,
//! while [`gateway_router`] returns the same routes unlimited for tests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod auth;
pub mod bridge;
pub mod bridge_data;
pub mod bridge_device;
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

pub fn gateway_enabled(ctx: &AppContext) -> bool {
    ctx.analytics_provider().is_some()
        && ctx.session_provider().is_some()
        && ctx.user_provider().is_some()
}

fn build_jwt_extractor(ctx: &AppContext) -> Option<Arc<JwtContextExtractor>> {
    let Some(analytics) = ctx.session_provider() else {
        tracing::warn!("Gateway router: analytics provider unavailable — gateway disabled");
        return None;
    };
    let Some(user_provider) = ctx.user_provider() else {
        tracing::warn!("Gateway router: user provider unavailable — gateway disabled");
        return None;
    };
    let jti_revocation =
        JtiRevocationChecker::from_repository(ctx.oauth_repositories().oauth.clone());
    Some(Arc::new(JwtContextExtractor::new(
        analytics,
        user_provider,
        jti_revocation,
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
    )?
    .with_artifact_ingest(ctx.artifact_ingest_arc())
    .with_session_store(ctx.session_store())
    .with_payload_cap(payload_cap_bytes))
}

/// The gateway surface split by rate-limit budget.
///
/// `bridge_auth` is separate from `traffic` because the two have opposite
/// shapes: inference is high-volume and elastic, sign-in is a handful of
/// requests that must succeed. Sharing one budget let a saturated gateway
/// refuse every credential exchange, which presents to the user as a rejected
/// token rather than as the rate limit it is.
struct GatewayParts {
    traffic: Router,
    bridge_auth: Router,
}

fn gateway_parts(ctx: &AppContext) -> anyhow::Result<Option<GatewayParts>> {
    let Some(jwt_extractor) = build_jwt_extractor(ctx) else {
        return Ok(None);
    };
    let gateway_repos = Arc::new(gateway_repositories(ctx)?);

    Ok(Some(GatewayParts {
        traffic: Router::new()
            .merge(inference_routes(ctx, &jwt_extractor, &gateway_repos))
            .merge(bridge_profile_routes(ctx, &jwt_extractor))
            .merge(bridge_session_routes(ctx, &jwt_extractor))
            .merge(bridge_release_routes(&jwt_extractor))
            .merge(otel_routes(ctx, &jwt_extractor))
            .route("/models", get(models::list))
            .route("/", get(models::root)),
        bridge_auth: bridge_auth_routes(ctx, &jwt_extractor),
    }))
}

/// The whole gateway with no rate limiting, for tests and for callers that
/// mount it behind their own limiter.
pub fn gateway_router(ctx: &AppContext) -> anyhow::Result<Option<Router>> {
    Ok(gateway_parts(ctx)?.map(|parts| common_layers(ctx, parts.traffic.merge(parts.bridge_auth))))
}

/// The gateway as the server mounts it: each half behind its own rate-limit
/// budget, with the access log outside both so a refusal is recorded rather
/// than discarded.
pub fn gateway_mount_router(
    ctx: &AppContext,
    limits: &RateLimitState,
) -> anyhow::Result<Option<Router>> {
    let Some(parts) = gateway_parts(ctx)? else {
        return Ok(None);
    };
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

    Ok(Some(common_layers(ctx, traffic.merge(bridge_auth))))
}

/// Layers every gateway route carries whatever its budget.
///
/// The access log is outermost so that a request refused by the rate limiter
/// still produces a record. It sat inside the limiter until 2026-09-22, which
/// is why an instance that was 429ing every sign-in showed nothing at all in
/// its logs.
fn common_layers(ctx: &AppContext, router: Router) -> Router {
    router
        .layer(Extension(ctx.clone()))
        .layer(axum::middleware::from_fn(log_gateway_request))
}
