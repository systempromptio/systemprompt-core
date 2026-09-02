//! Repository for declared agent services (named processes registered with the
//! platform).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::InstanceId;
use systemprompt_traits::RepositoryError;

use crate::error::AgentError;

#[derive(Debug)]
pub struct AgentServiceRow {
    pub name: String,
    pub pid: Option<i32>,
    pub port: i32,
    pub status: String,
}

#[derive(Debug)]
pub struct AgentServerIdRow {
    pub name: String,
}

#[derive(Debug)]
pub struct AgentServerIdPidRow {
    pub name: String,
    pub pid: i32,
}

#[derive(Debug, Clone)]
pub struct AgentServiceRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
    instance_id: InstanceId,
}

impl AgentServiceRepository {
    pub fn new(db: &DbPool, instance_id: InstanceId) -> Result<Self, AgentError> {
        let pool = db.pool_arc().map_err(|e| AgentError::Init(e.to_string()))?;
        let write_pool = db
            .write_pool_arc()
            .map_err(|e| AgentError::Init(e.to_string()))?;
        Ok(Self {
            pool,
            write_pool,
            instance_id,
        })
    }

    pub async fn register_agent(
        &self,
        name: &str,
        pid: u32,
        port: u16,
    ) -> Result<String, RepositoryError> {
        self.remove_agent_service(name).await?;

        let pool = &self.write_pool;
        let pid_i32 = pid as i32;
        let port_i32 = i32::from(port);

        sqlx::query!(
            "INSERT INTO services (instance_id, name, module_name, pid, port, status, updated_at)
             VALUES ($4, $1, 'agent', $2, $3, 'running', CURRENT_TIMESTAMP)
             ON CONFLICT (instance_id, name) DO UPDATE SET pid = $2, port = $3, status = \
             'running', heartbeat_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP",
            name,
            pid_i32,
            port_i32,
            self.instance_id.as_str()
        )
        .execute(pool.as_ref())
        .await
        .map_err(RepositoryError::database)?;

        Ok(name.to_owned())
    }

    pub async fn register_agent_starting(
        &self,
        name: &str,
        pid: u32,
        port: u16,
    ) -> Result<String, RepositoryError> {
        self.remove_agent_service(name).await?;

        let pool = &self.write_pool;
        let pid_i32 = pid as i32;
        let port_i32 = i32::from(port);

        sqlx::query!(
            "INSERT INTO services (instance_id, name, module_name, pid, port, status, updated_at)
             VALUES ($4, $1, 'agent', $2, $3, 'starting', CURRENT_TIMESTAMP)
             ON CONFLICT (instance_id, name) DO UPDATE SET pid = $2, port = $3, status = \
             'starting', heartbeat_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP",
            name,
            pid_i32,
            port_i32,
            self.instance_id.as_str()
        )
        .execute(pool.as_ref())
        .await
        .map_err(RepositoryError::database)?;

        Ok(name.to_owned())
    }

    pub async fn mark_running(&self, agent_name: &str) -> Result<(), RepositoryError> {
        let pool = &self.write_pool;

        sqlx::query!(
            "UPDATE services SET status = 'running', updated_at = CURRENT_TIMESTAMP WHERE instance_id = $2 AND name = $1",
            agent_name,
            self.instance_id.as_str()
        )
        .execute(pool.as_ref())
        .await
        .map_err(RepositoryError::database)?;

        Ok(())
    }

    pub async fn get_agent_status(
        &self,
        agent_name: &str,
    ) -> Result<Option<AgentServiceRow>, RepositoryError> {
        let pool = &self.pool;

        let row = sqlx::query!(
            "SELECT name, pid, port, status FROM services WHERE instance_id = $2 AND name = $1",
            agent_name,
            self.instance_id.as_str()
        )
        .fetch_optional(pool.as_ref())
        .await
        .map_err(RepositoryError::database)?;

        Ok(row.map(|r| AgentServiceRow {
            name: r.name,
            pid: r.pid,
            port: r.port,
            status: r.status,
        }))
    }

    pub async fn mark_crashed(&self, agent_name: &str) -> Result<(), RepositoryError> {
        let pool = &self.write_pool;

        sqlx::query!(
            "UPDATE services SET status = 'error', pid = NULL, updated_at = CURRENT_TIMESTAMP \
             WHERE instance_id = $2 AND name = $1",
            agent_name,
            self.instance_id.as_str()
        )
        .execute(pool.as_ref())
        .await
        .map_err(RepositoryError::database)?;

        Ok(())
    }

    pub async fn mark_stopped(&self, agent_name: &str) -> Result<(), RepositoryError> {
        let pool = &self.write_pool;

        sqlx::query!(
            "UPDATE services SET status = 'stopped', pid = NULL, updated_at = CURRENT_TIMESTAMP \
             WHERE instance_id = $2 AND name = $1",
            agent_name,
            self.instance_id.as_str()
        )
        .execute(pool.as_ref())
        .await
        .map_err(RepositoryError::database)?;

        Ok(())
    }

    pub async fn mark_error(&self, agent_name: &str) -> Result<(), RepositoryError> {
        let pool = &self.write_pool;

        sqlx::query!(
            "UPDATE services SET status = 'error', pid = NULL, updated_at = CURRENT_TIMESTAMP \
             WHERE instance_id = $2 AND name = $1",
            agent_name,
            self.instance_id.as_str()
        )
        .execute(pool.as_ref())
        .await
        .map_err(RepositoryError::database)?;

        Ok(())
    }

    pub async fn list_running_agents(&self) -> Result<Vec<AgentServerIdRow>, RepositoryError> {
        let pool = &self.pool;

        let rows = sqlx::query!("SELECT name FROM services WHERE instance_id = $1 AND status = 'running'",
            self.instance_id.as_str()
        )
            .fetch_all(pool.as_ref())
            .await
            .map_err(RepositoryError::database)?;

        Ok(rows
            .into_iter()
            .map(|r| AgentServerIdRow { name: r.name })
            .collect())
    }

    pub async fn list_running_agent_pids(
        &self,
    ) -> Result<Vec<AgentServerIdPidRow>, RepositoryError> {
        let pool = &self.pool;

        let rows = sqlx::query!(
            "SELECT name, pid FROM services WHERE instance_id = $1 AND status = 'running' AND pid \
             IS NOT NULL",
            self.instance_id.as_str()
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(RepositoryError::database)?;

        Ok(rows
            .into_iter()
            .filter_map(|r| r.pid.map(|pid| AgentServerIdPidRow { name: r.name, pid }))
            .collect())
    }

    pub async fn remove_agent_service(&self, agent_name: &str) -> Result<(), RepositoryError> {
        let pool = &self.write_pool;

        sqlx::query!(
            "DELETE FROM services WHERE instance_id = $2 AND name = $1",
            agent_name,
            self.instance_id.as_str()
        )
            .execute(pool.as_ref())
            .await
            .map_err(RepositoryError::database)?;

        Ok(())
    }

    pub async fn update_health_status(
        &self,
        agent_name: &str,
        health_status: &str,
    ) -> Result<(), RepositoryError> {
        let pool = &self.write_pool;

        sqlx::query!(
            "UPDATE services SET status = $1, updated_at = CURRENT_TIMESTAMP WHERE instance_id = $3 \
             AND name = $2",
            health_status,
            agent_name,
            self.instance_id.as_str()
        )
        .execute(pool.as_ref())
        .await
        .map_err(RepositoryError::database)?;

        Ok(())
    }
}
