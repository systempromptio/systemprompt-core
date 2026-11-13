//! Protocol-surface route mounting.
//!
//! Mounts the agent (A2A), MCP, stream, and content/admin/marketplace route
//! groups onto the server router. Each group is an [`ExtensionRouter`]
//! declaration — base path, `AuthzPolicy`, context kind, rate limit, body
//! limit — mounted through the one declaration mount in
//! `services::middleware::mount`. [`MountCtx`] carries the enforcement
//! primitives every route mount in this module tree shares.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use std::sync::Arc;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, ExtensionRouter, RateBudget, RateLimit, RouteGroup, RouteProtection,
};
use systemprompt_gateway::GatewayEngines;
use systemprompt_models::auth::UserType;
use systemprompt_models::modules::ApiPaths;
use systemprompt_runtime::AppContext;

use crate::services::middleware::{MountError, MountLayers, mount, place, protect};

pub(super) struct MountCtx<'a> {
    pub ctx: &'a AppContext,
    pub engines: &'a Arc<GatewayEngines>,
    pub layers: &'a MountLayers,
    pub serves: &'a dyn Fn(RouteGroup) -> bool,
}

impl MountCtx<'_> {
    pub(super) fn mount(
        &self,
        tree: Router,
        declared: ExtensionRouter,
    ) -> Result<Router, MountError> {
        mount(tree, declared, self.layers, self.ctx, self.serves)
    }

    pub(super) fn protect(
        &self,
        router: Router,
        protection: &RouteProtection,
        base_path: &'static str,
    ) -> Result<Router, MountError> {
        protect(router, protection, self.layers, base_path)
    }
}

pub(super) fn declare(
    router: Router,
    base_path: &'static str,
    policy: AuthzPolicy,
    context: ContextKind,
    group: RouteGroup,
) -> ExtensionRouter {
    ExtensionRouter::new(router, base_path, policy, context).in_group(group)
}

pub(super) fn mount_agent(mut router: Router, mount: &MountCtx<'_>) -> Result<Router, MountError> {
    let ctx = mount.ctx;
    let group = RouteGroup::Agent;

    router = mount.mount(
        router,
        declare(
            crate::routes::agent::contexts_router().with_state(ctx.clone()),
            ApiPaths::CORE_CONTEXTS,
            AuthzPolicy::user(),
            ContextKind::User,
            group,
        )
        .with_rate_limit(RateLimit::new("contexts", RateBudget::Contexts)),
    )?;

    router = mount.mount(
        router,
        declare(
            crate::routes::agent::webhook_router().with_state(ctx.clone()),
            ApiPaths::WEBHOOK,
            AuthzPolicy::authenticated(),
            ContextKind::User,
            group,
        ),
    )?;

    router = mount.mount(
        router,
        declare(
            crate::routes::agent::tasks_router().with_state(ctx.clone()),
            ApiPaths::CORE_TASKS,
            AuthzPolicy::user(),
            ContextKind::User,
            group,
        )
        .with_rate_limit(RateLimit::new("tasks", RateBudget::Tasks)),
    )?;

    router = mount.mount(
        router,
        declare(
            crate::routes::agent::artifacts_router().with_state(ctx.clone()),
            ApiPaths::CORE_ARTIFACTS,
            AuthzPolicy::user(),
            ContextKind::User,
            group,
        )
        .with_rate_limit(RateLimit::new("artifacts", RateBudget::Artifacts)),
    )?;

    router = mount.mount(
        router,
        declare(
            crate::routes::agent::registry_router(ctx),
            ApiPaths::AGENTS_REGISTRY,
            AuthzPolicy::public(),
            ContextKind::Public,
            group,
        )
        .with_rate_limit(RateLimit::new("agent_registry", RateBudget::AgentRegistry)),
    )?;

    mount.mount(
        router,
        declare(
            crate::routes::proxy::agents::router(ctx),
            ApiPaths::AGENTS_BASE,
            AuthzPolicy::authenticated(),
            ContextKind::A2a,
            group,
        )
        .with_body_limit(systemprompt_models::net::AGENT_REQUEST_BODY_LIMIT_BYTES)
        .with_rate_limit(RateLimit::new("agents", RateBudget::Agents)),
    )
}

#[cfg(any(feature = "slack", feature = "teams"))]
pub(super) fn mount_messaging(
    mut router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, super::RouteMountError> {
    use super::RouteMountError;
    use crate::services::middleware::RouterExt;

    let (ctx, limits) = (mount.ctx, &mount.layers.limits);
    let rate_config = &ctx.config().rate_limits;

    let messaging = crate::routes::messaging::MessagingState::guarded()
        .map_err(|source| RouteMountError::initialization("messaging_reply_client", source))?;

    #[cfg(feature = "teams")]
    {
        router = router.nest(
            ApiPaths::TEAMS_BASE,
            crate::routes::teams::teams_router(ctx, &messaging)
                .map_err(|source| RouteMountError::initialization("teams", source))?
                .with_rate_limit(limits, rate_config.agents_per_second, "agents")?,
        );
    }

    #[cfg(feature = "slack")]
    {
        router = router.nest(
            ApiPaths::SLACK_BASE,
            crate::routes::slack::slack_router(messaging)
                .with_state(ctx.clone())
                .with_rate_limit(limits, rate_config.agents_per_second, "agents")?,
        );
    }

    Ok(router)
}

pub(super) fn mount_mcp_and_stream(
    mut router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, MountError> {
    let ctx = mount.ctx;
    let group = RouteGroup::McpAndStream;

    router = mount.mount(
        router,
        declare(
            crate::routes::mcp::registry_router(ctx),
            ApiPaths::MCP_REGISTRY,
            AuthzPolicy::public(),
            ContextKind::Public,
            group,
        )
        .with_rate_limit(RateLimit::new("mcp_registry", RateBudget::McpRegistry)),
    )?;

    let mcp_rate = RateLimit::new("mcp", RateBudget::Mcp);
    let executions = mount.protect(
        crate::routes::proxy::mcp::executions_router(ctx),
        &RouteProtection::new(AuthzPolicy::authenticated(), ContextKind::User)
            .with_rate_limit(mcp_rate),
        ApiPaths::MCP_BASE,
    )?;
    let proxy = mount.protect(
        crate::routes::proxy::mcp::router(ctx),
        &RouteProtection::new(AuthzPolicy::deferred_to_handler(), ContextKind::Mcp)
            .with_rate_limit(mcp_rate),
        ApiPaths::MCP_BASE,
    )?;
    router = place(router, ApiPaths::MCP_BASE, executions.merge(proxy));

    mount.mount(
        router,
        declare(
            crate::routes::stream::stream_router(ctx),
            ApiPaths::STREAM_BASE,
            AuthzPolicy::user(),
            ContextKind::User,
            group,
        )
        .with_rate_limit(RateLimit::new("stream", RateBudget::Stream)),
    )
}

pub(super) fn mount_content_and_misc(
    mut router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, MountError> {
    let ctx = mount.ctx;
    let group = RouteGroup::ContentAndMisc;
    let content_rate = RateLimit::new("content", RateBudget::Content);

    let public_content = mount.protect(
        crate::routes::content::public_router(ctx),
        &RouteProtection::new(AuthzPolicy::public(), ContextKind::Public)
            .with_rate_limit(content_rate),
        ApiPaths::CONTENT_BASE,
    )?;
    let authenticated_content = mount.protect(
        crate::routes::content::authenticated_router(ctx),
        &RouteProtection::new(AuthzPolicy::user(), ContextKind::User).with_rate_limit(content_rate),
        ApiPaths::CONTENT_BASE,
    )?;
    router = place(
        router,
        ApiPaths::CONTENT_BASE,
        public_content.merge(authenticated_content),
    );

    router = mount.mount(
        router,
        declare(
            crate::routes::content::redirect_router(ctx.content_repositories()),
            systemprompt_extension::registry::WEB_ROOT_BASE_PATH,
            AuthzPolicy::public(),
            ContextKind::Public,
            group,
        )
        .with_rate_limit(content_rate),
    )?;

    router = mount.mount(
        router,
        declare(
            crate::routes::sync::router().with_state(ctx.clone()),
            ApiPaths::SYNC_BASE,
            AuthzPolicy::restricted_to(&[UserType::Service]),
            ContextKind::User,
            group,
        ),
    )?;

    router = mount.mount(
        router,
        declare(
            crate::routes::marketplace::router().with_state(ctx.clone()),
            ApiPaths::MARKETPLACE_BASE,
            AuthzPolicy::public(),
            ContextKind::Public,
            group,
        ),
    )?;

    router = mount.mount(
        router,
        declare(
            crate::routes::analytics::router(ctx),
            ApiPaths::ANALYTICS_BASE,
            AuthzPolicy::admin(),
            ContextKind::User,
            group,
        )
        .with_rate_limit(content_rate),
    )?;

    router = mount.mount(
        router,
        declare(
            crate::routes::engagement::router(ctx),
            ApiPaths::TRACK_ENGAGEMENT,
            AuthzPolicy::public(),
            ContextKind::Public,
            group,
        )
        .with_rate_limit(content_rate),
    )?;

    router = mount.mount(
        router,
        declare(
            crate::routes::users::router(ctx),
            ApiPaths::CORE_USERS,
            AuthzPolicy::user(),
            ContextKind::User,
            group,
        )
        .with_rate_limit(RateLimit::new("oauth_auth", RateBudget::OauthAuth)),
    )?;

    mount.mount(
        router,
        declare(
            crate::routes::admin::router().with_state(ctx.clone()),
            ApiPaths::ADMIN_BASE,
            AuthzPolicy::admin(),
            ContextKind::User,
            group,
        )
        .with_rate_limit(RateLimit::new("admin", RateBudget::PerSecond(10))),
    )
}
