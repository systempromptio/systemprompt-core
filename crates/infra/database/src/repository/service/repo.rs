//! Async repository over the `services` registry table.
//!
//! Rows are keyed by `(instance_id, name)`: every replica registers, judges
//! and reaps only the processes it spawned itself. The single cross-instance
//! statement is `delete_dead_instances` (in `maintenance`), which reaps rows
//! whose heartbeat stopped, so a replica that vanished without cleanup is
//! garbage-collected by the scheduler rather than by the next node to boot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_identifiers::InstanceId;

use super::model::{CreateServiceInput, ServiceConfig, UpsertServiceProcessInput};
use crate::DbPool;
use crate::error::DatabaseResult;

#[derive(Debug, Clone)]
pub struct ServiceRepository {
    pub(super) pool: Arc<PgPool>,
    pub(super) write_pool: Arc<PgPool>,
    pub(super) instance_id: InstanceId,
}

impl ServiceRepository {
    pub fn new(db: &DbPool, instance_id: InstanceId) -> Self {
        let pool = db.pool();
        let write_pool = db.write_pool();
        Self {
            pool,
            write_pool,
            instance_id,
        }
    }

    pub const fn instance_id(&self) -> &InstanceId {
        &self.instance_id
    }

    pub async fn find_service_by_name(&self, name: &str) -> DatabaseResult<Option<ServiceConfig>> {
        let row = sqlx::query_as!(
            ServiceConfig,
            r#"
            SELECT instance_id, name, module_name, status, pid, port, binary_mtime,
                   heartbeat_at::text as "heartbeat_at!",
                   created_at::text as "created_at!", updated_at::text as "updated_at!"
            FROM services
            WHERE instance_id = $1 AND name = $2
            "#,
            self.instance_id.as_str(),
            name
        )
        .fetch_optional(&*self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list_all_agent_service_names(&self) -> DatabaseResult<Vec<String>> {
        let rows = sqlx::query!(
            r#"SELECT name FROM services WHERE instance_id = $1 AND module_name = 'agent'"#,
            self.instance_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.name).collect())
    }

    pub async fn list_mcp_services(&self) -> DatabaseResult<Vec<ServiceConfig>> {
        self.list_services_by_type("mcp").await
    }

    pub async fn create_service(&self, input: CreateServiceInput<'_>) -> DatabaseResult<()> {
        let port_i32 = i32::from(input.port);
        sqlx::query!(
            r#"
            INSERT INTO services (instance_id, name, module_name, status, port, binary_mtime)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (instance_id, name) DO UPDATE SET
              module_name = EXCLUDED.module_name,
              status = EXCLUDED.status,
              port = EXCLUDED.port,
              binary_mtime = EXCLUDED.binary_mtime,
              heartbeat_at = CURRENT_TIMESTAMP,
              updated_at = CURRENT_TIMESTAMP
            "#,
            self.instance_id.as_str(),
            input.name,
            input.module_name,
            input.status,
            port_i32,
            input.binary_mtime
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn upsert_service_process(
        &self,
        input: UpsertServiceProcessInput<'_>,
    ) -> DatabaseResult<()> {
        let port_i32 = i32::from(input.port);
        sqlx::query!(
            r#"
            INSERT INTO services (instance_id, name, module_name, pid, port, status, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP)
            ON CONFLICT (instance_id, name) DO UPDATE SET
              pid = EXCLUDED.pid,
              port = EXCLUDED.port,
              status = EXCLUDED.status,
              heartbeat_at = CURRENT_TIMESTAMP,
              updated_at = CURRENT_TIMESTAMP
            "#,
            self.instance_id.as_str(),
            input.name,
            input.module_name,
            input.pid,
            port_i32,
            input.status
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn update_service_status(
        &self,
        service_name: &str,
        status: &str,
    ) -> DatabaseResult<()> {
        sqlx::query!(
            r#"UPDATE services SET status = $1, updated_at = CURRENT_TIMESTAMP
               WHERE instance_id = $2 AND name = $3"#,
            status,
            self.instance_id.as_str(),
            service_name
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn delete_service(&self, service_name: &str) -> DatabaseResult<()> {
        sqlx::query!(
            r#"DELETE FROM services WHERE instance_id = $1 AND name = $2"#,
            self.instance_id.as_str(),
            service_name
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn update_service_pid(&self, service_name: &str, pid: i32) -> DatabaseResult<()> {
        sqlx::query!(
            r#"UPDATE services SET pid = $1, updated_at = CURRENT_TIMESTAMP
               WHERE instance_id = $2 AND name = $3"#,
            pid,
            self.instance_id.as_str(),
            service_name
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn update_service_port(&self, service_name: &str, port: u16) -> DatabaseResult<()> {
        let port_i32 = i32::from(port);
        sqlx::query!(
            r#"UPDATE services SET port = $1, updated_at = CURRENT_TIMESTAMP
               WHERE instance_id = $2 AND name = $3"#,
            port_i32,
            self.instance_id.as_str(),
            service_name
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn clear_service_pid(&self, service_name: &str) -> DatabaseResult<()> {
        sqlx::query!(
            r#"UPDATE services SET pid = NULL, updated_at = CURRENT_TIMESTAMP
               WHERE instance_id = $1 AND name = $2"#,
            self.instance_id.as_str(),
            service_name
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn list_all_running_services(&self) -> DatabaseResult<Vec<ServiceConfig>> {
        let rows = sqlx::query_as!(
            ServiceConfig,
            r#"
            SELECT instance_id, name, module_name, status, pid, port, binary_mtime,
                   heartbeat_at::text as "heartbeat_at!",
                   created_at::text as "created_at!", updated_at::text as "updated_at!"
            FROM services
            WHERE instance_id = $1 AND status = 'running'
            ORDER BY name
            "#,
            self.instance_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_running_services_by_module(
        &self,
        module_name: &str,
    ) -> DatabaseResult<Vec<ServiceConfig>> {
        let rows = sqlx::query_as!(
            ServiceConfig,
            r#"
            SELECT instance_id, name, module_name, status, pid, port, binary_mtime,
                   heartbeat_at::text as "heartbeat_at!",
                   created_at::text as "created_at!", updated_at::text as "updated_at!"
            FROM services
            WHERE instance_id = $1 AND module_name = $2 AND status = 'running'
            ORDER BY name
            "#,
            self.instance_id.as_str(),
            module_name
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn count_running_services(&self, module_name: &str) -> DatabaseResult<usize> {
        let row = sqlx::query!(
            r#"SELECT COUNT(*) as "count!" FROM services
               WHERE instance_id = $1 AND module_name = $2 AND status = 'running'"#,
            self.instance_id.as_str(),
            module_name
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(usize::try_from(row.count).unwrap_or(0))
    }

    pub async fn mark_service_crashed(&self, service_name: &str) -> DatabaseResult<()> {
        sqlx::query!(
            r#"UPDATE services SET status = 'error', pid = NULL, updated_at = CURRENT_TIMESTAMP
               WHERE instance_id = $1 AND name = $2"#,
            self.instance_id.as_str(),
            service_name
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn update_service_stopped(&self, service_name: &str) -> DatabaseResult<()> {
        sqlx::query!(
            r#"UPDATE services SET status = 'stopped', pid = NULL, updated_at = CURRENT_TIMESTAMP
               WHERE instance_id = $1 AND name = $2"#,
            self.instance_id.as_str(),
            service_name
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn list_running_services_with_pid(&self) -> DatabaseResult<Vec<ServiceConfig>> {
        self.list_all_running_services().await
    }

    pub async fn list_services_by_type(
        &self,
        module_name: &str,
    ) -> DatabaseResult<Vec<ServiceConfig>> {
        let rows = sqlx::query_as!(
            ServiceConfig,
            r#"
            SELECT instance_id, name, module_name, status, pid, port, binary_mtime,
                   heartbeat_at::text as "heartbeat_at!",
                   created_at::text as "created_at!", updated_at::text as "updated_at!"
            FROM services
            WHERE instance_id = $1 AND module_name = $2
            ORDER BY name
            "#,
            self.instance_id.as_str(),
            module_name
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }
}
