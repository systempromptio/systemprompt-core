//! Read-side listings over the `services` registry for this instance.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{InstanceId, ServiceName};
use systemprompt_models::services::ServiceModule;

use super::model::{ServiceConfig, ServiceRow, into_configs};
use super::repo::ServiceRepository;
use crate::error::DatabaseResult;

impl ServiceRepository {
    pub async fn list_all_agent_service_names(&self) -> DatabaseResult<Vec<ServiceName>> {
        let rows = sqlx::query!(
            r#"SELECT name FROM services WHERE instance_id = $1 AND module_name = 'agent'"#,
            self.instance_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| ServiceName::new(r.name)).collect())
    }

    pub async fn list_mcp_services(&self) -> DatabaseResult<Vec<ServiceConfig>> {
        self.list_services_by_type(ServiceModule::Mcp).await
    }

    pub async fn list_all_running_services(&self) -> DatabaseResult<Vec<ServiceConfig>> {
        let rows = sqlx::query_as!(
            ServiceRow,
            r#"
            SELECT instance_id as "instance_id: InstanceId", name, module_name, status, pid, port, binary_mtime,
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
        into_configs(rows)
    }

    pub async fn list_running_services_by_module(
        &self,
        module_name: ServiceModule,
    ) -> DatabaseResult<Vec<ServiceConfig>> {
        let rows = sqlx::query_as!(
            ServiceRow,
            r#"
            SELECT instance_id as "instance_id: InstanceId", name, module_name, status, pid, port, binary_mtime,
                   heartbeat_at::text as "heartbeat_at!",
                   created_at::text as "created_at!", updated_at::text as "updated_at!"
            FROM services
            WHERE instance_id = $1 AND module_name = $2 AND status = 'running'
            ORDER BY name
            "#,
            self.instance_id.as_str(),
            module_name.as_str()
        )
        .fetch_all(&*self.pool)
        .await?;
        into_configs(rows)
    }

    pub async fn count_running_services(
        &self,
        module_name: ServiceModule,
    ) -> DatabaseResult<usize> {
        let row = sqlx::query!(
            r#"SELECT COUNT(*) as "count!" FROM services
               WHERE instance_id = $1 AND module_name = $2 AND status = 'running'"#,
            self.instance_id.as_str(),
            module_name.as_str()
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(usize::try_from(row.count).unwrap_or(0))
    }

    pub async fn list_running_services_with_pid(&self) -> DatabaseResult<Vec<ServiceConfig>> {
        self.list_all_running_services().await
    }

    pub async fn list_services_by_type(
        &self,
        module_name: ServiceModule,
    ) -> DatabaseResult<Vec<ServiceConfig>> {
        let rows = sqlx::query_as!(
            ServiceRow,
            r#"
            SELECT instance_id as "instance_id: InstanceId", name, module_name, status, pid, port, binary_mtime,
                   heartbeat_at::text as "heartbeat_at!",
                   created_at::text as "created_at!", updated_at::text as "updated_at!"
            FROM services
            WHERE instance_id = $1 AND module_name = $2
            ORDER BY name
            "#,
            self.instance_id.as_str(),
            module_name.as_str()
        )
        .fetch_all(&*self.pool)
        .await?;
        into_configs(rows)
    }
}
