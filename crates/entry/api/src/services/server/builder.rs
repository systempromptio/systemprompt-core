//! Full API router construction and the global middleware stack.
//!
//! [`setup_api_server`] composes the route tree and applies the global layers
//! (body limit, analytics, context, session, CORS, trailing-slash, trace and
//! served-by headers, content negotiation, security headers) in the order they
//! must run. Binding and serving live in [`super::startup`], which binds the
//! listener before this router exists and swaps it in once bootstrap
//! completes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{StartupEventExt, StartupEventSender};

use super::routes::configure_routes;
use crate::services::middleware::{
    AnalyticsMiddleware, CorsMiddleware, PublicContextMiddleware, SessionMiddleware,
    inject_security_headers, inject_served_by, inject_trace_header, remove_trailing_slash,
};

pub use super::discovery::*;
pub use super::health::handle_health;

pub fn setup_api_server(ctx: &AppContext, events: Option<&StartupEventSender>) -> Result<Router> {
    let rate_config = &ctx.config().rate_limits;

    if rate_config.disabled
        && let Some(tx) = events
    {
        tx.warning("Rate limiting disabled - development mode only");
    }

    let router = configure_routes(ctx, events)?;
    apply_global_middleware(router, ctx)
}

fn apply_global_middleware(router: Router, ctx: &AppContext) -> Result<Router> {
    let mut router = router;

    router = router.layer(DefaultBodyLimit::max(2 * 1024 * 1024));

    let analytics_middleware = AnalyticsMiddleware::new(ctx)?;
    router = router.layer(axum::middleware::from_fn({
        let middleware = analytics_middleware;
        move |req, next| {
            let middleware = middleware.clone();
            async move { middleware.track_request(req, next).await }
        }
    }));

    let global_context_middleware = PublicContextMiddleware::new();
    router = router.layer(axum::middleware::from_fn({
        let middleware = global_context_middleware;
        move |req, next| async move { middleware.handle(req, next).await }
    }));

    let session_middleware = SessionMiddleware::new(ctx)?;
    router = router.layer(axum::middleware::from_fn({
        let middleware = session_middleware;
        move |req, next| {
            let middleware = middleware.clone();
            async move { middleware.handle(req, next).await }
        }
    }));

    let cors = CorsMiddleware::build_layer(ctx.config())?;
    router = router.layer(cors);

    router = router.layer(axum::middleware::from_fn(remove_trailing_slash));

    router = router.layer(axum::middleware::from_fn(inject_trace_header));

    router = router.layer(axum::middleware::from_fn(inject_served_by));

    if ctx.config().content_negotiation.enabled {
        router = router.layer(axum::middleware::from_fn(
            crate::services::middleware::content_negotiation_middleware,
        ));
    }

    if ctx.config().security_headers.enabled {
        let security_config = ctx.config().security_headers.clone();
        router = router.layer(axum::middleware::from_fn(move |req, next| {
            let config = security_config.clone();
            inject_security_headers(config, req, next)
        }));
    }

    Ok(router)
}
