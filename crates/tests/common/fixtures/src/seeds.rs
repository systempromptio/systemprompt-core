//! Typed reads of an `AppContext`'s host seeds for tests: each method reads the
//! seed from the kernel state and panics when a test context lacks it.

use std::sync::Arc;

use systemprompt_config::paths::AppPaths;
use systemprompt_database::{CoordinationRepositories, DbPool, ServiceRepository};
use systemprompt_events::{EventBridgeHandle, EventBroadcasters, EventRouter};
use systemprompt_governance::authz::SharedAuthzHook;
use systemprompt_governance::policy::GovernanceEngine;
use systemprompt_loader::subprocess::Spawner;
use systemprompt_loader::{BootContext, ServicesState};
use systemprompt_logging::LoggingRuntime;
use systemprompt_manifest::services::{ServicesConfig, SystemAdmin};
use systemprompt_manifest::{Config, Profile};
use systemprompt_models::auth::TokenAuthority;
use systemprompt_models::{BuildIdentity, RouteClassifier};
use systemprompt_runtime::{AppContext, PendingBinders, SchemaState, ShutdownRequest};
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::{BackgroundTasks, CacheEpochs, ConfigWatch, FileStorage};

pub trait AppContextSeeds {
    fn seed<T: std::any::Any + Send + Sync>(&self) -> &T;

    fn config(&self) -> &Arc<Config> {
        self.seed()
    }
    fn config_arc(&self) -> &Arc<Config> {
        self.seed()
    }
    fn db_pool(&self) -> &DbPool {
        self.seed()
    }
    fn app_paths(&self) -> &Arc<AppPaths> {
        self.seed()
    }
    fn app_paths_arc(&self) -> &Arc<AppPaths> {
        self.seed()
    }
    fn file_storage(&self) -> &Arc<dyn FileStorage> {
        self.seed()
    }
    fn event_bridge(&self) -> &Arc<EventBridgeHandle> {
        self.seed()
    }
    fn event_router(&self) -> &Arc<EventRouter> {
        self.seed()
    }
    fn event_broadcasters(&self) -> &Arc<EventBroadcasters> {
        self.seed::<Arc<EventRouter>>().broadcasters()
    }
    fn authz_hook(&self) -> &SharedAuthzHook {
        self.seed()
    }
    fn signing_keys(&self) -> &SigningKeys {
        self.seed()
    }
    fn schema_state(&self) -> &Arc<SchemaState> {
        self.seed()
    }
    fn service_repository(&self) -> &Arc<ServiceRepository> {
        self.seed()
    }
    fn coordination_repositories(&self) -> &Arc<CoordinationRepositories> {
        self.seed()
    }
    fn route_classifier(&self) -> &Arc<RouteClassifier> {
        self.seed()
    }
    fn build_identity(&self) -> &BuildIdentity {
        self.seed()
    }
    fn boot(&self) -> &BootContext {
        self.seed()
    }
    fn profile(&self) -> &Profile {
        self.boot().profile()
    }
    fn secrets(&self) -> &Arc<systemprompt_config::SecretsService> {
        self.boot().secrets()
    }
    fn token_authority(&self) -> Arc<dyn TokenAuthority> {
        Arc::clone(self.seed::<Arc<dyn TokenAuthority>>())
    }
    fn system_admin(&self) -> &SystemAdmin {
        self.seed()
    }
    fn logging(&self) -> &LoggingRuntime {
        self.seed()
    }
    fn shutdown_request(&self) -> &ShutdownRequest {
        self.seed()
    }
    fn background_tasks(&self) -> &BackgroundTasks {
        self.seed()
    }
    fn spawner(&self) -> &Arc<Spawner> {
        self.seed()
    }
    fn governance_watch(&self) -> &ConfigWatch<GovernanceEngine> {
        self.seed()
    }
    fn governance(&self) -> Arc<GovernanceEngine> {
        self.governance_watch().get()
    }
    fn services_state(&self) -> &ServicesState {
        self.seed()
    }
    fn services_config(&self) -> Arc<ServicesConfig> {
        self.services_state().config()
    }
    fn services_watch(&self) -> &ConfigWatch<ServicesConfig> {
        self.services_state().watch()
    }
    fn cache_epochs(&self) -> &CacheEpochs {
        self.seed()
    }
    fn take_late_binders(&self) -> Option<systemprompt_runtime::ApiLateBinders> {
        self.seed::<PendingBinders>().take()
    }
}

impl AppContextSeeds for AppContext {
    fn seed<T: std::any::Any + Send + Sync>(&self) -> &T {
        self.kernel()
            .state()
            .get::<T>()
            .unwrap_or_else(|missing| panic!("test context lacks a host seed: {missing}"))
    }
}
