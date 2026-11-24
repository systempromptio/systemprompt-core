//! [`AgentStatusView`]: the open, status-only view of the agent processes.
//!
//! It reads the agent service rows and the configured agents; it holds no
//! spawner, no secret and no control, so the agent member provides it open
//! for `admin agents status` and for the kernel's process set.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ServiceName;
use systemprompt_manifest::services::ServicesView;
use systemprompt_traits::{ServiceDetail, ServiceHealth, ServiceProcessStatus};

use super::database::AgentDatabaseService;
use super::{AgentStatus, OrchestrationResult};
use crate::repository::agent_service::AgentServiceRepository;
use crate::services::registry::AgentRegistry;

pub struct AgentStatusView {
    database: AgentDatabaseService,
    services: ServicesView,
}

impl std::fmt::Debug for AgentStatusView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentStatusView").finish_non_exhaustive()
    }
}

impl AgentStatusView {
    #[must_use]
    pub fn new(repository: AgentServiceRepository, services: ServicesView) -> Self {
        Self {
            database: AgentDatabaseService::new(repository, &services),
            services,
        }
    }

    pub async fn statuses(&self) -> OrchestrationResult<Vec<ServiceProcessStatus>> {
        let registry = AgentRegistry::new(&self.services);
        let agents = self.database.list_all_agents().await?;
        Ok(agents
            .into_iter()
            .filter_map(|(name, status)| {
                let config = registry.get_agent(name.as_str()).ok()?;
                let (pid, port, health, failure) = match status {
                    AgentStatus::Running { pid, port, .. } => {
                        (Some(pid), Some(port), ServiceHealth::Healthy, None)
                    },
                    AgentStatus::Failed { reason, .. } => {
                        (None, None, ServiceHealth::Unhealthy, Some(reason))
                    },
                };
                Some(ServiceProcessStatus {
                    name: ServiceName::new(name.as_str()),
                    pid,
                    port,
                    health,
                    detail: ServiceDetail::Agent {
                        enabled: config.enabled,
                        failure,
                    },
                })
            })
            .collect())
    }
}
