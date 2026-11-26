//! Full API router construction and the global middleware stack.
//!
//! [`setup_api_server`] composes the route tree over the gateway operations
//! the `gateway` member provides (sealed; `admitted_gateway_operations`
//! reads them from the kernel state, and their absence means the node builds
//! no gateway route), and applies the global layers
//! (panic fence, body limit, analytics, context, session, CORS, trailing-slash,
//! trace and served-by headers, content negotiation, security headers) in the
//! order they must run. Before any extension factory runs (the gateway guard
//! factories build with the engines, the routers after them), it refuses an
//! extension that takes a core row it does not own; that is the one owner
//! check on the boot path, and every router and gateway engine below it is
//! built downstream of it. The server run loop reads the gateway operations
//! once and hands them to the router (`assemble_api_server`), to the
//! accounting recovery boot step and to the brain consumer's model client.
//! Binding and serving live in
//! [`super::startup`], which binds the listener before this router exists and
//! swaps it in once bootstrap completes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use systemprompt_config::paths::AppPaths;
use systemprompt_extension::router::DynGatewayOperations;
use systemprompt_loader::BootContext;
use systemprompt_manifest::Config;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{StartupEventExt, StartupEventSender};

use super::readiness::ReadinessState;
use super::routes::configure_routes;
use crate::services::middleware::{
    AnalyticsMiddleware, CorsMiddleware, DEFAULT_BODY_LIMIT_BYTES, PublicContextMiddleware,
    RateLimitState, SessionMiddleware, inject_security_headers, inject_served_by,
    inject_trace_header, panic_fence, remove_trailing_slash, with_pre_auth_ip_limit,
};

pub use super::discovery::{
    authenticated_discovery_router, discovery_router, handle_agents_discovery,
    handle_core_discovery, handle_mcp_discovery, handle_root_discovery, probes_router,
};
pub use super::health::handle_health;

pub fn setup_api_server(
    ctx: &AppContext,
    composed: super::routes::Composed<'_>,
    events: Option<&StartupEventSender>,
    readiness: &Arc<ReadinessState>,
) -> Result<Router> {
    let gateway = admitted_gateway_operations(&composed);
    assemble_api_server(ctx, composed, events, readiness, gateway.as_ref())
}

pub(crate) fn admitted_gateway_operations(
    composed: &super::routes::Composed<'_>,
) -> Option<DynGatewayOperations> {
    let Some(gateway) = crate::routes::gateway::gateway_operations(composed.kernel) else {
        tracing::info!(
            reason = "gateway_member_not_admitted",
            "Gateway routes not built"
        );
        return None;
    };
    Some(gateway)
}

pub(crate) fn assemble_api_server(
    ctx: &AppContext,
    composed: super::routes::Composed<'_>,
    events: Option<&StartupEventSender>,
    readiness: &Arc<ReadinessState>,
    gateway: Option<&DynGatewayOperations>,
) -> Result<Router> {
    let state = ctx.kernel().state();
    let config = state.get::<Arc<Config>>()?;
    let services_cache =
        systemprompt_loader::bundle::cache_root(state.get::<BootContext>()?.profile());
    systemprompt_config::ensure_state_dirs_writable(
        &state
            .get::<Arc<AppPaths>>()?
            .writable_roots(config.role, &services_cache),
    )?;

    let rate_config = &config.rate_limits;

    if rate_config.disabled
        && let Some(tx) = events
    {
        tx.warning("Rate limiting disabled - development mode only");
    }

    for member in composed.admitted.withheld() {
        tracing::warn!(
            member = %member.owner,
            dependency = %member.dependency,
            reason = member.because.as_str(),
            "Member withheld with its dependency; its routes are not mounted"
        );
    }

    let router = configure_routes(ctx, composed, events, gateway)?
        .layer(axum::Extension(Arc::clone(readiness)));
    apply_global_middleware(router, ctx)
}

fn apply_global_middleware(router: Router, ctx: &AppContext) -> Result<Router> {
    let config = Arc::clone(ctx.kernel().state().get::<Arc<Config>>()?);
    let mut router = router.layer(panic_fence());

    router = router.layer(DefaultBodyLimit::max(DEFAULT_BODY_LIMIT_BYTES));

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
        move |req, next| async move { middleware.seed(req, next).await }
    }));

    let session_middleware = SessionMiddleware::new(ctx)?;
    router = router.layer(axum::middleware::from_fn({
        let middleware = session_middleware;
        move |req, next| {
            let middleware = middleware.clone();
            async move { middleware.handle(req, next).await }
        }
    }));

    router = with_pre_auth_ip_limit(router, &RateLimitState::from_context(ctx)?);

    let cors = CorsMiddleware::build_layer(&config)?;
    router = router.layer(cors);

    router = router.layer(axum::middleware::from_fn(remove_trailing_slash));

    router = router.layer(axum::middleware::from_fn(inject_trace_header));

    router = router.layer(axum::middleware::from_fn_with_state(
        config.instance_id.clone(),
        inject_served_by,
    ));

    router = router.layer(axum::middleware::from_fn(
        crate::services::middleware::strip_caller_purpose,
    ));

    if config.content_negotiation.enabled {
        router = router.layer(axum::middleware::from_fn(
            systemprompt_extension::router::content_negotiation_middleware,
        ));
    }

    if config.security_headers.enabled {
        let security_config = config.security_headers.clone();
        router = router.layer(axum::middleware::from_fn(move |req, next| {
            let config = security_config.clone();
            inject_security_headers(config, req, next)
        }));
    }

    let router = router.layer(axum::Extension(Arc::new(
        systemprompt_security::http::EdgeConfigView::of(&config),
    )));
    Ok(crate::services::middleware::load_shed::outermost(
        router,
        crate::services::middleware::load_shed::InFlightLimits {
            requests: config.max_in_flight,
            probes: config.max_probe_in_flight,
        },
    ))
}
