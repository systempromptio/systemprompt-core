//! The gateway's side of the HTTP surface contract.
//!
//! The contract itself
//! ([`GatewaySurface`](systemprompt_extension::router::GatewaySurface)
//! and its route state, routes and error) lives in `systemprompt-extension`
//! so the API mounts it without naming this crate. What stays here is what
//! the `gateway-http` handlers and the `gateway` member's kernel operations
//! share: [`GatewayHostHandles`], the live process handles and profile facts
//! the gateway reads, built from the host seeds by a core member's provide fn
//! or by kernel code ([`GatewayHostHandles::resolve`]), never by a route
//! state; [`GatewayLogIdentity`], what a handler attaches to its response for
//! the access log; the request helpers ([`credential`], [`authz`]); and the
//! narrow [`views`] of sealed kernel seeds the `gateway` member confers on
//! `gateway-http` for the bridge routes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod authz;
pub mod credential;
pub mod repositories;
pub mod views;

use std::sync::Arc;

use systemprompt_extension::prelude::{MissingDependency, Resolved};
use systemprompt_governance::authz::SharedAuthzHook;
use systemprompt_governance::policy::GovernanceEngine;
use systemprompt_identifiers::{SessionId, TenantId, TraceId, UserId};
use systemprompt_loader::{BootContext, ServicesState};
use systemprompt_logging::LoggingRuntime;
use systemprompt_manifest::profile::{BrainProfile, QuotaConfig, UnappliedPlaneMode};
use systemprompt_traits::{AnalyticsProvider, ConfigWatch};

/// The live process handles and profile facts the gateway reads; `tenant_id`
/// is the profile's cloud tenant, absent for a self-hosted profile.
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

impl GatewayHostHandles {
    pub fn resolve(r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        let boot = r.get::<BootContext>()?;
        let profile = boot.profile();
        Ok(Self {
            services: r.get::<ServicesState>()?,
            governance: r.get::<ConfigWatch<GovernanceEngine>>()?,
            authz_hook: r.get::<SharedAuthzHook>()?,
            analytics: r.get::<Arc<dyn AnalyticsProvider>>()?,
            logging: r.get::<LoggingRuntime>()?,
            payload_cap_bytes: profile.payload_cap_bytes(),
            quota: profile.quota_config(),
            unapplied_plane: profile.unapplied_plane_mode(),
            brain: profile.brain.clone(),
            tenant_id: profile
                .cloud
                .as_ref()
                .and_then(|cloud| cloud.tenant_id.clone()),
        })
    }
}

impl std::fmt::Debug for GatewayHostHandles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayHostHandles")
            .field("payload_cap_bytes", &self.payload_cap_bytes)
            .finish_non_exhaustive()
    }
}

/// The caller a gateway handler served, attached to its response for the
/// access log.
#[derive(Debug, Clone)]
pub struct GatewayLogIdentity {
    pub user: UserId,
    pub session: SessionId,
    pub trace: TraceId,
}
