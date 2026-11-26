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
//! The host read the surface takes is the consumer catalogue the
//! `gateway-http` surface builds: the marketplace candidate a consumer's
//! bridge manifest resolves to. Without `gateway-http`
//! there is no bridge manifest and these routes are not mounted (404).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::reserved::CoreId;
use systemprompt_extension::router::{DynConsumerCatalog, DynManagedSurface, ManagedRouteState};
use systemprompt_extension::{
    AuthzPolicy, ContextKind, HandlerVerifier, RateBudget, RateLimit, RouteProtection,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::modules::ApiPaths;

use super::RouteMountError;
use super::protocol::MountCtx;
use crate::services::middleware::place;

pub(super) fn mount(
    router: Router,
    mount: &MountCtx<'_>,
    catalog: DynConsumerCatalog,
) -> Result<Router, RouteMountError> {
    let owner = ExtensionId::new(CoreId::MarketplaceHttp.id());
    let kernel = mount.composed.kernel;
    let Some(standing) = mount.composed.admitted.built().standing_of(&owner) else {
        return Ok(router);
    };
    let Some(surface) = kernel.state().find::<DynManagedSurface>() else {
        return Ok(router);
    };
    let built = surface
        .build(ManagedRouteState {
            resolved: kernel.route_scoped(&owner, standing, &[]),
            catalog,
        })
        .map_err(|source| RouteMountError::initialization("managed_consumer", source))?;
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
            ApiPaths::API_V1,
        )
    })?;
    Ok(place(router, ApiPaths::API_V1, protected))
}
