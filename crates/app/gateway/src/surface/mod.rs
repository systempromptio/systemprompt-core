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
//! [`credential`] and [`authz`] are the request helpers the surface and the
//! API's remaining bridge handlers share; [`GatewayLogIdentity`] is what a
//! handler attaches to its response for the access log.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod authz;
pub mod credential;

use std::sync::Arc;

use axum::Router;
use systemprompt_extension::prelude::{ProvideError, Resolved};
use systemprompt_governance::authz::SharedAuthzHook;
use systemprompt_governance::policy::GovernanceEngine;
use systemprompt_identifiers::{SessionId, TraceId, UserId};
use systemprompt_loader::ServicesState;
use systemprompt_logging::LoggingRuntime;
use systemprompt_manifest::profile::{BrainProfile, QuotaConfig, UnappliedPlaneMode};
use systemprompt_traits::{AnalyticsProvider, ConfigWatch};

/// Builds the gateway's handler routes from the `gateway-http` route state.
pub trait GatewaySurface: Send + Sync + std::fmt::Debug {
    fn build(&self, state: GatewayRouteState<'_>) -> Result<Router, GatewaySurfaceError>;
}

pub type DynGatewaySurface = Arc<dyn GatewaySurface>;

/// The state the API mount hands a [`GatewaySurface`].
#[derive(Debug)]
pub struct GatewayRouteState<'a> {
    pub resolved: Resolved<'a>,
    pub host: GatewayHostHandles,
}

/// The live process handles and profile facts the gateway surface reads that
/// the kernel state does not hold.
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
    Resolve(#[from] ProvideError),
    #[error("the gateway journal does not open")]
    Journal(#[source] crate::at_rest::GatewayAtRestError),
}

/// The caller a gateway handler served, attached to its response for the
/// access log.
#[derive(Debug, Clone)]
pub struct GatewayLogIdentity {
    pub user: UserId,
    pub session: SessionId,
    pub trace: TraceId,
}
