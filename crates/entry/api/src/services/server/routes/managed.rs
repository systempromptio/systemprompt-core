//! Mounts the device-credential consumer routes the `marketplace-http`
//! member's consumer surface builds, under `/api/v1`; absent when
//! `marketplace-http` is not admitted.
//!
//! The member owns the handlers, the origin guard and the consumer contract
//! normaliser; this mount owns the protection between them: the origin guard
//! sits innermost, the group is handler-verified by the device credential
//! with the fixed 10/s `consumer_evidence` limit applied by the declaration,
//! and the normaliser wraps the protected router outermost so a refusal from
//! any inner layer is normalised too.
//!
//! The host read the surface takes is the
//! [`ManagedConsumer`] the `gateway-http` surface builds: the marketplace candidate a consumer's bridge manifest
//! resolves to. Without `gateway-http` there is no bridge manifest and these
//! routes are not mounted (404).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::reserved::CoreId;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, HandlerVerifier, RateBudget, RateLimit, RouteProtection,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_gateway::surface::ManagedConsumer;

use super::RouteMountError;
use super::protocol::MountCtx;
use crate::services::middleware::place;

const CONSUMER_BASE: &str = "/api/v1";

pub(super) fn mount(
    router: Router,
    mount: &MountCtx<'_>,
    managed_consumer: ManagedConsumer,
) -> Result<Router, RouteMountError> {
    let owner = ExtensionId::new(CoreId::MarketplaceHttp.id());
    let kernel = mount.composed.kernel;
    let Some(standing) = mount.composed.admitted.built().standing_of(&owner) else {
        return Ok(router);
    };
    let Some(built) = managed_consumer
        .build(kernel.state(), kernel.route_scoped(&owner, standing, &[]))
        .map_err(|source| RouteMountError::initialization("managed_consumer", source))?
    else {
        return Ok(router);
    };
    let protected = built.protect(|consumer| {
        mount.protect(
            consumer,
            &RouteProtection::new(
                AuthzPolicy::handler_verified(HandlerVerifier::ManagedDeviceCredential),
                ContextKind::HandlerResolved,
            )
            .with_rate_limit(RateLimit::new(
                "consumer_evidence",
                RateBudget::PerSecond(10),
            )),
            CONSUMER_BASE,
        )
    })?;
    Ok(place(router, CONSUMER_BASE, protected))
}
