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
use systemprompt_models::modules::ApiPaths;
use systemprompt_runtime::AppContext;

use crate::services::middleware::{MountError, MountLayers, mount, place, protect};

mod content;

pub(super) use content::mount_content_and_misc;

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

pub(super) const fn declare(
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
    use systemprompt_extension::HandlerVerifier;

    let ctx = mount.ctx;
    let messaging_rate = RateLimit::new("agents", RateBudget::Agents);

    let messaging = crate::routes::messaging::MessagingState::guarded()
        .map_err(|source| RouteMountError::initialization("messaging_reply_client", source))?;

    #[cfg(feature = "teams")]
    {
        router = mount.mount(
            router,
            declare(
                crate::routes::teams::teams_router(ctx, &messaging)
                    .map_err(|source| RouteMountError::initialization("teams", source))?,
                ApiPaths::TEAMS_BASE,
                AuthzPolicy::handler_verified(HandlerVerifier::TeamsBotToken),
                ContextKind::HandlerResolved,
                RouteGroup::Messaging,
            )
            .with_rate_limit(messaging_rate),
        )?;
    }

    #[cfg(feature = "slack")]
    {
        router = mount.mount(
            router,
            declare(
                crate::routes::slack::slack_router(messaging).with_state(ctx.clone()),
                ApiPaths::SLACK_BASE,
                AuthzPolicy::handler_verified(HandlerVerifier::SlackSignature),
                ContextKind::HandlerResolved,
                RouteGroup::Messaging,
            )
            .with_rate_limit(messaging_rate),
        )?;
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
