//! Gateway route mount: the inference surface plus its public session
//! routes. A gateway that cannot be built fails startup; a node that does not
//! admit the `gateway` member builds no engines and mounts none of it.
//!
//! The inference surface authenticates inside its handlers (its own
//! credential extraction accepts `x-api-key`), so each of its parts —
//! inference traffic and the bridge credential exchange — is protected as
//! handler-verified by the gateway credential with its own rate limit and
//! bearer window, and every admitted gateway route (the bridge inbox) carries
//! the protection its admission resolved. The parts are merged under the access
//! log, which sits outside every limiter so a refused request is still
//! recorded. The public session routes are a declaration like every other
//! group.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;
use systemprompt_extension::{AuthzPolicy, ContextKind, RateBudget, RateLimit, RouteGroup};
use systemprompt_gateway::GatewayEngines;
use systemprompt_models::modules::ApiPaths;

use super::RouteMountError;
use super::protocol::{MountCtx, declare};
use crate::routes::gateway::{GatewayMountParts, gateway_mount_parts, with_access_log};
use crate::services::middleware::{gateway_protection, place};

pub(super) fn mount_gateway(
    mut router: Router,
    mount: &MountCtx<'_>,
    engines: &Arc<GatewayEngines>,
) -> Result<Router, RouteMountError> {
    let ctx = mount.ctx;
    let base = ApiPaths::GATEWAY_BASE;
    let GatewayMountParts {
        traffic,
        bridge_auth,
        registered,
    } = gateway_mount_parts(ctx, engines, mount.composed.admitted, mount.composed.kernel)
        .map_err(|source| RouteMountError::initialization("gateway", source))?;
    let traffic = mount.protect(
        traffic,
        &gateway_protection("gateway", RateBudget::Gateway),
        base,
    )?;
    let bridge_auth = mount.protect(
        bridge_auth,
        &gateway_protection("bridge_auth", RateBudget::BridgeAuth),
        base,
    )?;
    let mut surface = traffic.merge(bridge_auth);
    for group in registered {
        surface = surface.merge(mount.protect(group.router, &group.protection, base)?);
    }
    router = place(router, base, with_access_log(ctx, surface));
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
