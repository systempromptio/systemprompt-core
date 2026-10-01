//! Agent process lifecycle: start, enable, disable, restart, and crash cleanup.
//!
//! [`AgentLifecycle`] owns the transitions for a single agent's worker process,
//! coordinating the database service and configured [`AppPaths`]. The free
//! functions are thin entry points that build a lifecycle for one operation;
//! the `operations` and `verification` submodules hold the spawn/teardown
//! logic and startup health-check probing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod operations;
mod verification;

use std::sync::Arc;
use systemprompt_config::paths::AppPaths;
use systemprompt_identifiers::AgentName;
use systemprompt_traits::StartupEventSender;

use crate::repository::agent_service::AgentServiceRepository;
use crate::services::agent_orchestration::OrchestrationResult;
use crate::services::agent_orchestration::database::AgentDatabaseService;

#[derive(Debug)]
pub struct AgentLifecycle {
    pub(crate) db_service: AgentDatabaseService,
    pub(crate) app_paths: Arc<AppPaths>,
}

impl AgentLifecycle {
    pub fn new(
        agent_service_repo: AgentServiceRepository,
        app_paths: Arc<AppPaths>,
    ) -> OrchestrationResult<Self> {
        let db_service = AgentDatabaseService::new(agent_service_repo)?;

        Ok(Self {
            db_service,
            app_paths,
        })
    }
}

pub async fn start_agent(
    agent_service_repo: AgentServiceRepository,
    app_paths: Arc<AppPaths>,
    agent_name: &AgentName,
    events: Option<&StartupEventSender>,
) -> OrchestrationResult<String> {
    let lifecycle = AgentLifecycle::new(agent_service_repo, app_paths)?;
    lifecycle.start_agent(agent_name, events).await
}

pub async fn enable_agent(
    agent_service_repo: AgentServiceRepository,
    app_paths: Arc<AppPaths>,
    agent_name: &AgentName,
    events: Option<&StartupEventSender>,
) -> OrchestrationResult<String> {
    let lifecycle = AgentLifecycle::new(agent_service_repo, app_paths)?;
    lifecycle.enable_agent(agent_name, events).await
}

pub async fn disable_agent(
    agent_service_repo: AgentServiceRepository,
    app_paths: Arc<AppPaths>,
    agent_name: &AgentName,
) -> OrchestrationResult<()> {
    let lifecycle = AgentLifecycle::new(agent_service_repo, app_paths)?;
    lifecycle.disable_agent(agent_name).await
}

pub async fn restart_agent(
    agent_service_repo: AgentServiceRepository,
    app_paths: Arc<AppPaths>,
    agent_name: &AgentName,
    events: Option<&StartupEventSender>,
) -> OrchestrationResult<String> {
    let lifecycle = AgentLifecycle::new(agent_service_repo, app_paths)?;
    lifecycle.restart_agent(agent_name, events).await
}
