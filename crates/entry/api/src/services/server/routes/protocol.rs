//! Protocol-surface route mounting.
//!
//! Mounts the OAuth, agent (A2A), MCP, stream, and content/admin/marketplace
//! route groups onto the server router, each nested under its `ApiPaths` base
//! with the correct context middleware, rate limit, and `AuthzPolicy` gate.

use axum::Router;
use std::sync::Arc;
use systemprompt_extension::LoaderError;
use systemprompt_models::modules::ApiPaths;
use systemprompt_oauth::OAuthState;
use systemprompt_runtime::AppContext;
use systemprompt_traits::AppContext as AppContextTrait;

use systemprompt_models::auth::UserType;

use crate::services::middleware::authz::AuthzPolicy;
use crate::services::middleware::{
    A2AContextMiddleware, McpContextMiddleware, PublicContextMiddleware, RouterExt,
    UserOnlyContextMiddleware,
};

fn create_oauth_state(ctx: &AppContext) -> Option<OAuthState> {
    let analytics = ctx.analytics_provider()?;
    let users = ctx.user_provider()?;
    let mcp_registry: Arc<dyn systemprompt_traits::McpRegistryProvider> =
        Arc::new(ctx.mcp_registry().clone());
    let state = OAuthState::new(Arc::clone(ctx.db_pool()), analytics, users)
        .with_mcp_registry(mcp_registry);
    Some(state)
}

pub(super) fn mount_oauth(
    mut router: Router,
    ctx: &AppContext,
    public_middleware: &PublicContextMiddleware,
    user_middleware: &UserOnlyContextMiddleware,
) -> Router {
    let rate_config = &ctx.config().rate_limits;
    if let Some(oauth_state) = create_oauth_state(ctx) {
        let oauth = crate::routes::oauth::public_router()
            .with_state(oauth_state.clone())
            .with_rate_limit(rate_config, rate_config.oauth_public_per_second)
            .with_auth(*public_middleware, AuthzPolicy::public())
            .merge(
                crate::routes::oauth::authenticated_router()
                    .with_state(oauth_state)
                    .with_rate_limit(rate_config, rate_config.oauth_auth_per_second)
                    .with_auth(user_middleware.clone(), AuthzPolicy::user()),
            );
        router = router.nest(ApiPaths::OAUTH_BASE, oauth);
    }
    router
}

pub(super) fn mount_agent(
    mut router: Router,
    ctx: &AppContext,
    public_middleware: &PublicContextMiddleware,
    user_middleware: &UserOnlyContextMiddleware,
    a2a_middleware: A2AContextMiddleware,
) -> Router {
    let rate_config = &ctx.config().rate_limits;

    router = router.nest(
        ApiPaths::CORE_CONTEXTS,
        crate::routes::agent::contexts_router()
            .with_state(ctx.clone())
            .with_rate_limit(rate_config, rate_config.contexts_per_second)
            .with_auth(user_middleware.clone(), AuthzPolicy::user()),
    );

    router = router.nest(
        ApiPaths::WEBHOOK,
        crate::routes::agent::webhook_router()
            .with_state(ctx.clone())
            .with_auth(user_middleware.clone(), AuthzPolicy::authenticated()),
    );

    router = router.nest(
        ApiPaths::CORE_TASKS,
        crate::routes::agent::tasks_router()
            .with_state(ctx.clone())
            .with_rate_limit(rate_config, rate_config.tasks_per_second)
            .with_auth(user_middleware.clone(), AuthzPolicy::user()),
    );

    router = router.nest(
        ApiPaths::CORE_ARTIFACTS,
        crate::routes::agent::artifacts_router()
            .with_state(ctx.clone())
            .with_rate_limit(rate_config, rate_config.artifacts_per_second)
            .with_auth(user_middleware.clone(), AuthzPolicy::user()),
    );

    router = router.nest(
        ApiPaths::AGENTS_REGISTRY,
        crate::routes::agent::registry_router(ctx)
            .with_rate_limit(rate_config, rate_config.agent_registry_per_second)
            .with_auth(*public_middleware, AuthzPolicy::public()),
    );

    router = router.nest(
        ApiPaths::AGENTS_BASE,
        crate::routes::proxy::agents::router(ctx)
            .with_rate_limit(rate_config, rate_config.agents_per_second)
            .with_auth(a2a_middleware, AuthzPolicy::authenticated()),
    );

    router
}

/// Mount the chat-platform inbound surfaces (Slack, Teams). Both authenticate
/// per-request at the handler (Slack signature / Teams activity token), so no
/// JWT middleware is applied — only a rate limit mirroring the agent surface
/// they ultimately dispatch into.
pub(super) fn mount_messaging(mut router: Router, ctx: &AppContext) -> Router {
    let rate_config = &ctx.config().rate_limits;

    router = router.nest(
        ApiPaths::SLACK_BASE,
        crate::routes::slack::slack_router()
            .with_state(ctx.clone())
            .with_rate_limit(rate_config, rate_config.agents_per_second),
    );

    router = router.nest(
        ApiPaths::TEAMS_BASE,
        crate::routes::teams::teams_router()
            .with_state(ctx.clone())
            .with_rate_limit(rate_config, rate_config.agents_per_second),
    );

    router
}

pub(super) fn mount_mcp_and_stream(
    mut router: Router,
    ctx: &AppContext,
    public_middleware: &PublicContextMiddleware,
    user_middleware: &UserOnlyContextMiddleware,
    mcp_middleware: McpContextMiddleware,
) -> Result<Router, LoaderError> {
    let rate_config = &ctx.config().rate_limits;

    router = router.nest(
        ApiPaths::MCP_REGISTRY,
        crate::routes::mcp::registry_router(ctx)
            .with_rate_limit(rate_config, rate_config.mcp_registry_per_second)
            .with_auth(*public_middleware, AuthzPolicy::public()),
    );

    // Why: MCP routes admit Anon at the route gate so the proxy handler
    // (services/proxy/auth.rs) can emit an RFC 9728-compliant
    // `WWW-Authenticate: Bearer resource_metadata="…"` challenge with the
    // path-scoped resource URL. A coarser `restricted_to([User, Admin, Mcp,
    // Service])` gate here collapses the response to a generic 403 and breaks
    // spec-compliant MCP clients (Cowork, Claude Code, etc.), which only
    // start their OAuth discovery handshake on a 401 carrying the challenge.
    // The proxy is the single auth boundary for `/api/v1/mcp/*`.
    router = router.nest(
        ApiPaths::MCP_BASE,
        crate::routes::proxy::mcp::router(ctx)
            .with_rate_limit(rate_config, rate_config.mcp_per_second)
            .with_auth(mcp_middleware, AuthzPolicy::public()),
    );

    router = router.nest(
        ApiPaths::STREAM_BASE,
        crate::routes::stream::stream_router(ctx)
            .map_err(|e| LoaderError::InitializationFailed {
                extension: "stream".to_owned(),
                message: e.to_string(),
            })?
            .with_rate_limit(rate_config, rate_config.stream_per_second)
            .with_auth(user_middleware.clone(), AuthzPolicy::user()),
    );

    Ok(router)
}

pub(super) fn mount_content_and_misc(
    mut router: Router,
    ctx: &AppContext,
    public_middleware: &PublicContextMiddleware,
    user_middleware: &UserOnlyContextMiddleware,
) -> Result<Router, LoaderError> {
    let rate_config = &ctx.config().rate_limits;

    let content = crate::routes::content::public_router(ctx)
        .with_rate_limit(rate_config, rate_config.content_per_second)
        .with_auth(*public_middleware, AuthzPolicy::public())
        .merge(
            crate::routes::content::authenticated_router(ctx)
                .with_rate_limit(rate_config, rate_config.content_per_second)
                .with_auth(user_middleware.clone(), AuthzPolicy::user()),
        );
    router = router.nest(ApiPaths::CONTENT_BASE, content);

    router = router.merge(
        crate::routes::content::redirect_router(ctx.db_pool())
            .with_rate_limit(rate_config, rate_config.content_per_second)
            .with_auth(*public_middleware, AuthzPolicy::public()),
    );

    router = router.nest(
        ApiPaths::SYNC_BASE,
        crate::routes::sync::router()
            .with_state(ctx.clone())
            .with_auth(
                user_middleware.clone(),
                AuthzPolicy::restricted_to(&[UserType::Service]),
            ),
    );

    router = router.nest(
        ApiPaths::MARKETPLACE_BASE,
        crate::routes::marketplace::router()
            .with_state(ctx.clone())
            .with_auth(*public_middleware, AuthzPolicy::public()),
    );

    router = router.nest(
        ApiPaths::ANALYTICS_BASE,
        crate::routes::analytics::router(ctx)
            .map_err(|e| LoaderError::InitializationFailed {
                extension: "analytics".to_owned(),
                message: e.to_string(),
            })?
            .with_rate_limit(rate_config, rate_config.content_per_second)
            .with_auth(user_middleware.clone(), AuthzPolicy::admin()),
    );

    router = router.nest(
        ApiPaths::TRACK_ENGAGEMENT,
        crate::routes::engagement::router(ctx)
            .map_err(|e| LoaderError::InitializationFailed {
                extension: "engagement".to_owned(),
                message: e.to_string(),
            })?
            .with_rate_limit(rate_config, rate_config.content_per_second)
            .with_auth(*public_middleware, AuthzPolicy::public()),
    );

    router = router.nest(
        ApiPaths::CORE_USERS,
        crate::routes::users::router(ctx)
            .with_rate_limit(rate_config, rate_config.oauth_auth_per_second)
            .with_auth(user_middleware.clone(), AuthzPolicy::user()),
    );

    router = router.nest(
        ApiPaths::ADMIN_BASE,
        crate::routes::admin::router()
            .with_state(ctx.clone())
            .with_rate_limit(rate_config, 10)
            .with_auth(user_middleware.clone(), AuthzPolicy::admin()),
    );

    if let Some(gateway) = crate::routes::gateway::gateway_router(ctx) {
        router = router.nest(ApiPaths::GATEWAY_BASE, gateway);
    }

    Ok(router)
}
