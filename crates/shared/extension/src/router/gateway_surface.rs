//! The gateway contracts the API composes without naming the gateway
//! capability.
//!
//! The `gateway-http` member contributes a [`GatewaySurface`]: one builder
//! the API mount calls with the route-scoped state of `gateway-http`
//! ([`GatewayRouteState`]). The member owns the handlers; the API owns the
//! layering around what the builder returns — the confined 404 outside the
//! built-in gateway prefixes, the rate scopes and bearer windows. The route
//! state is the kernel resolution scoped to `gateway-http` with core
//! standing: the open state, the sealed values `gateway-http` provided itself
//! (its repositories and the gateway host handles it builds from the host
//! seeds in its own provide) and the sealed values conferred on it, and
//! nothing else sealed.
//!
//! The `gateway` member provides, sealed, the one [`GatewayOperations`] of
//! the process: the access-log layer the API wraps the whole gateway surface
//! in (outside every limiter, so a refused request is still recorded), the
//! settlement of accounting receipts a previous process left unsettled, and
//! the in-process model client the brain's capture consumer extracts through.
//! Its presence is the API's one test of whether the gateway is composed: a
//! node without it builds no gateway route and runs no gateway boot step, and
//! a brain that needs it refuses to start. Kernel code calls it with the
//! kernel's own resolution, the same authority the gateway's boot work read
//! before it moved behind this contract.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;
use systemprompt_traits::BoxedSource;

use super::DynConsumerCatalog;
use crate::brain::BrainModelClient;
use crate::{MissingDependency, Resolved};

/// Builds the gateway's handler routes from the `gateway-http` route state.
pub trait GatewaySurface: Send + Sync + std::fmt::Debug {
    fn build(
        &self,
        state: GatewayRouteState<'_>,
    ) -> Result<GatewaySurfaceRoutes, GatewaySurfaceError>;
}

pub type DynGatewaySurface = Arc<dyn GatewaySurface>;

/// The state the API mount hands a [`GatewaySurface`].
#[derive(Debug)]
pub struct GatewayRouteState<'a> {
    pub resolved: Resolved<'a>,
}

/// The routes a [`GatewaySurface`] builds.
///
/// `traffic` verifies its own callers; the API mount places `principal_bound`
/// behind the kernel's gateway verifier, which attaches the caller's
/// [`GatewayPrincipal`](super::GatewayPrincipal) before a handler runs.
/// `public_sessions` is the API-key session mint the mount places under the
/// public gateway base with its own rate scope. `consumer_catalog` is the
/// bridge manifest's granted-skill resolution the `marketplace-http`
/// consumer routes read; absent when no bridge route is built, and those
/// routes are then not mounted.
#[derive(Default)]
pub struct GatewaySurfaceRoutes {
    pub traffic: Router,
    pub principal_bound: Router,
    pub public_sessions: Router,
    pub consumer_catalog: Option<DynConsumerCatalog>,
}

impl std::fmt::Debug for GatewaySurfaceRoutes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewaySurfaceRoutes")
            .field("consumer_catalog", &self.consumer_catalog.is_some())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GatewaySurfaceError {
    #[error(transparent)]
    Resolve(#[from] MissingDependency),
    #[error("this host does not seed the gateway host handles")]
    HostUnseeded,
    #[error("the gateway journal does not open")]
    Journal(#[source] BoxedSource),
    #[error("the bridge release feed does not build")]
    ReleaseFeed(#[source] BoxedSource),
}

/// The `gateway` member's kernel-facing work: the access-log layer, the
/// accounting recovery boot step and the brain's model client.
///
/// Held as `Arc<dyn GatewayOperations>` in the kernel state, hence
/// `#[async_trait]`.
#[async_trait::async_trait]
pub trait GatewayOperations: Send + Sync + std::fmt::Debug {
    fn access_log(
        &self,
        kernel: &Resolved<'_>,
        router: Router,
    ) -> Result<Router, GatewayOperationsError>;

    async fn recover_accounting(
        &self,
        kernel: &Resolved<'_>,
    ) -> Result<usize, GatewayOperationsError>;

    async fn brain_model_client(
        &self,
        kernel: &Resolved<'_>,
    ) -> Result<Arc<dyn BrainModelClient>, GatewayOperationsError>;
}

pub type DynGatewayOperations = Arc<dyn GatewayOperations>;

#[derive(Debug, thiserror::Error)]
pub enum GatewayOperationsError {
    #[error(transparent)]
    Resolve(#[from] MissingDependency),
    #[error(transparent)]
    Surface(#[from] GatewaySurfaceError),
    #[error("the gateway is disabled in the services configuration")]
    Disabled,
    #[error("the gateway {operation} failed")]
    Failed {
        operation: &'static str,
        #[source]
        source: BoxedSource,
    },
}
