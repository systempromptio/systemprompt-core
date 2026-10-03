//! Database-backed view of agent service state, reconciled against live
//! processes.
//!
//! [`AgentDatabaseService`] wraps the agent-service repository and the
//! config-driven [`AgentRegistry`], translating stored rows into
//! [`AgentStatus`] while verifying that recorded PIDs still correspond to
//! running processes. It is the single source of truth the lifecycle, monitor,
//! and reconciler services query and mutate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::AgentError;
use crate::repository::agent_service::AgentServiceRepository;
use crate::services::agent_orchestration::{
    AgentStatus, OrchestrationError, OrchestrationResult, process,
};
use crate::services::registry::AgentRegistry;
use systemprompt_identifiers::AgentName;
use systemprompt_manifest::services::{AgentConfig, ServiceStatus};
use systemprompt_traits::RepositoryError;

#[derive(Debug)]
pub struct AgentDatabaseService {
    pub repository: AgentServiceRepository,
    pub registry: AgentRegistry,
}

impl AgentDatabaseService {
    pub fn new(repository: AgentServiceRepository) -> OrchestrationResult<Self> {
        let registry = AgentRegistry::new().map_err(OrchestrationError::Registry)?;

        Ok(Self {
            repository,
            registry,
        })
    }

    #[must_use]
    pub const fn with_registry(
        repository: AgentServiceRepository,
        registry: AgentRegistry,
    ) -> Self {
        Self {
            repository,
            registry,
        }
    }

    pub async fn register_agent(
        &self,
        name: &AgentName,
        pid: u32,
        port: u16,
    ) -> OrchestrationResult<()> {
        self.repository
            .register_agent(name, pid, port)
            .await
            .map_err(OrchestrationError::from)
    }

    pub async fn get_status(&self, agent_name: &AgentName) -> OrchestrationResult<AgentStatus> {
        let row = self
            .repository
            .find_agent_status(agent_name)
            .await
            .map_err(OrchestrationError::from)?;

        let Some(row) = row else {
            return Ok(failed_status("No service record found"));
        };

        match row.status {
            ServiceStatus::Running => {
                let Some(pid) = row.pid else {
                    self.mark_failed(agent_name).await?;
                    return Ok(failed_status("Running row has no recorded process id"));
                };
                let (pid, port) = stored_process(pid, row.port)?;
                if process::process_exists(pid) {
                    Ok(AgentStatus::Running { pid, port })
                } else {
                    self.mark_failed(agent_name).await?;
                    Ok(failed_status("Process died unexpectedly"))
                }
            },
            ServiceStatus::Starting => Ok(failed_status("Agent is starting")),
            ServiceStatus::Stopping => Ok(failed_status("Agent is stopping")),
            ServiceStatus::Stopped => Ok(failed_status("Agent is stopped")),
            ServiceStatus::Error => Ok(failed_status("Agent process failed")),
        }
    }

    pub async fn mark_failed(&self, agent_name: &AgentName) -> OrchestrationResult<()> {
        self.repository
            .mark_error(agent_name)
            .await
            .map_err(OrchestrationError::from)
    }

    pub async fn list_running_agents(&self) -> OrchestrationResult<Vec<AgentName>> {
        let rows = self
            .repository
            .list_running_agents()
            .await
            .map_err(OrchestrationError::from)?;

        Ok(rows.into_iter().map(|row| row.name).collect())
    }

    pub async fn list_all_agents(&self) -> OrchestrationResult<Vec<(AgentName, AgentStatus)>> {
        let agent_configs = self.registry.list_agents().await?;

        let mut agents = Vec::new();

        for agent_config in agent_configs {
            let agent_name = AgentName::new(agent_config.name);

            let status = self.get_status(&agent_name).await?;

            agents.push((agent_name, status));
        }

        Ok(agents)
    }

    pub async fn agent_exists(&self, agent_name: &AgentName) -> OrchestrationResult<bool> {
        match self.registry.get_agent(agent_name.as_str()).await {
            Ok(_) => Ok(true),
            Err(AgentError::NotFound(_)) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn get_agent_config(
        &self,
        agent_name: &AgentName,
    ) -> OrchestrationResult<AgentConfig> {
        match self.registry.get_agent(agent_name.as_str()).await {
            Ok(agent_config) => Ok(agent_config),
            Err(AgentError::NotFound(_)) => {
                Err(OrchestrationError::AgentNotFound(agent_name.to_string()))
            },
            Err(e) => Err(e.into()),
        }
    }

    pub async fn remove_agent_service(&self, agent_name: &AgentName) -> OrchestrationResult<()> {
        self.repository
            .remove_agent_service(agent_name)
            .await
            .map_err(OrchestrationError::from)
    }

    pub async fn update_agent_running(
        &self,
        agent_name: &AgentName,
        pid: u32,
        port: u16,
    ) -> OrchestrationResult<()> {
        self.repository
            .register_agent(agent_name, pid, port)
            .await
            .map_err(OrchestrationError::from)
    }

    pub async fn update_agent_stopped(&self, agent_name: &AgentName) -> OrchestrationResult<()> {
        self.repository
            .mark_stopped(agent_name)
            .await
            .map_err(OrchestrationError::from)
    }

    pub async fn register_agent_starting(
        &self,
        agent_name: &AgentName,
        pid: u32,
        port: u16,
    ) -> OrchestrationResult<()> {
        self.repository
            .register_agent_starting(agent_name, pid, port)
            .await
            .map_err(OrchestrationError::from)
    }

    pub async fn mark_running(&self, agent_name: &AgentName) -> OrchestrationResult<()> {
        self.repository
            .mark_running(agent_name)
            .await
            .map_err(OrchestrationError::from)
    }
}

fn failed_status(reason: &str) -> AgentStatus {
    AgentStatus::Failed {
        reason: reason.to_owned(),
        last_attempt: None,
        retry_count: 0,
    }
}

fn stored_process(pid: i32, port: i32) -> OrchestrationResult<(u32, u16)> {
    let pid = u32::try_from(pid)
        .map_err(|e| RepositoryError::decode(format!("stored pid {pid} is not a process id"), e))?;
    let port = u16::try_from(port)
        .map_err(|e| RepositoryError::decode(format!("stored port {port} is not a TCP port"), e))?;
    Ok((pid, port))
}
