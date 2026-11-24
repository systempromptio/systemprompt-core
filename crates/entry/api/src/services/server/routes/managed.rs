//! Mounts the device-credential consumer routes the `marketplace-http`
//! member's
//! [`ManagedSurface`](systemprompt_marketplace::surface::ManagedSurface)
//! builds, under `/api/v1`; absent when `marketplace-http` is not admitted.
//!
//! The member owns the handlers, the origin guard and the consumer contract
//! normaliser; this mount owns the protection between them: the origin guard
//! sits innermost, the group is handler-verified by the device credential
//! with the fixed 10/s `consumer_evidence` limit applied by the declaration,
//! and the normaliser wraps the protected router outermost so a refusal from
//! any inner layer is normalised too.
//!
//! [`BridgeConsumerCatalog`] is the host read the surface takes: the
//! marketplace candidate a consumer's bridge manifest resolves to, assembled
//! by the bridge manifest path that still lives in this crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;
use systemprompt_extension::reserved::CoreId;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, HandlerVerifier, RateBudget, RateLimit, RouteProtection,
};
use systemprompt_identifiers::{ExtensionId, UserId};
use systemprompt_marketplace::MarketplaceCandidate;
use systemprompt_marketplace::surface::{ConsumerCatalog, DynManagedSurface, ManagedRouteState};
use systemprompt_runtime::AppContext;
use systemprompt_traits::BoxedSource;

use super::RouteMountError;
use super::protocol::MountCtx;
use crate::services::middleware::place;

const CONSUMER_BASE: &str = "/api/v1";

struct BridgeConsumerCatalog {
    ctx: AppContext,
}

#[async_trait::async_trait]
impl ConsumerCatalog for BridgeConsumerCatalog {
    async fn candidate(&self, consumer: &UserId) -> Result<MarketplaceCandidate, BoxedSource> {
        let services = self.ctx.services_config();
        let (candidate, _) = crate::routes::gateway::bridge_manifest::assemble_candidate(
            &self.ctx,
            self.ctx.profile(),
            consumer,
            (*services).clone(),
            crate::routes::gateway::bridge_resolved::Freshness::Memo,
        )
        .await
        .map_err(|error| -> BoxedSource { Box::new(error) })?;
        Ok(candidate)
    }
}

pub(super) fn mount(router: Router, mount: &MountCtx<'_>) -> Result<Router, RouteMountError> {
    let owner = ExtensionId::new(CoreId::MarketplaceHttp.id());
    let kernel = mount.composed.kernel;
    let (Some(standing), Some(surface)) = (
        mount.composed.admitted.built().standing_of(&owner),
        kernel.state().find::<DynManagedSurface>(),
    ) else {
        return Ok(router);
    };
    let built = surface
        .build(ManagedRouteState {
            resolved: kernel.route_scoped(&owner, standing, &[]),
            catalog: Arc::new(BridgeConsumerCatalog {
                ctx: mount.ctx.clone(),
            }),
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
            CONSUMER_BASE,
        )
    })?;
    Ok(place(router, CONSUMER_BASE, protected))
}
