//! LLM gateway router and its access log.
//!
//! [`gateway_router`] assembles the bridge-facing surface: the `/messages`,
//! `/responses`, and `/chat/completions` proxy endpoints (each bound to an
//! [`InboundAdapter`](crate::services::gateway::protocol::InboundAdapter)), the
//! `/auth/bridge/*` credential-exchange routes ([`auth`]), the `/bridge/*`
//! manifest and heartbeat routes, the unauthenticated `/otel` ingest
//! ([`otel`]), and `/models`. The router is gated on the availability of the
//! analytics, user, and JTI-revocation providers; if any is missing it returns
//! `None` and the gateway stays unmounted. `log_gateway_request` is the
//! middleware that records every request to the logging repository.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod auth;
pub mod bridge;
pub mod bridge_data;
pub mod bridge_heartbeat;
pub mod bridge_manifest;
pub mod bridge_plugin_file;
pub mod bridge_profile_usage;
pub mod bridge_release;
pub mod bridge_stream;
pub mod bridge_whoami;
pub mod messages;
pub mod models;
pub mod otel;
pub mod sessions;

mod access_log;
mod routers;

use axum::routing::{get, post};
use axum::{Extension, Router};
use std::sync::Arc;
use systemprompt_runtime::AppContext;
use systemprompt_traits::AppContext as _;

use self::access_log::log_gateway_request;
use self::routers::{
    bridge_auth_routes, bridge_profile_routes, bridge_release_routes, bridge_session_routes,
    inference_routes,
};
use crate::services::middleware::{JtiRevocationChecker, JwtContextExtractor};

pub(crate) use self::access_log::GatewayLogIdentity;

fn build_jwt_extractor(ctx: &AppContext) -> Option<Arc<JwtContextExtractor>> {
    let Some(analytics) = ctx.analytics_provider() else {
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

pub fn gateway_router(ctx: &AppContext) -> Option<Router> {
    let jwt_extractor = build_jwt_extractor(ctx)?;
    let gateway_repos = crate::services::gateway::GatewayRepositories::new(
        ctx.db_pool(),
        ctx.context_materializer(),
    )
    .inspect_err(|e| tracing::error!(error = %e, "Gateway repositories init failed"))
    .ok()
    .map(Arc::new)?;

    Some(
        Router::new()
            .merge(inference_routes(ctx, &jwt_extractor, &gateway_repos))
            .merge(bridge_auth_routes(ctx, &jwt_extractor))
            .merge(bridge_profile_routes(ctx, &jwt_extractor))
            .merge(bridge_session_routes(ctx, &jwt_extractor))
            .merge(bridge_release_routes(&jwt_extractor))
            .route(
                "/otel",
                post(|request| async move { otel::handle(request).await }),
            )
            .route(
                "/otel/{*rest}",
                post(|request| async move { otel::handle(request).await }),
            )
            .route("/models", get(models::list))
            .route("/", get(models::root))
            .layer(Extension(ctx.clone()))
            .layer(axum::middleware::from_fn(log_gateway_request)),
    )
}
