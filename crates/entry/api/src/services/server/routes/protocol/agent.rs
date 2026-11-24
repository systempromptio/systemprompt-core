//! The agent protocol mounts: the routes the `agent-http` member's
//! [`AgentSurface`](systemprompt_extension::router::AgentSurface) builds, and
//! the A2A proxy. The member owns the surface's handlers; this mount owns
//! their layering.
//!
//! The webhook broadcasts are mounted at `/api/v1/webhook` behind the
//! authenticated gate with a user request context under the `webhook` rate
//! scope; the A2A proxy at `/api/v1/agents` behind the authenticated gate
//! with the A2A context, the agent body limit and the `agents` scope. The
//! exact `/api/v1/agents/registry` reservation belongs to `agent-http`: with
//! the member not admitted an authenticated caller gets 404 there, from its
//! own declaration under the user context, so the request never reaches the
//! proxy's `/{service_name}` route or its A2A context extraction. The
//! well-known agent cards are merged under the public discovery declaration.
//! Every surface route is absent (404) when `agent-http` is not admitted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use axum::http::StatusCode;
use axum::routing::any;
use systemprompt_extension::reserved::{CoreId, WEB_ROOT_BASE_PATH};
use systemprompt_extension::router::{AgentRouteState, AgentSurfaceRoutes, DynAgentSurface};
use systemprompt_extension::{AuthzPolicy, ContextKind, RateBudget, RateLimit, RouteGroup};
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::modules::ApiPaths;

use super::super::RouteMountError;
use super::{MountCtx, declare};
use crate::services::middleware::MountError;

pub const WEBHOOK_RATE: RateLimit = RateLimit::new("webhook", RateBudget::PerSecond(200));

#[derive(Debug, Default)]
pub(in crate::services::server::routes) struct AgentMounts {
    pub(in crate::services::server::routes) webhook: Option<Router>,
    pub(in crate::services::server::routes) stream: Option<Router>,
    pub(in crate::services::server::routes) cards: Option<Router>,
}

impl From<AgentSurfaceRoutes> for AgentMounts {
    fn from(routes: AgentSurfaceRoutes) -> Self {
        Self {
            webhook: Some(routes.webhook),
            stream: Some(routes.stream),
            cards: Some(routes.agent_cards),
        }
    }
}

pub(in crate::services::server::routes) fn agent_routes(
    mount: &MountCtx<'_>,
) -> Result<AgentMounts, RouteMountError> {
    let owner = ExtensionId::new(CoreId::AgentHttp.id());
    let kernel = mount.composed.kernel;
    let (Some(standing), Some(surface)) = (
        mount.composed.admitted.built().standing_of(&owner),
        kernel.state().find::<DynAgentSurface>(),
    ) else {
        return Ok(AgentMounts::default());
    };
    surface
        .build(AgentRouteState {
            resolved: kernel.route_scoped(&owner, standing, &[]),
        })
        .map(AgentMounts::from)
        .map_err(|source| RouteMountError::initialization("agent_surface", source))
}

pub(in crate::services::server::routes) fn mount_agent(
    mut router: Router,
    mount: &MountCtx<'_>,
    webhook: Option<Router>,
) -> Result<Router, MountError> {
    let ctx = mount.ctx;
    let group = RouteGroup::Agent;

    if let Some(webhook) = webhook {
        router = mount.mount(
            router,
            declare(
                webhook,
                ApiPaths::WEBHOOK,
                AuthzPolicy::authenticated(),
                ContextKind::User,
                group,
            )
            .with_rate_limit(WEBHOOK_RATE),
        )?;
    }

    if agent_http_absent(mount) {
        router = mount.mount(
            router,
            declare(
                Router::new().route("/", any(|| async { StatusCode::NOT_FOUND })),
                ApiPaths::AGENTS_REGISTRY,
                AuthzPolicy::authenticated(),
                ContextKind::User,
                group,
            ),
        )?;
    }

    mount.mount(
        router,
        declare(
            crate::routes::proxy::agents::router(ctx)?,
            ApiPaths::AGENTS_BASE,
            AuthzPolicy::authenticated(),
            ContextKind::A2a,
            group,
        )
        .with_body_limit(systemprompt_models::net::AGENT_REQUEST_BODY_LIMIT_BYTES)
        .with_rate_limit(RateLimit::new("agents", RateBudget::Agents)),
    )
}

pub(in crate::services::server::routes) fn mount_agent_cards(
    router: Router,
    mount: &MountCtx<'_>,
    cards: Option<Router>,
) -> Result<Router, MountError> {
    let Some(cards) = cards.filter(|_| (mount.serves)(RouteGroup::WellKnown)) else {
        return Ok(router);
    };
    mount.mount(
        router,
        declare(
            cards,
            WEB_ROOT_BASE_PATH,
            AuthzPolicy::public(),
            ContextKind::Public,
            RouteGroup::WellKnown,
        ),
    )
}

fn agent_http_absent(mount: &MountCtx<'_>) -> bool {
    mount
        .composed
        .admitted
        .built()
        .standing_of(&ExtensionId::new(CoreId::AgentHttp.id()))
        .is_none()
}
