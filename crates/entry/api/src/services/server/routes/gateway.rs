//! Gateway route mount: the `gateway-http` surface, the `identity-http`
//! bridge credential exchange and the public session routes. A gateway that
//! cannot be built fails startup; a node that does not admit the `gateway`
//! member builds no engines and mounts none of it, and a member that is not
//! admitted contributes nothing (404).
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

use axum::Router;
use systemprompt_extension::{AuthzPolicy, ContextKind, RateBudget, RateLimit, RouteGroup};
use systemprompt_gateway::surface::ManagedConsumer;
use systemprompt_models::modules::ApiPaths;

use super::RouteMountError;
use super::protocol::{MountCtx, declare};
use crate::routes::gateway::{GatewayMountParts, gateway_mount_parts, with_access_log};
use crate::services::middleware::{gateway_protection, place};

fn mount_gateway(
    mut router: Router,
    mount: &MountCtx<'_>,
    identity_bridge_auth: Option<Router>,
) -> Result<(Router, Option<ManagedConsumer>), RouteMountError> {
    let ctx = mount.ctx;
    let base = ApiPaths::GATEWAY_BASE;
    let GatewayMountParts {
        traffic,
        bridge_auth,
        public_sessions,
        managed_consumer,
        registered,
    } = gateway_mount_parts(
        ctx,
        mount.composed.admitted,
        mount.composed.kernel,
        identity_bridge_auth.unwrap_or_default(),
    )
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
    router = place(router, base, with_access_log(ctx.logging(), surface));
    router = mount.mount(
        router,
        declare(
            public_sessions,
            ApiPaths::GATEWAY_PUBLIC_BASE,
            AuthzPolicy::public(),
            ContextKind::Public,
            RouteGroup::Gateway,
        )
        .with_rate_limit(RateLimit::new("oauth_public", RateBudget::OauthPublic)),
    )?;
    Ok((router, managed_consumer))
}

pub(super) fn mount_with_features(
    router: Router,
    mount: &MountCtx<'_>,
    gateway_on: bool,
    identity_bridge_auth: Option<Router>,
) -> Result<Router, RouteMountError> {
    let (router, managed_consumer) = if gateway_on {
        mount_gateway(router, mount, identity_bridge_auth)?
    } else {
        (router, None)
    };
    super::feature_mount::mount_feature_routes(router, mount, managed_consumer)
}
