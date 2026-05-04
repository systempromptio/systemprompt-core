mod extension_mount;
mod protocol;
mod static_setup;

use axum::Router;
use std::sync::Arc;
use systemprompt_extension::LoaderError;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{AppContext as AppContextTrait, StartupEventSender};
use systemprompt_users::BannedIpRepository;

use crate::services::middleware::{
    ContextMiddleware, JwtContextExtractor, RouterExt, ip_ban_middleware,
};

pub fn configure_routes(
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
) -> Result<Router, LoaderError> {
    let mut router = Router::new();

    let jwt_extractor = build_jwt_extractor(ctx)?;

    let public_middleware = ContextMiddleware::public(jwt_extractor.clone());
    let user_middleware = ContextMiddleware::user_only(jwt_extractor.clone());
    let full_middleware = ContextMiddleware::full(jwt_extractor.clone());
    let mcp_middleware = ContextMiddleware::mcp(jwt_extractor);

    router = protocol::mount_oauth(router, ctx, &public_middleware, &user_middleware);
    router = protocol::mount_agent(
        router,
        ctx,
        &public_middleware,
        &user_middleware,
        full_middleware,
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

    router = router.merge(discovery_router(ctx).with_auth_middleware(public_middleware.clone()));
    router =
        router.merge(authenticated_discovery_router(ctx).with_auth_middleware(user_middleware));
    router = router.merge(wellknown_router(ctx).with_auth_middleware(public_middleware.clone()));

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
            extension: "ip_ban_middleware".to_string(),
            message: e.to_string(),
        }
    })?);

    Ok(router.layer(axum::middleware::from_fn(move |req, next| {
        let repo = Arc::clone(&banned_ip_repo);
        async move { ip_ban_middleware(req, next, repo).await }
    })))
}

fn build_jwt_extractor(ctx: &AppContext) -> Result<JwtContextExtractor, LoaderError> {
    let extractor = JwtContextExtractor::new(
        systemprompt_config::SecretsBootstrap::jwt_secret().map_err(|e| {
            LoaderError::InitializationFailed {
                extension: "jwt".to_string(),
                message: e.to_string(),
            }
        })?,
        ctx.db_pool(),
    );
    Ok(match ctx.analytics_provider() {
        Some(analytics) => extractor.with_analytics_provider(analytics),
        None => extractor,
    })
}

fn discovery_router(ctx: &AppContext) -> Router {
    super::builder::discovery_router(ctx)
}

fn authenticated_discovery_router(ctx: &AppContext) -> Router {
    super::builder::authenticated_discovery_router(ctx)
}

fn wellknown_router(ctx: &AppContext) -> Router {
    crate::routes::oauth::wellknown_routes().merge(crate::routes::wellknown_router(ctx))
}
