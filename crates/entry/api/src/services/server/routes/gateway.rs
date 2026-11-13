//! Gateway route mount: the inference router plus its public session routes.
//! A gateway that cannot be built fails startup.
//!
//! The inference router authenticates inside its handlers (its own credential
//! extraction accepts `x-api-key`), so it is mounted with the rate limits and
//! bearer windows `gateway_mount_router` applies and no context middleware;
//! the public session routes are a declaration like every other group.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::RouteMountError;
use axum::Router;
use systemprompt_extension::{AuthzPolicy, ContextKind, RateBudget, RateLimit, RouteGroup};
use systemprompt_models::modules::ApiPaths;

use super::protocol::{MountCtx, declare};

pub(super) fn mount_gateway(
    mut router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, RouteMountError> {
    let ctx = mount.ctx;
    let gateway =
        crate::routes::gateway::gateway_mount_router(ctx, &mount.layers.limits, mount.engines)
            .map_err(|source| RouteMountError::initialization("gateway", source))?;
    router = router.nest(ApiPaths::GATEWAY_BASE, gateway);
    router = mount.mount(
        router,
        declare(
            crate::routes::gateway::sessions::public_router(ctx),
            ApiPaths::GATEWAY_PUBLIC_BASE,
            AuthzPolicy::public(),
            ContextKind::Public,
            RouteGroup::Gateway,
        )
        .with_rate_limit(RateLimit::new("oauth_public", RateBudget::OauthPublic)),
    )?;
    Ok(router)
}
