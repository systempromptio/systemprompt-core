//! Router assembly for the API server.
//!
//! [`configure_routes`] composes the full route tree: protocol surfaces (OAuth,
//! agent, MCP, stream, content), extension-mounted routes, discovery and
//! well-known endpoints, static content, and the global IP-ban and metrics
//! layers. Each surface is gated with its `AuthzPolicy` at mount time.

mod extension_mount;
mod protocol;
mod static_setup;

use axum::Router;
use std::sync::Arc;
use systemprompt_extension::LoaderError;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{AppContext as AppContextTrait, StartupEventSender};
use systemprompt_users::BannedIpRepository;

use crate::services::middleware::authz::AuthzPolicy;
use crate::services::middleware::client_addr::parse_trusted_proxies;
use crate::services::middleware::{
    A2AContextMiddleware, JtiRevocationChecker, JwtContextExtractor, McpContextMiddleware,
    PublicContextMiddleware, RouterExt, UserOnlyContextMiddleware, ip_ban_middleware,
};

pub(super) fn configure_routes(
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
) -> Result<Router, LoaderError> {
    let mut router = Router::new();

    let metrics_handle =
        super::metrics::install_recorder().map_err(|e| LoaderError::InitializationFailed {
            extension: "prometheus_metrics".to_owned(),
            message: e.to_string(),
        })?;

    let jwt_extractor = build_jwt_extractor(ctx)?;

    let public_middleware = PublicContextMiddleware::new();
    let user_middleware = UserOnlyContextMiddleware::new(jwt_extractor.clone());
    let a2a_middleware = A2AContextMiddleware::new(jwt_extractor.clone());
    let mcp_middleware = McpContextMiddleware::new(jwt_extractor);

    router = protocol::mount_oauth(router, ctx, &public_middleware, &user_middleware);
    router = protocol::mount_agent(
        router,
        ctx,
        &public_middleware,
        &user_middleware,
        a2a_middleware,
    );
    router = protocol::mount_mcp_and_stream(
        router,
        ctx,
        &public_middleware,
        &user_middleware,
        mcp_middleware,
    )?;
    router = protocol::mount_content_and_misc(router, ctx, &public_middleware, &user_middleware)?;

    router = extension_mount::mount_extension_routes(router, ctx, &user_middleware, events)?;

    router = router.merge(
        discovery_router(ctx, metrics_handle).with_auth(public_middleware, AuthzPolicy::public()),
    );
    router = router.merge(
        authenticated_discovery_router(ctx)
            .with_auth(user_middleware, AuthzPolicy::authenticated()),
    );
    router =
        router.merge(wellknown_router(ctx).with_auth(public_middleware, AuthzPolicy::public()));

    router = router.route(
        "/auth/link-passkey",
        axum::routing::get(crate::routes::oauth::webauthn::link::link_passkey_page),
    );

    router = router.merge(static_setup::build_static_router(
        ctx,
        public_middleware,
        events,
    ));

    let banned_ip_repo = Arc::new(BannedIpRepository::new(ctx.db_pool()).map_err(|e| {
        LoaderError::InitializationFailed {
            extension: "ip_ban_middleware".to_owned(),
            message: e.to_string(),
        }
    })?);
    let trusted_proxies = Arc::new(parse_trusted_proxies(&ctx.config().trusted_proxies));

    router = router.layer(axum::middleware::from_fn(move |req, next| {
        let repo = Arc::clone(&banned_ip_repo);
        let proxies = Arc::clone(&trusted_proxies);
        async move { ip_ban_middleware(req, next, repo, proxies).await }
    }));

    Ok(router.layer(axum::middleware::from_fn(super::metrics::track_metrics)))
}

fn build_jwt_extractor(ctx: &AppContext) -> Result<JwtContextExtractor, LoaderError> {
    let analytics = ctx
        .analytics_provider()
        .ok_or_else(|| LoaderError::InitializationFailed {
            extension: "jwt".to_owned(),
            message: "AnalyticsProvider is required for JWT session enforcement".to_owned(),
        })?;
    let user_provider = ctx
        .user_provider()
        .ok_or_else(|| LoaderError::InitializationFailed {
            extension: "jwt".to_owned(),
            message: "UserProvider is required for JWT validation".to_owned(),
        })?;
    let jti_revocation = JtiRevocationChecker::from_pool(ctx.db_pool()).map_err(|e| {
        LoaderError::InitializationFailed {
            extension: "jti_revocation".to_owned(),
            message: e.to_string(),
        }
    })?;
    Ok(JwtContextExtractor::new(
        analytics,
        user_provider,
        jti_revocation,
    ))
}

fn discovery_router(
    ctx: &AppContext,
    metrics_handle: metrics_exporter_prometheus::PrometheusHandle,
) -> Router {
    super::builder::discovery_router(ctx, metrics_handle)
}

fn authenticated_discovery_router(ctx: &AppContext) -> Router {
    super::builder::authenticated_discovery_router(ctx)
}

fn wellknown_router(ctx: &AppContext) -> Router {
    crate::routes::oauth::wellknown_routes(ctx).merge(crate::routes::wellknown_router(ctx))
}
