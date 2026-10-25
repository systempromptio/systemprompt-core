//! Service accessors: the process-wide governance engine, the services
//! config every request reads, AI service,
//! artifact ingest, logging runtime and lifecycle owners the composition root
//! built once.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_governance::policy::{GovernanceEngine, GovernanceEngineError};
use systemprompt_logging::LoggingRuntime;
use systemprompt_manifest::services::{ServicesConfig, SystemAdmin};
use systemprompt_traits::{BackgroundTasks, CacheEpochs, ConfigWatch};

use super::{AppContext, ShutdownRequest};

impl AppContext {
    pub fn system_admin(&self) -> &SystemAdmin {
        &self.subsystems.system_admin
    }

    pub const fn logging(&self) -> &LoggingRuntime {
        &self.subsystems.logging
    }

    pub const fn shutdown_request(&self) -> &ShutdownRequest {
        &self.subsystems.shutdown
    }

    pub const fn background_tasks(&self) -> &BackgroundTasks {
        &self.subsystems.background_tasks
    }

    pub const fn spawner(&self) -> &Arc<systemprompt_loader::subprocess::Spawner> {
        &self.subsystems.spawner
    }

    pub fn request_restart(&self, reason: &str) {
        self.subsystems.shutdown.request(reason);
    }

    pub fn config_planes(
        &self,
    ) -> systemprompt_provider_contracts::ConfigPlaneResult<crate::ConfigPlaneSet> {
        crate::ConfigPlaneSet::new(
            self.extension_registry().all_config_planes(),
            self.config().planes.authority.clone(),
            std::path::PathBuf::from(&self.config().services_path),
        )
    }

    #[must_use]
    pub fn governance(&self) -> Arc<GovernanceEngine> {
        self.subsystems.governance.get()
    }

    #[must_use]
    pub const fn governance_watch(&self) -> &ConfigWatch<GovernanceEngine> {
        &self.subsystems.governance
    }

    #[must_use]
    pub fn services_config(&self) -> Arc<ServicesConfig> {
        self.subsystems.services.config()
    }

    #[must_use]
    pub const fn services_state(&self) -> &systemprompt_loader::ServicesState {
        &self.subsystems.services
    }

    #[must_use]
    pub fn authz_services(&self) -> systemprompt_governance::authz::AuthzServices {
        crate::builder::core_layer::authz_services(self.services_state())
    }

    #[must_use]
    pub const fn services_watch(&self) -> &ConfigWatch<ServicesConfig> {
        self.subsystems.services.watch()
    }

    pub async fn load_governance(&self) -> Result<GovernanceEngine, GovernanceEngineError> {
        crate::builder::load_governance(self.db_pool()).await
    }

    #[must_use]
    pub const fn cache_epochs(&self) -> &CacheEpochs {
        &self.subsystems.cache_epochs
    }

    #[must_use]
    pub fn artifact_ingest(&self) -> &systemprompt_mcp::ArtifactIngest {
        &self.subsystems.artifact_ingest
    }

    #[must_use]
    pub fn artifact_ingest_arc(&self) -> Arc<systemprompt_mcp::ArtifactIngest> {
        Arc::clone(&self.subsystems.artifact_ingest)
    }

    #[must_use]
    pub const fn ai_service(&self) -> Option<&Arc<systemprompt_ai::AiService>> {
        self.subsystems.ai_service.as_ref()
    }

    #[must_use]
    pub fn ai_service_arc(&self) -> Option<Arc<systemprompt_ai::AiService>> {
        self.subsystems.ai_service.clone()
    }
}
