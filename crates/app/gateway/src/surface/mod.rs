//! The gateway's HTTP surface contract.
//!
//! The `gateway-http` member contributes a [`GatewaySurface`]: one builder
//! the API mount calls with the route-scoped state of `gateway-http`
//! ([`GatewayRouteState`]). The member owns the handlers; the API owns the
//! layering around what the builder returns — the confined 404 outside the
//! built-in gateway prefixes, the rate scopes and bearer windows, and the
//! access log outside every limiter.
//!
//! [`GatewayRouteState::resolved`] is the kernel resolution scoped to
//! `gateway-http` with core standing: it serves the open state and the
//! sealed values conferred on `gateway-http` (the engines, the at-rest
//! material, the session mint) and nothing else sealed.
//! [`GatewayHostHandles`] carries the live process handles the kernel state
//! does not hold (the reloadable services and governance, the authorization
//! hook, the analytics extractor, the logging runtime) and the profile facts
//! the repositories are sized by.
//!
//! [`credential`] and [`authz`] are the request helpers the surface's
//! handlers share; [`GatewayLogIdentity`] is what a handler attaches to its
//! response for the access log. [`views`] are the narrow views of sealed
//! kernel seeds the `gateway` member confers on `gateway-http` for the bridge
//! routes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod authz;
pub mod credential;
pub mod repositories;
pub mod views;

use std::sync::Arc;

use axum::Router;
use systemprompt_extension::prelude::{Dependencies, MissingDependency, Resolved};
use systemprompt_governance::authz::SharedAuthzHook;
use systemprompt_governance::policy::GovernanceEngine;
use systemprompt_identifiers::{SessionId, TenantId, TraceId, UserId};
use systemprompt_loader::ServicesState;
use systemprompt_logging::LoggingRuntime;
use systemprompt_manifest::profile::{BrainProfile, QuotaConfig, UnappliedPlaneMode};
use systemprompt_marketplace::surface::{
    DynConsumerCatalog, DynManagedSurface, ManagedRouteState, ManagedRoutes,
};
use systemprompt_traits::{AnalyticsProvider, ConfigWatch};

/// Builds the gateway's handler routes from the `gateway-http` route state.
pub trait GatewaySurface: Send + Sync + std::fmt::Debug {
    fn build(
        &self,
        state: GatewayRouteState<'_>,
    ) -> Result<GatewaySurfaceRoutes, GatewaySurfaceError>;
}

/// The routes a [`GatewaySurface`] builds.
///
/// `traffic` verifies its own callers; the API mount places `principal_bound`
/// behind the kernel's gateway verifier, which attaches the caller's
/// [`GatewayPrincipal`](systemprompt_extension::router::GatewayPrincipal)
/// before a handler runs. `public_sessions` is the API-key session mint the
/// mount places under the public gateway base with its own rate scope.
/// `managed_consumer` is the bridge manifest's candidate resolution the
/// `marketplace-http` consumer routes read; absent when `gateway-http` is not
/// admitted, and those routes are then not mounted.
#[derive(Default)]
pub struct GatewaySurfaceRoutes {
    pub traffic: Router,
    pub principal_bound: Router,
    pub public_sessions: Router,
    pub managed_consumer: Option<ManagedConsumer>,
}

impl std::fmt::Debug for GatewaySurfaceRoutes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewaySurfaceRoutes")
            .field("managed_consumer", &self.managed_consumer.is_some())
            .finish_non_exhaustive()
    }
}

pub type DynGatewaySurface = Arc<dyn GatewaySurface>;

/// The bridge manifest's candidate resolution, and the build of the
/// `marketplace-http` consumer routes over it.
///
/// The API mount hands it the `marketplace-http` route state; the consumer
/// routes exist only when both `gateway-http` built this value and the
/// marketplace member provided its surface.
#[derive(Clone)]
pub struct ManagedConsumer {
    catalog: DynConsumerCatalog,
}

impl ManagedConsumer {
    #[must_use]
    pub const fn new(catalog: DynConsumerCatalog) -> Self {
        Self { catalog }
    }

    #[must_use]
    pub const fn catalog(&self) -> &DynConsumerCatalog {
        &self.catalog
    }

    pub fn build(
        self,
        state: &Dependencies,
        resolved: Resolved<'_>,
    ) -> Result<Option<ManagedRoutes>, MissingDependency> {
        let Some(surface) = state.find::<DynManagedSurface>() else {
            return Ok(None);
        };
        surface
            .build(ManagedRouteState {
                resolved,
                catalog: self.catalog,
            })
            .map(Some)
    }
}

impl std::fmt::Debug for ManagedConsumer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedConsumer").finish_non_exhaustive()
    }
}

/// The state the API mount hands a [`GatewaySurface`].
#[derive(Debug)]
pub struct GatewayRouteState<'a> {
    pub resolved: Resolved<'a>,
    pub host: GatewayHostHandles,
}

/// The live process handles and profile facts the gateway surface reads that
/// the kernel state does not hold; `tenant_id` is the profile's cloud tenant,
/// absent for a self-hosted profile.
#[derive(Clone)]
pub struct GatewayHostHandles {
    pub services: ServicesState,
    pub governance: ConfigWatch<GovernanceEngine>,
    pub authz_hook: SharedAuthzHook,
    pub analytics: Arc<dyn AnalyticsProvider>,
    pub logging: LoggingRuntime,
    pub payload_cap_bytes: usize,
    pub quota: QuotaConfig,
    pub unapplied_plane: UnappliedPlaneMode,
    pub brain: BrainProfile,
    pub tenant_id: Option<TenantId>,
}

impl std::fmt::Debug for GatewayHostHandles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayHostHandles")
            .field("payload_cap_bytes", &self.payload_cap_bytes)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GatewaySurfaceError {
    #[error(transparent)]
    Resolve(#[from] MissingDependency),
    #[error("the gateway journal does not open")]
    Journal(#[source] crate::at_rest::GatewayAtRestError),
    #[error("the bridge release feed does not build")]
    ReleaseFeed(#[source] systemprompt_traits::BoxedSource),
}

/// The caller a gateway handler served, attached to its response for the
/// access log.
#[derive(Debug, Clone)]
pub struct GatewayLogIdentity {
    pub user: UserId,
    pub session: SessionId,
    pub trace: TraceId,
}
