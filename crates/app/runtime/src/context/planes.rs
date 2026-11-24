//! The four planes an [`AppContext`](super::AppContext) groups its handles
//! into: data access, configuration, plugins and runtime subsystems.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_config::paths::AppPaths;
use systemprompt_database::{CoordinationRepositories, DbPool, ServiceRepository};
use systemprompt_events::{EventBridgeHandle, EventRouter};
use systemprompt_governance::authz::SharedAuthzHook;
use systemprompt_governance::policy::GovernanceEngine;
use systemprompt_loader::BootContext;
use systemprompt_logging::LoggingRuntime;
use systemprompt_manifest::Config;
use systemprompt_manifest::services::SystemAdmin;
use systemprompt_models::RouteClassifier;
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::{BackgroundTasks, CacheEpochs, ConfigWatch, FileStorage};

use super::ShutdownRequest;

/// The four planes an [`AppContext`](super::AppContext) holds.
#[derive(Debug, Clone)]
pub struct ContextPlanes {
    pub data: DataPlane,
    pub cfg: ConfigPlane,
    pub plugins: Plugins,
    pub subsystems: Subsystems,
}

/// Database pool and the platform repositories layered on it.
#[derive(Clone)]
pub struct DataPlane {
    pub database: DbPool,
    pub service_repository: Arc<ServiceRepository>,
    pub coordination: Arc<CoordinationRepositories>,
}

#[derive(Clone)]
pub struct ConfigPlane {
    pub boot: BootContext,
    pub config: Arc<Config>,
    pub app_paths: Arc<AppPaths>,
    pub route_classifier: Arc<RouteClassifier>,
    pub build: systemprompt_models::BuildIdentity,
}

#[derive(Clone)]
pub struct Plugins {
    pub subject_providers: systemprompt_governance::authz::SubjectProviderSet,
    pub subjects: systemprompt_models::authz::SubjectSourceHandle,
}

#[derive(Clone)]
pub struct Subsystems {
    pub system_admin: Arc<SystemAdmin>,
    pub logging: LoggingRuntime,
    pub authz_hook: SharedAuthzHook,
    pub signing_keys: SigningKeys,
    pub token_authority: Arc<dyn systemprompt_models::auth::TokenAuthority>,
    pub governance: ConfigWatch<GovernanceEngine>,
    pub services: systemprompt_loader::ServicesState,
    pub cache_epochs: CacheEpochs,
    pub schema_install: Arc<crate::SchemaState>,
    pub event_bridge: Arc<EventBridgeHandle>,
    pub event_router: EventRouter,
    pub file_storage: Arc<dyn FileStorage>,
    pub shutdown: ShutdownRequest,
    pub background_tasks: BackgroundTasks,
    pub spawner: Arc<systemprompt_loader::subprocess::Spawner>,
    pub late_binders: crate::builder::kernel_binders::PendingBinders,
}
