//! The handles an [`AppContext`](super::AppContext) adds to the kernel state
//! when it is built: the values only the context assembly produces. Every
//! other host value is a kernel seed already ([`crate::kernel_state`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::{CoordinationRepositories, ServiceRepository};
use systemprompt_events::EventBridgeHandle;
use systemprompt_governance::authz::SharedAuthzHook;
use systemprompt_governance::policy::GovernanceEngine;
use systemprompt_loader::BootContext;
use systemprompt_logging::LoggingRuntime;
use systemprompt_models::RouteClassifier;
use systemprompt_traits::ConfigWatch;

use super::ShutdownRequest;

/// The four planes [`AppContext::from_parts`](super::AppContext::from_parts)
/// seals into the kernel state.
#[derive(Debug, Clone)]
pub struct ContextPlanes {
    pub data: DataPlane,
    pub cfg: ConfigPlane,
    pub plugins: Plugins,
    pub subsystems: Subsystems,
}

/// The platform repositories the lifecycle code reads.
#[derive(Clone)]
pub struct DataPlane {
    pub service_repository: Arc<ServiceRepository>,
    pub coordination: Arc<CoordinationRepositories>,
}

#[derive(Clone)]
pub struct ConfigPlane {
    pub boot: BootContext,
    pub route_classifier: Arc<RouteClassifier>,
    pub build: systemprompt_models::BuildIdentity,
}

#[derive(Clone)]
pub struct Plugins {
    pub subject_providers: systemprompt_governance::authz::SubjectProviderSet,
}

#[derive(Clone)]
pub struct Subsystems {
    pub logging: LoggingRuntime,
    pub authz_hook: SharedAuthzHook,
    pub governance: ConfigWatch<GovernanceEngine>,
    pub services: systemprompt_loader::ServicesState,
    pub schema_install: Arc<crate::SchemaState>,
    pub event_bridge: Arc<EventBridgeHandle>,
    pub shutdown: ShutdownRequest,
    pub late_binders: crate::builder::kernel_binders::PendingBinders,
}
