//! Service repository for managing service records in the database.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::DbPool;

/// Database record for a service
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub module_name: String,
    pub status: String,
    pub pid: Option<i32>,
    pub port: i32,
    pub binary_mtime: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

/// Input for creating a new service
#[derive(Debug)]
pub struct CreateServiceInput<'a> {
    pub name: &'a str,
    pub module_name: &'a str,
    pub status: &'a str,
    pub port: u16,
    pub binary_mtime: Option<i64>,
}

/// Repository for managing services in the database
#[derive(Debug, Clone)]
pub struct ServiceRepository {
    db_pool: DbPool,
}

impl ServiceRepository {
    pub const fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }

    fn get_pool(&self) -> Result<std::sync::Arc<sqlx::PgPool>> {
        self.db_pool
            .pool()
            .ok_or_else(|| anyhow::anyhow!("No database pool available"))
    }

    pub async fn get_service_by_name(&self, name: &str) -> Result<Option<ServiceConfig>> {
        let pool = self.get_pool()?;
        let row = sqlx::query!(
            r#"
            SELECT name, module_name, status, pid, port, binary_mtime,
                   created_at::text as "created_at!", updated_at::text as "updated_at!"
            FROM services
            WHERE name = $1
            "#,
            name
        )
        .fetch_optional(&*pool)
        .await?;

        Ok(row.map(|r| ServiceConfig {
            name: r.name,
            module_name: r.module_name,
            status: r.status,
            pid: r.pid,
            port: r.port,
            binary_mtime: r.binary_mtime,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }))
    }

    pub async fn get_all_agent_service_names(&self) -> Result<Vec<String>> {
        let pool = self.get_pool()?;
        let rows = sqlx::query!(
            r#"
            SELECT name FROM services WHERE module_name = 'agent'
            "#
        )
        .fetch_all(&*pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.name).collect())
    }

    pub async fn get_mcp_services(&self) -> Result<Vec<ServiceConfig>> {
        let pool = self.get_pool()?;
        let rows = sqlx::query!(
            r#"
            SELECT name, module_name, status, pid, port, binary_mtime,
                   created_at::text as "created_at!", updated_at::text as "updated_at!"
            FROM services
            WHERE module_name = 'mcp'
            ORDER BY name
            "#
        )
        .fetch_all(&*pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ServiceConfig {
                name: r.name,
                module_name: r.module_name,
                status: r.status,
                pid: r.pid,
                port: r.port,
                binary_mtime: r.binary_mtime,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect())
    }

    pub async fn create_service(&self, input: CreateServiceInput<'_>) -> Result<()> {
        let pool = self.get_pool()?;
        let port_i32 = i32::from(input.port);
        sqlx::query!(
            r#"
            INSERT INTO services (name, module_name, status, port, binary_mtime)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (name) DO UPDATE SET
              module_name = EXCLUDED.module_name,
              status = EXCLUDED.status,
              port = EXCLUDED.port,
              binary_mtime = EXCLUDED.binary_mtime,
              updated_at = CURRENT_TIMESTAMP
            "#,
            input.name,
            input.module_name,
            input.status,
            port_i32,
            input.binary_mtime
        )
        .execute(&*pool)
        .await?;
        Ok(())
    }

    pub async fn update_service_status(&self, service_name: &str, status: &str) -> Result<()> {
        let pool = self.get_pool()?;
        sqlx::query!(
            r#"
            UPDATE services SET status = $1, updated_at = CURRENT_TIMESTAMP WHERE name = $2
            "#,
            status,
            service_name
        )
        .execute(&*pool)
        .await?;
        Ok(())
    }

    pub async fn delete_service(&self, service_name: &str) -> Result<()> {
        let pool = self.get_pool()?;
        sqlx::query!(
            r#"
            DELETE FROM services WHERE name = $1
            "#,
            service_name
        )
        .execute(&*pool)
        .await?;
        Ok(())
    }

    pub async fn update_service_pid(&self, service_name: &str, pid: i32) -> Result<()> {
        let pool = self.get_pool()?;
        sqlx::query!(
            r#"
            UPDATE services SET pid = $1, updated_at = CURRENT_TIMESTAMP WHERE name = $2
            "#,
            pid,
            service_name
        )
        .execute(&*pool)
        .await?;
        Ok(())
    }

    pub async fn clear_service_pid(&self, service_name: &str) -> Result<()> {
        let pool = self.get_pool()?;
        sqlx::query!(
            r#"
            UPDATE services SET pid = NULL, updated_at = CURRENT_TIMESTAMP WHERE name = $1
            "#,
            service_name
        )
        .execute(&*pool)
        .await?;
        Ok(())
    }

    pub async fn get_all_running_services(&self) -> Result<Vec<ServiceConfig>> {
        let pool = self.get_pool()?;
        let rows = sqlx::query!(
            r#"
            SELECT name, module_name, status, pid, port, binary_mtime,
                   created_at::text as "created_at!", updated_at::text as "updated_at!"
            FROM services
            WHERE status = 'running'
            ORDER BY name
            "#
        )
        .fetch_all(&*pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ServiceConfig {
                name: r.name,
                module_name: r.module_name,
                status: r.status,
                pid: r.pid,
                port: r.port,
                binary_mtime: r.binary_mtime,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect())
    }

    pub async fn count_running_services(&self, module_name: &str) -> Result<usize> {
        let pool = self.get_pool()?;
        let row = sqlx::query!(
            r#"
            SELECT COUNT(*) as "count!" FROM services WHERE module_name = $1 AND status = 'running'
            "#,
            module_name
        )
        .fetch_one(&*pool)
        .await?;

        Ok(usize::try_from(row.count).unwrap_or(0))
    }

    pub async fn mark_service_crashed(&self, service_name: &str) -> Result<()> {
        let pool = self.get_pool()?;
        sqlx::query!(
            r#"
            UPDATE services SET status = 'error', pid = NULL, updated_at = CURRENT_TIMESTAMP WHERE name = $1
            "#,
            service_name
        )
        .execute(&*pool)
        .await?;
        Ok(())
    }

    pub async fn update_service_stopped(&self, service_name: &str) -> Result<()> {
        let pool = self.get_pool()?;
        sqlx::query!(
            r#"
            UPDATE services
            SET status = 'stopped', pid = NULL, updated_at = CURRENT_TIMESTAMP
            WHERE name = $1
            "#,
            service_name
        )
        .execute(&*pool)
        .await?;
        Ok(())
    }

    /// Alias for `get_all_running_services` - returns services with pid field
    pub async fn get_running_services_with_pid(&self) -> Result<Vec<ServiceConfig>> {
        self.get_all_running_services().await
    }

    pub async fn get_services_by_type(&self, module_name: &str) -> Result<Vec<ServiceConfig>> {
        let pool = self.get_pool()?;
        let rows = sqlx::query!(
            r#"
            SELECT name, module_name, status, pid, port, binary_mtime,
                   created_at::text as "created_at!", updated_at::text as "updated_at!"
            FROM services
            WHERE module_name = $1
            ORDER BY name
            "#,
            module_name
        )
        .fetch_all(&*pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ServiceConfig {
                name: r.name,
                module_name: r.module_name,
                status: r.status,
                pid: r.pid,
                port: r.port,
                binary_mtime: r.binary_mtime,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect())
    }

    pub async fn cleanup_stale_entries(&self) -> Result<u64> {
        let pool = self.get_pool()?;
        let result = sqlx::query!(
            r#"
            DELETE FROM services
            WHERE status IN ('error', 'crashed')
               OR (status = 'running' AND pid IS NULL)
            "#
        )
        .execute(&*pool)
        .await?;
        Ok(result.rows_affected())
    }
}
