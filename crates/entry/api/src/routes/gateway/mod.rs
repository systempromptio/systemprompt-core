pub mod auth;
pub mod cowork;
pub mod messages;
pub mod models;

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Router};
use std::sync::Arc;
use std::time::Instant;
use systemprompt_models::SecretsBootstrap;
use systemprompt_runtime::AppContext;

use crate::services::middleware::JwtContextExtractor;

async fn log_gateway_request(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let started = Instant::now();
    let resp = next.run(req).await;
    let status = resp.status().as_u16();
    let elapsed_ms = started.elapsed().as_millis() as u64;
    tracing::info!(
        target: "systemprompt_api::gateway",
        method = %method,
        path = %path,
        status = status,
        elapsed_ms = elapsed_ms,
        "gateway request"
    );
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
            "/auth/cowork/pat",
            post(move |request| {
                let context = ctx_pat.clone();
                async move { auth::pat(context, request).await }
            }),
        )
        .route(
            "/auth/cowork/session",
            post(move |request| {
                let context = ctx_session.clone();
                async move { auth::session(context, request).await }
            }),
        )
        .route(
            "/auth/cowork/mtls",
            post(move |request| {
                let context = ctx_mtls.clone();
                async move { auth::mtls(context, request).await }
            }),
        )
        .route("/auth/cowork/capabilities", get(auth::capabilities))
        .route("/cowork/pubkey", get(cowork::pubkey))
        .route("/cowork/profile", get(cowork::profile))
        .route("/models", get(models::list))
        .route("/", get(models::root))
        .layer(Extension(ctx.clone()))
        .layer(axum::middleware::from_fn(log_gateway_request));

    Some(router)
}
