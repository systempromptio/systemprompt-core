//! Agent process lifecycle: start, enable, disable, restart, and crash cleanup.
//!
//! [`AgentLifecycle`] owns the transitions for a single agent's worker process,
//! coordinating the database service and configured [`AppPaths`]. The
//! `operations` and `verification` submodules hold the spawn/teardown logic and
//! startup health-check probing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod operations;
mod verification;

use std::sync::Arc;
use systemprompt_config::paths::AppPaths;
use systemprompt_loader::ServicesState;
use systemprompt_loader::subprocess::Spawner;

use crate::repository::agent_service::AgentServiceRepository;
use crate::services::agent_orchestration::database::AgentDatabaseService;

#[derive(Debug)]
pub struct AgentLifecycle {
    pub(crate) db_service: AgentDatabaseService,
    pub(crate) app_paths: Arc<AppPaths>,
    pub(crate) spawner: Arc<Spawner>,
}

impl AgentLifecycle {
    pub fn new(
        agent_service_repo: AgentServiceRepository,
        app_paths: Arc<AppPaths>,
        spawner: Arc<Spawner>,
        services: &ServicesState,
    ) -> Self {
        Self {
            db_service: AgentDatabaseService::new(agent_service_repo, services),
            app_paths,
            spawner,
        }
    }
}
