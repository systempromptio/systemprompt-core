//! LLM gateway router and its access log.
//!
//! [`gateway_router`] assembles the bridge-facing surface: the `/messages`
//! and `/responses` proxy endpoints (each bound to an
//! [`InboundAdapter`]), the
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
pub mod bridge_profile_usage;
pub mod bridge_whoami;
pub mod messages;
pub mod models;
pub mod otel;

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Router};
use std::sync::Arc;
use std::time::Instant;
use systemprompt_logging::{LogActor, LogEntry, LogLevel};
use systemprompt_runtime::AppContext;
use systemprompt_traits::AppContext as _;

use crate::services::gateway::protocol::inbound::InboundAdapter;
use crate::services::gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use crate::services::gateway::protocol::inbound::openai_responses::OpenAiResponsesInbound;
use crate::services::middleware::{JtiRevocationChecker, JwtContextExtractor};

async fn log_gateway_request(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let started = Instant::now();
    let resp = next.run(req).await;
    let status = resp.status().as_u16();
    let elapsed_ms = started.elapsed().as_millis() as u64;

    let metadata = serde_json::json!({
        "method": method.to_string(),
        "path": path,
        "status": status,
        "elapsed_ms": elapsed_ms,
    });

    let level = if status >= 500 {
        LogLevel::Error
    } else if status >= 400 {
        LogLevel::Warn
    } else {
        LogLevel::Info
    };

    if status >= 500 {
        tracing::error!(method = %method, path = %path, status, elapsed_ms, "gateway request failed");
    } else if status >= 400 {
        tracing::warn!(method = %method, path = %path, status, elapsed_ms, "gateway request rejected");
    } else {
        tracing::info!(method = %method, path = %path, status, elapsed_ms, "gateway request");
    }

    // Persist off the response hot path, via the background batch writer.
    match LogActor::platform(systemprompt_identifiers::TraceId::system()) {
        Ok(actor) => {
            let entry = LogEntry::new(
                level,
                "systemprompt_api::gateway",
                format!("{method} {path} -> {status} ({elapsed_ms}ms)"),
                actor,
            )
            .with_metadata(metadata);
            systemprompt_logging::enqueue_background(entry);
        },
        Err(e) => {
            tracing::warn!(error = %e, "gateway access log skipped: system admin not initialized");
        },
    }

    resp
}

fn build_jwt_extractor(ctx: &AppContext) -> Option<Arc<JwtContextExtractor>> {
    let Some(analytics) = ctx.analytics_provider() else {
        tracing::warn!("Gateway router: analytics provider unavailable — gateway disabled");
        return None;
    };
    let Some(user_provider) = ctx.user_provider() else {
        tracing::warn!("Gateway router: user provider unavailable — gateway disabled");
        return None;
    };
    let jti_revocation = match JtiRevocationChecker::from_pool(ctx.db_pool()) {
        Ok(checker) => checker,
        Err(e) => {
            tracing::warn!(error = %e, "Gateway router: JTI revocation unavailable — gateway disabled");
            return None;
        },
    };
    Some(Arc::new(JwtContextExtractor::new(
        analytics,
        user_provider,
        jti_revocation,
    )))
}

fn inference_routes(ctx: &AppContext, jwt_extractor: &Arc<JwtContextExtractor>) -> Router {
    let ctx_messages = ctx.clone();
    let ctx_responses = ctx.clone();
    let jwt_messages = Arc::clone(jwt_extractor);
    let jwt_responses = Arc::clone(jwt_extractor);
    let anthropic_inbound: Arc<dyn InboundAdapter> = Arc::new(AnthropicMessagesInbound);
    let responses_inbound: Arc<dyn InboundAdapter> = Arc::new(OpenAiResponsesInbound);

    Router::new()
        .route(
            "/messages",
            post(move |request| {
                let extractor = Arc::clone(&jwt_messages);
                let context = ctx_messages.clone();
                let inbound = Arc::clone(&anthropic_inbound);
                async move { messages::handle(inbound, extractor, context, request).await }
            }),
        )
        .route(
            "/responses",
            post(move |request| {
                let extractor = Arc::clone(&jwt_responses);
                let context = ctx_responses.clone();
                let inbound = Arc::clone(&responses_inbound);
                async move { messages::handle(inbound, extractor, context, request).await }
            }),
        )
}

fn bridge_auth_routes(ctx: &AppContext, jwt_extractor: &Arc<JwtContextExtractor>) -> Router {
    let ctx_pat = ctx.clone();
    let ctx_session = ctx.clone();
    let ctx_session_pat = ctx.clone();
    let ctx_mtls = ctx.clone();
    let ctx_oauth_client = ctx.clone();
    let jwt_oauth_client = Arc::clone(jwt_extractor);

    Router::new()
        .route(
            "/auth/bridge/pat",
            post(move |request| {
                let context = ctx_pat.clone();
                async move { auth::pat(context, request).await }
            }),
        )
        .route(
            "/auth/bridge/session",
            post(move |headers, body| {
                let context = ctx_session.clone();
                async move { auth::session(context, headers, body).await }
            }),
        )
        .route(
            "/auth/bridge/session-pat",
            post(move |body| {
                let context = ctx_session_pat.clone();
                async move { auth::session_pat(context, body).await }
            }),
        )
        .route(
            "/auth/bridge/mtls",
            post(move |headers, body| {
                let context = ctx_mtls.clone();
                async move { auth::mtls(context, headers, body).await }
            }),
        )
        .route(
            "/auth/bridge/oauth-client",
            post(move |request| {
                let extractor = Arc::clone(&jwt_oauth_client);
                let context = ctx_oauth_client.clone();
                async move { auth::provision_oauth_client(extractor, context, request).await }
            }),
        )
        .route("/auth/bridge/capabilities", get(auth::capabilities))
}

fn bridge_profile_routes(ctx: &AppContext, jwt_extractor: &Arc<JwtContextExtractor>) -> Router {
    let ctx_whoami = ctx.clone();
    let ctx_manifest = ctx.clone();
    let ctx_enabled_hosts = ctx.clone();
    let ctx_host_model_filter = ctx.clone();
    let ctx_profile_usage = ctx.clone();
    let ctx_heartbeat = ctx.clone();
    let jwt_whoami = Arc::clone(jwt_extractor);
    let jwt_manifest = Arc::clone(jwt_extractor);
    let jwt_enabled_hosts = Arc::clone(jwt_extractor);
    let jwt_host_model_filter = Arc::clone(jwt_extractor);
    let jwt_profile_usage = Arc::clone(jwt_extractor);
    let jwt_heartbeat = Arc::clone(jwt_extractor);

    Router::new()
        .route("/bridge/pubkey", get(bridge::pubkey))
        .route("/bridge/profile", get(bridge::profile))
        .route(
            "/bridge/whoami",
            get(move |headers| {
                let extractor = Arc::clone(&jwt_whoami);
                let context = ctx_whoami.clone();
                async move { bridge_whoami::handle(extractor, context, headers).await }
            }),
        )
        .route(
            "/bridge/manifest",
            get(move |headers| {
                let extractor = Arc::clone(&jwt_manifest);
                let context = ctx_manifest.clone();
                async move { bridge_manifest::manifest(extractor, context, headers).await }
            }),
        )
        .route(
            "/bridge/profile/enabled_hosts",
            post(move |headers, body| {
                let extractor = Arc::clone(&jwt_enabled_hosts);
                let context = ctx_enabled_hosts.clone();
                async move { bridge::set_enabled_host(extractor, context, headers, body).await }
            }),
        )
        .route(
            "/bridge/profile/host-model-filter",
            post(move |headers, body| {
                let extractor = Arc::clone(&jwt_host_model_filter);
                let context = ctx_host_model_filter.clone();
                async move {
                    bridge::set_host_model_filter(extractor, context, headers, body).await
                }
            }),
        )
        .route(
            "/bridge/profile/usage",
            get(move |headers| {
                let extractor = Arc::clone(&jwt_profile_usage);
                let context = ctx_profile_usage.clone();
                async move { bridge_profile_usage::handle(extractor, context, headers).await }
            }),
        )
        .route(
            "/bridge/heartbeat",
            post(move |headers, body| {
                let extractor = Arc::clone(&jwt_heartbeat);
                let context = ctx_heartbeat.clone();
                async move { bridge_heartbeat::handle(extractor, context, headers, body).await }
            }),
        )
}

pub fn gateway_router(ctx: &AppContext) -> Option<Router> {
    let jwt_extractor = build_jwt_extractor(ctx)?;

    Some(
        Router::new()
            .merge(inference_routes(ctx, &jwt_extractor))
            .merge(bridge_auth_routes(ctx, &jwt_extractor))
            .merge(bridge_profile_routes(ctx, &jwt_extractor))
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
