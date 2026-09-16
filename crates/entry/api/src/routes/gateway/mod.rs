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
pub mod bridge_stream;
pub mod bridge_whoami;
pub mod messages;
pub mod models;
pub mod otel;
pub mod sessions;

mod access_log;
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
use crate::services::middleware::{JtiRevocationChecker, JwtContextExtractor};

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
        systemprompt_config::ProfileBootstrap::get_path()?,
        systemprompt_config::SecretsBootstrap::get()?,
    )?;
    Ok(crate::services::gateway::GatewayRepositories::new(
        ctx.db_pool(),
        journal,
        ctx.context_materializer(),
    )?)
}

pub fn gateway_router(ctx: &AppContext) -> anyhow::Result<Option<Router>> {
    let Some(jwt_extractor) = build_jwt_extractor(ctx) else {
        return Ok(None);
    };
    let gateway_repos = Arc::new(gateway_repositories(ctx)?);

    Ok(Some(
        Router::new()
            .merge(inference_routes(ctx, &jwt_extractor, &gateway_repos))
            .merge(bridge_auth_routes(ctx, &jwt_extractor))
            .merge(bridge_profile_routes(ctx, &jwt_extractor))
            .merge(bridge_session_routes(ctx, &jwt_extractor))
            .merge(bridge_release_routes(&jwt_extractor))
            .merge(otel_routes(ctx, &jwt_extractor))
            .route("/models", get(models::list))
            .route("/", get(models::root))
            .layer(Extension(ctx.clone()))
            .layer(axum::middleware::from_fn(log_gateway_request)),
    ))
}
