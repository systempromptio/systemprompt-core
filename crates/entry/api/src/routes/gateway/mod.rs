pub mod auth;
pub mod bridge;
pub mod bridge_heartbeat;
pub mod messages;
pub mod models;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Router};
use std::sync::Arc;
use std::time::Instant;
use systemprompt_config::SecretsBootstrap;
use systemprompt_database::DbPool;
use systemprompt_logging::{LogEntry, LogLevel, LoggingRepository};
use systemprompt_runtime::AppContext;

use crate::services::middleware::JwtContextExtractor;

async fn log_gateway_request(State(pool): State<DbPool>, req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
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

    if status >= 500 {
        tracing::error!(method = %method, path = %path, status, elapsed_ms, "gateway request failed");
    } else if status >= 400 {
        tracing::warn!(method = %method, path = %path, status, elapsed_ms, "gateway request rejected");
    } else {
        tracing::info!(method = %method, path = %path, status, elapsed_ms, "gateway request");
    }

    if let Ok(repo) = LoggingRepository::new(&pool) {
        let level = if status >= 500 {
            LogLevel::Error
        } else if status >= 400 {
            LogLevel::Warn
        } else {
            LogLevel::Info
        };
        let entry = LogEntry::new(
            level,
            "systemprompt_api::gateway",
            format!("{method} {path} -> {status} ({elapsed_ms}ms)"),
        )
        .with_metadata(metadata);
        if let Err(e) = repo
            .with_database(true)
            .with_terminal(false)
            .log(entry)
            .await
        {
            tracing::warn!(error = %e, "gateway access log persist failed");
        }
    }

    resp
}

pub fn gateway_router(ctx: &AppContext) -> Option<Router> {
    let jwt_secret = match SecretsBootstrap::jwt_secret() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "Gateway router: JWT secret unavailable — gateway disabled");
            return None;
        },
    };

    let jwt_extractor = Arc::new(JwtContextExtractor::new(jwt_secret, ctx.db_pool()));

    let ctx_messages = ctx.clone();
    let ctx_pat = ctx.clone();
    let ctx_session = ctx.clone();
    let ctx_mtls = ctx.clone();
    let ctx_heartbeat = ctx.clone();
    let ctx_manifest = ctx.clone();
    let ctx_oauth_client = ctx.clone();
    let jwt_heartbeat = Arc::clone(&jwt_extractor);
    let jwt_manifest = Arc::clone(&jwt_extractor);
    let jwt_oauth_client = Arc::clone(&jwt_extractor);

    let router = Router::new()
        .route(
            "/messages",
            post(move |request| {
                let extractor = Arc::clone(&jwt_extractor);
                let context = ctx_messages.clone();
                async move { messages::handle(extractor, context, request).await }
            }),
        )
        .route(
            "/auth/bridge/pat",
            post(move |request| {
                let context = ctx_pat.clone();
                async move { auth::pat(context, request).await }
            }),
        )
        .route(
            "/auth/bridge/session",
            post(move |request| {
                let context = ctx_session.clone();
                async move { auth::session(context, request).await }
            }),
        )
        .route(
            "/auth/bridge/mtls",
            post(move |request| {
                let context = ctx_mtls.clone();
                async move { auth::mtls(context, request).await }
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
        .route("/bridge/pubkey", get(bridge::pubkey))
        .route("/bridge/profile", get(bridge::profile))
        .route(
            "/bridge/manifest",
            get(move |headers| {
                let extractor = Arc::clone(&jwt_manifest);
                let context = ctx_manifest.clone();
                async move { bridge::manifest(extractor, context, headers).await }
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
        .route("/models", get(models::list))
        .route("/", get(models::root))
        .layer(Extension(ctx.clone()))
        .layer(axum::middleware::from_fn_with_state(
            Arc::clone(ctx.db_pool()),
            log_gateway_request,
        ));

    Some(router)
}
