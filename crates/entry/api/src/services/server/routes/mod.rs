//! Router assembly for the API server.
//!
//! `configure_routes` composes the route tree for this node's role: protocol
//! surfaces (OAuth, agent, MCP, stream, content, gateway), extension-mounted
//! routes (each declaration filtered by the role table for its own route
//! group), discovery and well-known endpoints, static content, and the global
//! IP-ban and metrics layers. [`role::route_groups`] decides which groups a
//! role mounts. Each surface is a route declaration applied by
//! `services::middleware::mount`: its `AuthzPolicy`, context middleware, rate
//! limit, body limit and protection layers come from the declaration.
//!
//! `configure_routes` is the boot path's assembly and runs downstream of the
//! core owner check `setup_api_server` makes before the gateway engines are
//! built; the public `configure_routes_for_role` makes that check itself.
//!
//! The static router is merged after the IP-ban layer is applied, so public
//! pages and assets are served without a ban-list lookup; only the API routes
//! are gated. Metrics stay outermost and still measure static responses.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod documents;
mod error;
mod extension_mount;
mod feature_mount;
mod gateway;
#[cfg(feature = "bridge")]
mod managed;
mod oauth_mount;
mod protocol;
pub mod role;
mod static_setup;

use axum::Router;
use std::sync::Arc;
use systemprompt_extension::reserved::WEB_ROOT_BASE_PATH;
use systemprompt_extension::{AuthzPolicy, ContextKind, LoaderError, RouteGroup};
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::profile::NodeRole;

use crate::routes::admin::services::RefreshLock;
pub(super) use error::RouteMountError;
pub use protocol::Composed;
use protocol::{MountCtx, declare};
use systemprompt_gateway::GatewayEngines;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{DynIpBanStore, StartupEventSender};

use crate::services::middleware::{
    A2AContextMiddleware, BodyBudget, JwtContextExtractor, McpContextMiddleware, MountLayers,
    ProtectionGuards, PublicContextMiddleware, RateLimitState, RootFallbackOwner,
    UserOnlyContextMiddleware, ip_ban_middleware,
};

pub(super) fn configure_routes(
    ctx: &AppContext,
    composed: Composed<'_>,
    events: Option<&StartupEventSender>,
    engines: Option<&Arc<GatewayEngines>>,
) -> Result<Router, RouteMountError> {
    build_routes(ctx, composed, ctx.config().role, events, engines)
}

pub fn configure_routes_for_role(
    ctx: &AppContext,
    composed: Composed<'_>,
    role: NodeRole,
    events: Option<&StartupEventSender>,
    engines: &Arc<GatewayEngines>,
) -> anyhow::Result<Router> {
    Ok(build_routes(ctx, composed, role, events, Some(engines))?)
}

fn build_routes(
    ctx: &AppContext,
    composed: Composed<'_>,
    role: NodeRole,
    events: Option<&StartupEventSender>,
    engines: Option<&Arc<GatewayEngines>>,
) -> Result<Router, RouteMountError> {
    let groups = role::route_groups(role);
    let serves = |group: RouteGroup| groups.contains(&group);
    let mut router = Router::new();

    let metrics: Vec<_> = composed
        .admitted
        .built()
        .members()
        .flat_map(|member| member.sink.metrics().iter().copied())
        .collect();
    super::metrics::install_recorder(&ctx.config().instance_id, &metrics)
        .map_err(|source| RouteMountError::initialization("prometheus_metrics", source))?;

    let layers = mount_layers(ctx, role)?;
    let refresh_lock = RefreshLock::default();

    let mount = MountCtx {
        ctx,
        layers: &layers,
        serves: &serves,
        composed,
    };
    let identity = oauth_mount::identity_routes(&mount)?;
    let (identity_wellknown, identity_passkey, identity_accounts) = if let Some(routes) = identity {
        if serves(RouteGroup::Oauth) {
            router = oauth_mount::mount_oauth(
                router,
                &mount,
                oauth_mount::IdentityOauthRoutes {
                    public: routes.oauth_public,
                    authenticated: routes.oauth_authenticated,
                    revocation: routes.oauth_revocation,
                    sso: routes.sso,
                },
            )?;
        }
        (
            Some(routes.wellknown),
            Some(routes.passkey_link_page),
            Some((routes.sessions, routes.api_keys)),
        )
    } else {
        (None, None, None)
    };
    if serves(RouteGroup::Agent) {
        router = protocol::mount_agent(router, &mount)?;
    }
    if serves(RouteGroup::McpAndStream) {
        router = protocol::mount_mcp(router, &mount)?;
        router = protocol::mount_stream(router, &mount)?;
    }
    if serves(RouteGroup::ContentAndMisc) {
        router = mount_content(router, &mount)?;
        if let Some((sessions, api_keys)) = identity_accounts {
            router = oauth_mount::mount_accounts(router, &mount, sessions, api_keys)?;
        }
    }
    if serves(RouteGroup::InternalLinks) {
        router = documents::mount_internal_links(router, &mount)?;
    }
    if serves(RouteGroup::Gateway) && engines.is_some() {
        router = gateway::mount_gateway(router, &mount)?;
    }
    router = feature_mount::mount_feature_routes(router, &mount)?;
    crate::services::messaging::bind_dispatcher(ctx)
        .map_err(|source| RouteMountError::initialization("messaging_dispatcher", source))?;
    router = extension_mount::mount_extension_routes(router, &mount, &refresh_lock, events)?;
    router = mount_discovery(router, &mount, identity_wellknown)?;
    if serves(RouteGroup::Oauth)
        && let Some(page) = identity_passkey
    {
        router = oauth_mount::mount_passkey_link_page(router, &mount, page)?;
    }

    router = router.layer(axum::Extension(refresh_lock));
    finish_router(router, &mount, events)
}

fn mount_layers(ctx: &AppContext, role: NodeRole) -> Result<MountLayers, RouteMountError> {
    let jwt_extractor = JwtContextExtractor::from_kernel(ctx.kernel().state())?;
    let limits = if role.shares_process_ownership() {
        RateLimitState::from_context(ctx).unmetered()
    } else {
        RateLimitState::from_context(ctx)
    };
    Ok(MountLayers {
        limits,
        public: PublicContextMiddleware::new(),
        user: UserOnlyContextMiddleware::new(jwt_extractor.clone()),
        user_ui: UserOnlyContextMiddleware::new(jwt_extractor.clone().admitting_ui_sessions()),
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
        ctx.kernel().state().find::<DynIpBanStore>().cloned(),
    )?;
    let trusted_proxies = Arc::new(ctx.config().trusted_proxies.clone());

    router = router.layer(axum::middleware::from_fn(move |req, next| {
        let repo = Arc::clone(&bans);
        let proxies = Arc::clone(&trusted_proxies);
        async move { ip_ban_middleware(req, next, repo, proxies).await }
    }));

    if (mount.serves)(RouteGroup::Static) {
        router = RootFallbackOwner::default().place(
            router,
            static_setup::build_static_router(ctx, mount.layers, events)?,
        )?;
    }

    Ok(router.layer(axum::middleware::from_fn(super::metrics::track_metrics)))
}

fn mount_discovery(
    mut router: Router,
    mount: &MountCtx<'_>,
    identity_wellknown: Option<Router>,
) -> Result<Router, RouteMountError> {
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
                wellknown_router(ctx, identity_wellknown)?,
                WEB_ROOT_BASE_PATH,
                AuthzPolicy::public(),
                ContextKind::Public,
                RouteGroup::WellKnown,
            ),
        )?;
    }
    Ok(router)
}

fn require_provider<T: std::any::Any>(
    extension: &str,
    provider: Option<T>,
) -> Result<T, LoaderError> {
    provider.ok_or_else(|| LoaderError::missing_provider::<T>(ExtensionId::new(extension)))
}

fn wellknown_router(ctx: &AppContext, identity: Option<Router>) -> Result<Router, LoaderError> {
    let kernel = crate::routes::wellknown_router(ctx)?;
    Ok(match identity {
        Some(identity) => identity.merge(kernel),
        None => kernel,
    })
}

fn mount_content(mut router: Router, mount: &MountCtx<'_>) -> Result<Router, RouteMountError> {
    router = protocol::mount_content_and_misc(router, mount)?;
    router = documents::mount_files(router, mount)?;
    Ok(router)
}
