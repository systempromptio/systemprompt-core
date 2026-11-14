//! Router assembly for the API server.
//!
//! `configure_routes` composes the route tree for this node's role: protocol
//! surfaces (OAuth, agent, MCP, stream, content, gateway), extension-mounted
//! routes, discovery and well-known endpoints, static content, and the global
//! IP-ban and metrics layers. [`role::route_groups`] decides which groups a
//! role mounts. Each surface is a route declaration applied by
//! `services::middleware::mount`: its `AuthzPolicy`, context middleware, rate
//! limit, body limit and protection layers come from the declaration.
//!
//! The static router is merged after the IP-ban layer is applied, so public
//! pages and assets are served without a ban-list lookup; only the API routes
//! are gated. Metrics stay outermost and still measure static responses.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "agent-runs")]
mod agent_runs;
#[cfg(feature = "brain")]
mod brain;
mod documents;
mod error;
mod extension_mount;
mod feature_mount;
mod gateway;
mod issues;
#[cfg(feature = "bridge")]
mod managed;
#[cfg(feature = "notifications")]
mod notifications;
mod oauth_mount;
mod protocol;
pub mod role;
mod static_setup;

use axum::Router;
use std::sync::Arc;
use systemprompt_extension::registry::WEB_ROOT_BASE_PATH;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, ExtensionContext as _, LoaderError, RateBudget, RateLimit, RouteGroup,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::profile::NodeRole;

use crate::routes::admin::services::RefreshLock;
pub(super) use error::RouteMountError;
use protocol::{MountCtx, declare};
use systemprompt_gateway::GatewayEngines;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{DynIpBanStore, StartupEventSender};

use crate::services::middleware::{
    A2AContextMiddleware, BodyBudget, JwtContextExtractor, McpContextMiddleware, MountLayers,
    ProtectionGuards, PublicContextMiddleware, RateLimitState, UserOnlyContextMiddleware,
    ip_ban_middleware,
};

pub(super) fn configure_routes(
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
    engines: &Arc<GatewayEngines>,
) -> Result<Router, RouteMountError> {
    build_routes(ctx, ctx.config().role, events, engines)
}

pub fn configure_routes_for_role(
    ctx: &AppContext,
    role: NodeRole,
    events: Option<&StartupEventSender>,
    engines: &Arc<GatewayEngines>,
) -> anyhow::Result<Router> {
    Ok(build_routes(ctx, role, events, engines)?)
}

fn build_routes(
    ctx: &AppContext,
    role: NodeRole,
    events: Option<&StartupEventSender>,
    engines: &Arc<GatewayEngines>,
) -> Result<Router, RouteMountError> {
    let groups = role::route_groups(role);
    let serves = |group: RouteGroup| groups.contains(&group);
    let mut router = Router::new();

    let metrics = ctx
        .extension_registry()
        .all_metrics(&ctx.profile().extensions.disabled)?;
    super::metrics::install_recorder(&ctx.config().instance_id, &metrics)
        .map_err(|source| RouteMountError::initialization("prometheus_metrics", source))?;

    let layers = mount_layers(ctx, role)?;
    let refresh_lock = RefreshLock::default();

    let mount = MountCtx {
        ctx,
        engines,
        layers: &layers,
        serves: &serves,
    };
    if serves(RouteGroup::Oauth) {
        router = oauth_mount::mount_oauth(router, &mount)?;
    }
    if serves(RouteGroup::Agent) {
        router = protocol::mount_agent(router, &mount)?;
    }
    if serves(RouteGroup::McpAndStream) {
        router = protocol::mount_mcp_and_stream(router, &mount)?;
    }
    if serves(RouteGroup::ContentAndMisc) {
        router = mount_content(router, &mount)?;
    }
    if serves(RouteGroup::InternalLinks) {
        router = documents::mount_internal_links(router, &mount)?;
    }
    if serves(RouteGroup::Gateway) {
        router = gateway::mount_gateway(router, &mount)?;
    }
    router = feature_mount::mount_feature_routes(router, &mount)?;
    if serves(RouteGroup::Extensions) {
        router = extension_mount::mount_extension_routes(router, &mount, &refresh_lock, events)?;
    }
    router = mount_discovery(router, &mount)?;
    if serves(RouteGroup::Oauth) {
        router = mount.mount(router, link_passkey_router())?;
    }

    router = router.layer(axum::Extension(refresh_lock));
    finish_router(router, &mount, events)
}

fn mount_layers(ctx: &AppContext, role: NodeRole) -> Result<MountLayers, LoaderError> {
    let jwt_extractor = JwtContextExtractor::from_app_context(ctx)?;
    let limits = if role.shares_process_ownership() {
        RateLimitState::from_context(ctx).unmetered()
    } else {
        RateLimitState::from_context(ctx)
    };
    Ok(MountLayers {
        limits,
        public: PublicContextMiddleware::new(),
        user: UserOnlyContextMiddleware::new(jwt_extractor.clone()),
        a2a: A2AContextMiddleware::new(
            jwt_extractor.clone(),
            systemprompt_models::net::AGENT_REQUEST_BODY_LIMIT_BYTES,
            BodyBudget::new(systemprompt_models::net::AGENT_BUFFERED_BYTES_BUDGET),
        ),
        mcp: McpContextMiddleware::new(jwt_extractor.clone()),
        bearer: Some(Arc::new(jwt_extractor)),
        guards: ProtectionGuards::from_context(ctx)?,
    })
}

fn finish_router(
    mut router: Router,
    mount: &MountCtx<'_>,
    events: Option<&StartupEventSender>,
) -> Result<Router, RouteMountError> {
    let ctx = mount.ctx;
    let bans = require_provider(
        "ip_ban",
        ctx.extension_state().find::<DynIpBanStore>().cloned(),
    )?;
    let trusted_proxies = Arc::new(ctx.config().trusted_proxies.clone());

    router = router.layer(axum::middleware::from_fn(move |req, next| {
        let repo = Arc::clone(&bans);
        let proxies = Arc::clone(&trusted_proxies);
        async move { ip_ban_middleware(req, next, repo, proxies).await }
    }));

    if (mount.serves)(RouteGroup::Static) {
        router = router.merge(static_setup::build_static_router(
            ctx,
            mount.layers,
            events,
        )?);
    }

    Ok(router.layer(axum::middleware::from_fn(super::metrics::track_metrics)))
}

fn mount_discovery(mut router: Router, mount: &MountCtx<'_>) -> Result<Router, RouteMountError> {
    let ctx = mount.ctx;
    let serves = mount.serves;
    if serves(RouteGroup::Discovery) {
        router = mount.mount(
            router,
            declare(
                super::builder::discovery_router(ctx)?,
                WEB_ROOT_BASE_PATH,
                AuthzPolicy::public(),
                ContextKind::Public,
                RouteGroup::Discovery,
            ),
        )?;
    }
    if serves(RouteGroup::Probes) {
        router = mount.mount(
            router,
            declare(
                super::builder::probes_router(ctx)?,
                WEB_ROOT_BASE_PATH,
                AuthzPolicy::public(),
                ContextKind::Public,
                RouteGroup::Probes,
            ),
        )?;
    }
    if serves(RouteGroup::AuthenticatedDiscovery) {
        router = mount.mount(
            router,
            declare(
                super::builder::authenticated_discovery_router(ctx)?,
                WEB_ROOT_BASE_PATH,
                AuthzPolicy::authenticated(),
                ContextKind::User,
                RouteGroup::AuthenticatedDiscovery,
            ),
        )?;
    }
    if serves(RouteGroup::WellKnown) {
        router = mount.mount(
            router,
            declare(
                wellknown_router(ctx)?,
                WEB_ROOT_BASE_PATH,
                AuthzPolicy::public(),
                ContextKind::Public,
                RouteGroup::WellKnown,
            ),
        )?;
    }
    Ok(router)
}

fn link_passkey_router() -> systemprompt_extension::ExtensionRouter {
    declare(
        Router::new().route(
            "/auth/link-passkey",
            axum::routing::get(crate::routes::oauth::webauthn::link::link_passkey_page),
        ),
        WEB_ROOT_BASE_PATH,
        AuthzPolicy::public(),
        ContextKind::Public,
        RouteGroup::Oauth,
    )
    .with_rate_limit(RateLimit::new("oauth_public", RateBudget::OauthPublic))
}

fn require_provider<T: std::any::Any>(
    extension: &str,
    provider: Option<T>,
) -> Result<T, LoaderError> {
    provider.ok_or_else(|| LoaderError::missing_provider::<T>(ExtensionId::new(extension)))
}

fn wellknown_router(ctx: &AppContext) -> Result<Router, LoaderError> {
    Ok(crate::routes::oauth::wellknown_routes(ctx).merge(crate::routes::wellknown_router(ctx)?))
}

fn mount_content(mut router: Router, mount: &MountCtx<'_>) -> Result<Router, RouteMountError> {
    router = protocol::mount_content_and_misc(router, mount)?;
    router = documents::mount_documents(router, mount)?;
    router = issues::mount_issues(router, mount)?;
    #[cfg(feature = "notifications")]
    {
        router = notifications::mount_notifications(router, mount)?;
    }
    #[cfg(feature = "brain")]
    {
        router = brain::mount_brain_routes(router, mount)?;
    }
    Ok(router)
}
