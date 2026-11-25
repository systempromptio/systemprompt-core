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
#[cfg(feature = "bridge")]
mod feature_mount;
#[cfg(not(feature = "bridge"))]
mod feature_mount {
    #[expect(
        clippy::unnecessary_wraps,
        reason = "every route group mounted here is behind a cargo feature"
    )]
    pub(super) fn mount_feature_routes(
        router: axum::Router,
        _mount: &super::protocol::MountCtx<'_>,
        _managed_consumer: Option<systemprompt_gateway::surface::ManagedConsumer>,
    ) -> Result<axum::Router, super::RouteMountError> {
        Ok(router)
    }
}
mod gateway;
#[cfg(feature = "bridge")]
mod managed;
mod oauth_mount;
mod protocol;
pub mod role;
mod static_setup;

use axum::Router;
use std::sync::Arc;
use systemprompt_extension::RouteGroup;
use systemprompt_manifest::profile::NodeRole;

use crate::routes::admin::services::RefreshLock;
pub(super) use error::RouteMountError;
pub(crate) use oauth_mount::identity_bridge_auth;
use protocol::MountCtx;
pub use protocol::{Composed, WEBHOOK_RATE};
use systemprompt_gateway::GatewayEngines;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{DynIpBanStore, StartupEventSender};

use crate::services::middleware::{
    A2AContextMiddleware, BodyBudget, JwtContextExtractor, McpContextMiddleware, MountLayers,
    ProtectionGuards, PublicContextMiddleware, RateLimitState, RootFallbackOwner,
    UserOnlyContextMiddleware, ip_ban_middleware,
};

mod discovery_mount;
use discovery_mount::{mount_discovery, require_provider};

pub(super) fn configure_routes(
    ctx: &AppContext,
    composed: Composed<'_>,
    events: Option<&StartupEventSender>,
    engines: Option<&Arc<GatewayEngines>>,
) -> Result<Router, RouteMountError> {
    let role = ctx
        .kernel()
        .state()
        .get::<Arc<systemprompt_manifest::Config>>()?
        .role;
    build_routes(ctx, composed, role, events, engines)
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

fn install_metrics(ctx: &AppContext, composed: &Composed<'_>) -> Result<(), RouteMountError> {
    let metrics: Vec<_> = composed
        .admitted
        .built()
        .members()
        .flat_map(|member| member.sink.metrics().iter().copied())
        .collect();
    let config = ctx
        .kernel()
        .state()
        .get::<Arc<systemprompt_manifest::Config>>()?;
    super::metrics::install_recorder(&config.instance_id, &metrics)
        .map_err(|source| RouteMountError::initialization("prometheus_metrics", source))?;
    Ok(())
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

    install_metrics(ctx, &composed)?;

    let layers = mount_layers(ctx, role)?;
    let refresh_lock = RefreshLock::default();

    let mount = MountCtx {
        ctx,
        layers: &layers,
        serves: &serves,
        composed,
    };
    let identity = oauth_mount::identity_routes(&mount)?;
    let (identity_wellknown, identity_passkey, identity_accounts, identity_bridge_auth) =
        if let Some(routes) = identity {
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
                Some(routes.bridge_auth),
            )
        } else {
            (None, None, None, None)
        };
    let agent = protocol::agent_routes(&mount)?;
    if serves(RouteGroup::Agent) {
        router = protocol::mount_agent(router, &mount, agent.webhook)?;
    }
    if serves(RouteGroup::McpAndStream) {
        router = protocol::mount_mcp(router, &mount)?;
        router = protocol::mount_stream(router, &mount, agent.stream)?;
    }
    router = protocol::mount_agent_cards(router, &mount, agent.cards)?;
    if serves(RouteGroup::ContentAndMisc) {
        let (sessions, api_keys) = identity_accounts.unzip();
        router = mount_content(router, &mount, api_keys)?;
        if let Some(sessions) = sessions {
            router = oauth_mount::mount_sessions(router, &mount, sessions)?;
        }
    }
    if serves(RouteGroup::InternalLinks) {
        router = documents::mount_internal_links(router, &mount)?;
    }
    let gateway_on = serves(RouteGroup::Gateway) && engines.is_some();
    router = gateway::mount_with_features(router, &mount, gateway_on, identity_bridge_auth)?;
    crate::services::agent_dispatch::bind_dispatchers(ctx)
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
        RateLimitState::from_context(ctx)?.unmetered()
    } else {
        RateLimitState::from_context(ctx)?
    };
    let body_ceiling = ctx
        .kernel()
        .state()
        .get::<Arc<systemprompt_manifest::Config>>()?
        .max_route_body_bytes;
    Ok(MountLayers {
        body_ceiling,
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
    let trusted_proxies = Arc::new(
        ctx.kernel()
            .state()
            .get::<Arc<systemprompt_manifest::Config>>()?
            .trusted_proxies
            .clone(),
    );

    router = router.layer(axum::middleware::from_fn(move |req, next| {
        let repo = Arc::clone(&bans);
        let proxies = Arc::clone(&trusted_proxies);
        async move { ip_ban_middleware(req, next, repo, proxies).await }
    }));

    if (mount.serves)(RouteGroup::Static) {
        let site_fallback = mount
            .composed
            .admitted
            .site_fallback()
            .map(|site| {
                site.fallback
                    .materialize(&mount.composed.kernel.route_scoped(
                        &site.owner,
                        site.standing,
                        &[],
                    ))
            })
            .transpose()?;
        router = RootFallbackOwner::default().place(
            router,
            static_setup::build_static_router(ctx, mount.layers, events, site_fallback)?,
        )?;
    }

    Ok(router.layer(axum::middleware::from_fn(super::metrics::track_metrics)))
}

fn mount_content(
    mut router: Router,
    mount: &MountCtx<'_>,
    api_keys: Option<Router>,
) -> Result<Router, RouteMountError> {
    router = protocol::mount_content_and_misc(router, mount, api_keys)?;
    router = documents::mount_files(router, mount)?;
    Ok(router)
}
