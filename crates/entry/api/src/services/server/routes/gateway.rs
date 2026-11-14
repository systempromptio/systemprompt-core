//! Gateway route mount: the inference surface plus its public session
//! routes. A gateway that cannot be built fails startup.
//!
//! The inference surface authenticates inside its handlers (its own
//! credential extraction accepts `x-api-key`), so each of its three halves —
//! inference traffic, the bridge credential exchange and the bridge inbox —
//! is protected as handler-verified by the gateway credential with its own
//! rate limit and bearer window. The halves are merged under the access log,
//! which sits outside every limiter so a refused request is still recorded.
//! The public session routes are a declaration like every other group.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, HandlerVerifier, RateBudget, RateLimit, RouteGroup, RouteProtection,
};
use systemprompt_models::modules::ApiPaths;

use super::RouteMountError;
use super::protocol::{MountCtx, declare};
use crate::routes::gateway::{GatewayMountParts, gateway_mount_parts, with_access_log};
use crate::services::middleware::place;

const fn gateway_credential(scope: &'static str, budget: RateBudget) -> RouteProtection {
    RouteProtection::new(
        AuthzPolicy::handler_verified(HandlerVerifier::GatewayCredential),
        ContextKind::HandlerResolved,
    )
    .with_rate_limit(RateLimit::new(scope, budget).with_bearer_window())
}

pub(super) fn mount_gateway(
    mut router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, RouteMountError> {
    let ctx = mount.ctx;
    let base = ApiPaths::GATEWAY_BASE;
    let GatewayMountParts {
        traffic,
        bridge_auth,
        notifications,
    } = gateway_mount_parts(ctx, mount.engines)
        .map_err(|source| RouteMountError::initialization("gateway", source))?;
    let traffic = mount.protect(
        traffic,
        &gateway_credential("gateway", RateBudget::Gateway),
        base,
    )?;
    let bridge_auth = mount.protect(
        bridge_auth,
        &gateway_credential("bridge_auth", RateBudget::BridgeAuth),
        base,
    )?;
    let notifications = mount.protect(
        notifications,
        &gateway_credential("bridge_notifications", RateBudget::Notifications),
        base,
    )?;
    router = place(
        router,
        base,
        with_access_log(ctx, traffic.merge(bridge_auth).merge(notifications)),
    );
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
