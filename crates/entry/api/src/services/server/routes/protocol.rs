use axum::Router;
use std::sync::Arc;
use systemprompt_extension::LoaderError;
use systemprompt_models::modules::ApiPaths;
use systemprompt_oauth::OAuthState;
use systemprompt_runtime::AppContext;
use systemprompt_traits::AppContext as AppContextTrait;

use crate::services::middleware::{ContextMiddleware, JwtContextExtractor, RouterExt};

fn create_oauth_state(ctx: &AppContext) -> Option<OAuthState> {
    let analytics = ctx.analytics_provider()?;
    let users = ctx.user_provider()?;
    let state = OAuthState::new(Arc::clone(ctx.db_pool()), analytics, users);
    Some(state)
}

pub(super) fn mount_oauth(
    mut router: Router,
    ctx: &AppContext,
    public_middleware: &ContextMiddleware<JwtContextExtractor>,
    user_middleware: &ContextMiddleware<JwtContextExtractor>,
) -> Router {
    let rate_config = &ctx.config().rate_limits;
    if let Some(oauth_state) = create_oauth_state(ctx) {
        router = router.nest(
            ApiPaths::OAUTH_BASE,
            crate::routes::oauth::public_router()
                .with_state(oauth_state.clone())
                .with_rate_limit(rate_config, rate_config.oauth_public_per_second)
                .with_auth_middleware(public_middleware.clone()),
        );

        router = router.nest(
            ApiPaths::OAUTH_BASE,
            crate::routes::oauth::authenticated_router()
                .with_state(oauth_state)
                .with_rate_limit(rate_config, rate_config.oauth_auth_per_second)
                .with_auth_middleware(user_middleware.clone()),
        );
    }
    router
}

pub(super) fn mount_agent(
    mut router: Router,
    ctx: &AppContext,
    public_middleware: &ContextMiddleware<JwtContextExtractor>,
    user_middleware: &ContextMiddleware<JwtContextExtractor>,
    full_middleware: ContextMiddleware<JwtContextExtractor>,
) -> Router {
    let rate_config = &ctx.config().rate_limits;

    router = router.nest(
        ApiPaths::CORE_CONTEXTS,
        crate::routes::agent::contexts_router()
            .with_state(ctx.clone())
            .with_rate_limit(rate_config, rate_config.contexts_per_second)
            .with_auth_middleware(user_middleware.clone()),
    );

    router = router.nest(
        ApiPaths::WEBHOOK,
        crate::routes::agent::webhook_router()
            .with_state(ctx.clone())
            .with_auth_middleware(user_middleware.clone()),
    );

    router = router.nest(
        ApiPaths::CORE_TASKS,
        crate::routes::agent::tasks_router()
            .with_state(ctx.clone())
            .with_rate_limit(rate_config, rate_config.tasks_per_second)
            .with_auth_middleware(user_middleware.clone()),
    );

    router = router.nest(
        ApiPaths::CORE_ARTIFACTS,
        crate::routes::agent::artifacts_router()
            .with_state(ctx.clone())
            .with_rate_limit(rate_config, rate_config.artifacts_per_second)
            .with_auth_middleware(user_middleware.clone()),
    );

    router = router.nest(
        ApiPaths::AGENTS_REGISTRY,
        crate::routes::agent::registry_router(ctx)
            .with_rate_limit(rate_config, rate_config.agent_registry_per_second)
            .with_auth_middleware(public_middleware.clone()),
    );

    router = router.nest(
        ApiPaths::AGENTS_BASE,
        crate::routes::proxy::agents::router(ctx)
            .with_rate_limit(rate_config, rate_config.agents_per_second)
            .with_auth_middleware(full_middleware),
    );

    router
}

pub(super) fn mount_mcp_and_stream(
    mut router: Router,
    ctx: &AppContext,
    public_middleware: &ContextMiddleware<JwtContextExtractor>,
    user_middleware: &ContextMiddleware<JwtContextExtractor>,
    mcp_middleware: ContextMiddleware<JwtContextExtractor>,
) -> Result<Router, LoaderError> {
    let rate_config = &ctx.config().rate_limits;

    router = router.nest(
        ApiPaths::MCP_REGISTRY,
        crate::routes::mcp::registry_router()
            .with_rate_limit(rate_config, rate_config.mcp_registry_per_second)
            .with_auth_middleware(public_middleware.clone()),
    );

    router = router.nest(
        ApiPaths::MCP_BASE,
        crate::routes::proxy::mcp::router(ctx)
            .with_rate_limit(rate_config, rate_config.mcp_per_second)
            .with_auth_middleware(mcp_middleware),
    );

    router = router.nest(
        ApiPaths::STREAM_BASE,
        crate::routes::stream::stream_router(ctx)
            .map_err(|e| LoaderError::InitializationFailed {
                extension: "stream".to_string(),
                message: e.to_string(),
            })?
            .with_rate_limit(rate_config, rate_config.stream_per_second)
            .with_auth_middleware(user_middleware.clone()),
    );

    Ok(router)
}

pub(super) fn mount_content_and_misc(
    mut router: Router,
    ctx: &AppContext,
    public_middleware: &ContextMiddleware<JwtContextExtractor>,
    user_middleware: &ContextMiddleware<JwtContextExtractor>,
) -> Result<Router, LoaderError> {
    let rate_config = &ctx.config().rate_limits;

    router = router.nest(
        ApiPaths::CONTENT_BASE,
        crate::routes::content::router(ctx)
            .with_rate_limit(rate_config, rate_config.content_per_second)
            .with_auth_middleware(public_middleware.clone()),
    );

    router = router.merge(
        crate::routes::content::redirect_router(ctx.db_pool())
            .with_rate_limit(rate_config, rate_config.content_per_second)
            .with_auth_middleware(public_middleware.clone()),
    );

    router = router.nest(
        ApiPaths::SYNC_BASE,
        crate::routes::sync::router().with_state(ctx.clone()),
    );

    router = router.nest(
        ApiPaths::MARKETPLACE_BASE,
        crate::routes::marketplace::router()
            .with_state(ctx.clone())
            .with_auth_middleware(public_middleware.clone()),
    );

    router = router.nest(
        ApiPaths::ANALYTICS_BASE,
        crate::routes::analytics::router(ctx)
            .map_err(|e| LoaderError::InitializationFailed {
                extension: "analytics".to_string(),
                message: e.to_string(),
            })?
            .with_rate_limit(rate_config, rate_config.content_per_second)
            .with_auth_middleware(user_middleware.clone()),
    );

    router = router.nest(
        ApiPaths::TRACK_ENGAGEMENT,
        crate::routes::engagement::router(ctx)
            .map_err(|e| LoaderError::InitializationFailed {
                extension: "engagement".to_string(),
                message: e.to_string(),
            })?
            .with_rate_limit(rate_config, rate_config.content_per_second)
            .with_auth_middleware(public_middleware.clone()),
    );

    router = router.nest(
        ApiPaths::ADMIN_BASE,
        crate::routes::admin::router()
            .with_state(ctx.clone())
            .with_rate_limit(rate_config, 10)
            .with_auth_middleware(user_middleware.clone()),
    );

    if let Some(gateway) = crate::routes::gateway::gateway_router(ctx) {
        router = router.nest(ApiPaths::GATEWAY_BASE, gateway);
    }

    Ok(router)
}
