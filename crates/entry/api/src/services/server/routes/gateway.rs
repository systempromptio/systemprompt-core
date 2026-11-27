//! Gateway route mount: the `gateway-http` surface, the `identity-http`
//! bridge credential exchange and the public session routes. A gateway that
//! cannot be built fails startup; a node that does not admit the `gateway`
//! member has no gateway operations and mounts none of it, and a member that
//! is not admitted contributes nothing (404).
//!
//! The inference surface authenticates inside its handlers (its own
//! credential extraction accepts `x-api-key`), so each of its parts —
//! inference traffic, the bridge credential exchange and the bridge release
//! feed — is protected as
//! handler-verified by the gateway credential with its own rate limit and
//! bearer window, and every admitted gateway route (the bridge inbox) carries
//! the protection its admission resolved. The parts are merged under the
//! `gateway` member's access log, which sits outside every limiter so a
//! refused request is still recorded. The public session routes are a
//! declaration like every other group.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::router::{DynConsumerCatalog, DynGatewayOperations};
use systemprompt_extension::{AuthzPolicy, ContextKind, RateBudget, RateLimit, RouteGroup};
use systemprompt_models::modules::ApiPaths;

use super::RouteMountError;
use super::protocol::{MountCtx, declare};
use crate::routes::gateway::{GatewayMountParts, gateway_mount_parts};
use crate::services::middleware::{gateway_protection, place};

// Why: a bridge reads the release feed on a timer and a phone downloads its
// app once, so the rate per caller is fixed and small rather than a profile
// budget.
const BRIDGE_RELEASE_PER_SECOND: u64 = 5;

fn mount_gateway(
    mut router: Router,
    mount: &MountCtx<'_>,
    gateway: &DynGatewayOperations,
    identity_bridge_auth: Option<Router>,
) -> Result<(Router, Option<DynConsumerCatalog>), RouteMountError> {
    let ctx = mount.ctx;
    let base = ApiPaths::GATEWAY_BASE;
    let GatewayMountParts {
        traffic,
        bridge_auth,
        bridge_release,
        public_sessions,
        consumer_catalog,
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
    let bridge_release = mount.protect(
        bridge_release,
        &gateway_protection(
            "bridge_release",
            RateBudget::PerSecond(BRIDGE_RELEASE_PER_SECOND),
        ),
        base,
    )?;
    let mut surface = traffic.merge(bridge_auth).merge(bridge_release);
    for group in registered {
        surface = surface.merge(mount.protect(group.router, &group.protection, base)?);
    }
    let surface = gateway
        .access_log(&ctx.kernel().resolved(), surface)
        .map_err(|source| RouteMountError::initialization("gateway_access_log", source))?;
    router = place(router, base, surface);
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
    Ok((router, consumer_catalog))
}

pub(super) fn mount_with_features(
    router: Router,
    mount: &MountCtx<'_>,
    gateway: Option<&DynGatewayOperations>,
    identity_bridge_auth: Option<Router>,
) -> Result<Router, RouteMountError> {
    let (router, consumer_catalog) = match gateway {
        Some(gateway) => mount_gateway(router, mount, gateway, identity_bridge_auth)?,
        None => (router, None),
    };
    super::feature_mount::mount_feature_routes(router, mount, consumer_catalog)
}
