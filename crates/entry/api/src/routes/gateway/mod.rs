pub mod auth;
pub mod cowork;
pub mod messages;

use axum::{Extension, Router};
use axum::routing::{get, post};
use std::sync::Arc;
use systemprompt_models::SecretsBootstrap;
use systemprompt_runtime::AppContext;

use crate::services::middleware::JwtContextExtractor;

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
        .route("/cowork/manifest", get(cowork::manifest))
        .layer(Extension(ctx.clone()));

    Some(router)
}
