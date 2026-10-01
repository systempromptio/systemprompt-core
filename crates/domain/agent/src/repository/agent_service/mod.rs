//! Repository for declared agent services (named processes registered with the
//! platform).
//!
//! The `services` table belongs to `systemprompt-database`; this adapter keeps
//! the agent vocabulary (register, mark running/stopped/error) and delegates
//! every statement to [`ServiceRepository`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod status;

use systemprompt_database::{DbPool, ServiceRepository, UpsertServiceProcessInput};
use systemprompt_identifiers::InstanceId;
use systemprompt_traits::RepositoryError;

use crate::error::AgentError;

pub use status::AgentServiceStatus;

const AGENT_MODULE: &str = "agent";

#[derive(Debug)]
pub struct AgentServiceRow {
    pub name: String,
    pub pid: Option<i32>,
    pub port: i32,
    pub status: AgentServiceStatus,
}

#[derive(Debug)]
pub struct AgentServerIdRow {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct AgentServiceRepository {
    services: ServiceRepository,
}

impl AgentServiceRepository {
    pub fn new(db: &DbPool, instance_id: InstanceId) -> Result<Self, AgentError> {
        let services =
            ServiceRepository::new(db, instance_id).map_err(|e| AgentError::Init(e.to_string()))?;
        Ok(Self { services })
    }

    pub async fn register_agent(
        &self,
        name: &str,
        pid: u32,
        port: u16,
    ) -> Result<(), RepositoryError> {
        self.register_process(name, pid, port, AgentServiceStatus::Running)
            .await
    }

    pub async fn register_agent_starting(
        &self,
        name: &str,
        pid: u32,
        port: u16,
    ) -> Result<(), RepositoryError> {
        self.register_process(name, pid, port, AgentServiceStatus::Starting)
            .await
    }

    async fn register_process(
        &self,
        name: &str,
        pid: u32,
        port: u16,
        status: AgentServiceStatus,
    ) -> Result<(), RepositoryError> {
        self.remove_agent_service(name).await?;
        self.services
            .upsert_service_process(UpsertServiceProcessInput {
                name,
                module_name: AGENT_MODULE,
                pid: db_pid(pid)?,
                port,
                status: status.as_str(),
            })
            .await?;
        Ok(())
    }

    pub async fn mark_running(&self, agent_name: &str) -> Result<(), RepositoryError> {
        self.services
            .update_service_status(agent_name, AgentServiceStatus::Running.as_str())
            .await?;
        Ok(())
    }

    pub async fn get_agent_status(
        &self,
        agent_name: &str,
    ) -> Result<Option<AgentServiceRow>, RepositoryError> {
        let Some(row) = self.services.find_service_by_name(agent_name).await? else {
            return Ok(None);
        };
        if row.module_name != AGENT_MODULE {
            return Ok(None);
        }
        Ok(Some(AgentServiceRow {
            status: AgentServiceStatus::parse(&row.status)?,
            name: row.name,
            pid: row.pid,
            port: row.port,
        }))
    }

    pub async fn mark_stopped(&self, agent_name: &str) -> Result<(), RepositoryError> {
        self.services.update_service_stopped(agent_name).await?;
        Ok(())
    }

    pub async fn mark_error(&self, agent_name: &str) -> Result<(), RepositoryError> {
        self.services.mark_service_crashed(agent_name).await?;
        Ok(())
    }

    pub async fn list_running_agents(&self) -> Result<Vec<AgentServerIdRow>, RepositoryError> {
        let rows = self
            .services
            .list_running_services_by_module(AGENT_MODULE)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| AgentServerIdRow { name: r.name })
            .collect())
    }

    pub async fn remove_agent_service(&self, agent_name: &str) -> Result<(), RepositoryError> {
        self.services.delete_service(agent_name).await?;
        Ok(())
    }
}

fn db_pid(pid: u32) -> Result<i32, RepositoryError> {
    i32::try_from(pid).map_err(|_overflow| {
        RepositoryError::InvalidData(format!("pid {pid} exceeds the services.pid column"))
    })
}
