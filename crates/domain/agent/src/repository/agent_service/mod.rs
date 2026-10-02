//! Repository for declared agent services (named processes registered with the
//! platform).
//!
//! The `services` table belongs to `systemprompt-database`; this adapter keeps
//! the agent vocabulary (register, mark running/stopped/error) and delegates
//! every statement to [`ServiceRepository`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_database::{DbPool, ServiceRepository, UpsertServiceProcessInput};
use systemprompt_identifiers::{AgentName, InstanceId, ServiceName};
use systemprompt_models::services::{ServiceModule, ServiceStatus};
use systemprompt_traits::RepositoryError;

#[derive(Debug)]
pub struct AgentServiceRow {
    pub name: AgentName,
    pub pid: Option<i32>,
    pub port: i32,
    pub status: ServiceStatus,
}

#[derive(Debug)]
pub struct AgentServerIdRow {
    pub name: AgentName,
}

#[derive(Debug, Clone)]
pub struct AgentServiceRepository {
    services: ServiceRepository,
}

impl AgentServiceRepository {
    pub fn new(db: &DbPool, instance_id: InstanceId) -> Self {
        let services = ServiceRepository::new(db, instance_id);
        Self { services }
    }

    pub async fn register_agent(
        &self,
        name: &AgentName,
        pid: u32,
        port: u16,
    ) -> Result<(), RepositoryError> {
        self.register_process(name, pid, port, ServiceStatus::Running)
            .await
    }

    pub async fn register_agent_starting(
        &self,
        name: &AgentName,
        pid: u32,
        port: u16,
    ) -> Result<(), RepositoryError> {
        self.register_process(name, pid, port, ServiceStatus::Starting)
            .await
    }

    async fn register_process(
        &self,
        name: &AgentName,
        pid: u32,
        port: u16,
        status: ServiceStatus,
    ) -> Result<(), RepositoryError> {
        self.remove_agent_service(name).await?;
        self.services
            .upsert_service_process(UpsertServiceProcessInput {
                name: &ServiceName::of_agent(name),
                module_name: ServiceModule::Agent,
                pid: db_pid(pid)?,
                port,
                status,
            })
            .await?;
        Ok(())
    }

    pub async fn mark_running(&self, agent_name: &AgentName) -> Result<(), RepositoryError> {
        self.services
            .update_service_status(&ServiceName::of_agent(agent_name), ServiceStatus::Running)
            .await?;
        Ok(())
    }

    pub async fn find_agent_status(
        &self,
        agent_name: &AgentName,
    ) -> Result<Option<AgentServiceRow>, RepositoryError> {
        let Some(row) = self
            .services
            .find_service_by_name(&ServiceName::of_agent(agent_name))
            .await?
        else {
            return Ok(None);
        };
        if row.module_name != ServiceModule::Agent {
            return Ok(None);
        }
        Ok(Some(AgentServiceRow {
            status: row.status,
            name: AgentName::new(row.name.as_str()),
            pid: row.pid,
            port: row.port,
        }))
    }

    pub async fn mark_stopped(&self, agent_name: &AgentName) -> Result<(), RepositoryError> {
        self.services
            .update_service_stopped(&ServiceName::of_agent(agent_name))
            .await?;
        Ok(())
    }

    pub async fn mark_error(&self, agent_name: &AgentName) -> Result<(), RepositoryError> {
        self.services
            .mark_service_crashed(&ServiceName::of_agent(agent_name))
            .await?;
        Ok(())
    }

    pub async fn list_running_agents(&self) -> Result<Vec<AgentServerIdRow>, RepositoryError> {
        let rows = self
            .services
            .list_running_services_by_module(ServiceModule::Agent)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| AgentServerIdRow {
                name: AgentName::new(r.name.as_str()),
            })
            .collect())
    }

    pub async fn remove_agent_service(
        &self,
        agent_name: &AgentName,
    ) -> Result<(), RepositoryError> {
        self.services
            .delete_service(&ServiceName::of_agent(agent_name))
            .await?;
        Ok(())
    }
}

fn db_pid(pid: u32) -> Result<i32, RepositoryError> {
    i32::try_from(pid).map_err(|_overflow| {
        RepositoryError::invalid_data("services.pid", format!("{pid} exceeds the column range"))
    })
}
