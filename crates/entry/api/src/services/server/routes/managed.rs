//! The device-credential consumer surface the bridge reports installation
//! evidence through.
//!
//! The group is handler-verified by the device credential: the origin guard
//! sits innermost, the fixed 10/s `consumer_evidence` limit is applied by the
//! declaration, and the consumer contract normaliser wraps the protected
//! router outermost so a refusal from any inner layer is normalised too.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, HandlerVerifier, RateBudget, RateLimit, RouteProtection,
};

use super::protocol::MountCtx;
use crate::services::middleware::{MountError, place};

const CONSUMER_BASE: &str = "/api/v1";

pub(super) fn mount(router: Router, mount: &MountCtx<'_>) -> Result<Router, MountError> {
    let consumer = crate::routes::managed::consumer::router()
        .layer(axum::middleware::from_fn_with_state(
            mount.ctx.clone(),
            crate::routes::managed::origin::protect,
        ))
        .with_state(mount.ctx.clone());
    let protected = mount
        .protect(
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
        )?
        .layer(axum::middleware::from_fn(
            crate::routes::managed::contract::normalize,
        ));
    Ok(place(router, CONSUMER_BASE, protected))
}
