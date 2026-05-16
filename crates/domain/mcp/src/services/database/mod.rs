//! Database-backed MCP service state.
//!
//! Registers and unregisters running servers, reconciles persisted state
//! against the live registry, and prunes stale or disabled service records.

pub mod state;
pub mod sync;

use crate::McpServerConfig;
use crate::error::McpDomainResult;
use std::sync::Arc;
use systemprompt_database::ServiceRepository;
use systemprompt_models::AppPaths;

#[derive(Debug, Clone)]
pub struct DatabaseManager {
    db_pool: systemprompt_database::DbPool,
    app_paths: Arc<AppPaths>,
}

impl DatabaseManager {
    pub const fn new(db_pool: systemprompt_database::DbPool, app_paths: Arc<AppPaths>) -> Self {
        Self { db_pool, app_paths }
    }

    pub fn app_paths(&self) -> &AppPaths {
        &self.app_paths
    }

    pub async fn register_service(
        &self,
        config: &McpServerConfig,
        pid: u32,
    ) -> McpDomainResult<String> {
        state::register_service(&self.db_pool, &self.app_paths, config, pid).await
    }

    pub async fn unregister_service(&self, service_name: &str) -> McpDomainResult<()> {
        state::unregister_service(&self.db_pool, service_name).await
    }

    pub async fn get_service_by_name(&self, name: &str) -> McpDomainResult<Option<ServiceInfo>> {
        state::get_service_by_name(&self.db_pool, name).await
    }

    pub async fn get_running_servers(&self) -> McpDomainResult<Vec<McpServerConfig>> {
        state::get_running_servers(&self.db_pool).await
    }

    pub async fn update_service_status(&self, name: &str, status: &str) -> McpDomainResult<()> {
        let repo = ServiceRepository::new(&self.db_pool)?;
        repo.update_service_status(name, status)
            .await
            .map_err(Into::into)
    }

    pub async fn clear_service_pid(&self, name: &str) -> McpDomainResult<()> {
        let repo = ServiceRepository::new(&self.db_pool)?;
        repo.clear_service_pid(name).await.map_err(Into::into)
    }

    pub async fn cleanup_stale_services(&self) -> McpDomainResult<()> {
        sync::cleanup_stale_services(&self.db_pool).await
    }

    pub async fn delete_crashed_services(&self) -> McpDomainResult<()> {
        sync::delete_crashed_services(&self.db_pool).await
    }

    pub async fn sync_state(&self, servers: &[McpServerConfig]) -> McpDomainResult<()> {
        sync::sync_database_state(&self.db_pool, servers).await
    }

    pub async fn delete_disabled_services(
        &self,
        enabled_servers: &[McpServerConfig],
    ) -> McpDomainResult<usize> {
        sync::delete_disabled_services(&self.db_pool, enabled_servers).await
    }

    pub async fn register_existing_process(
        &self,
        config: &McpServerConfig,
        pid: u32,
    ) -> McpDomainResult<String> {
        state::register_existing_process(&self.db_pool, &self.app_paths, config, pid).await
    }

    pub const fn db_pool(&self) -> &systemprompt_database::DbPool {
        &self.db_pool
    }
}

#[derive(Debug, Clone)]
pub struct ServiceInfo {
    pub name: String,
    pub status: String,
    pub pid: Option<i32>,
    pub port: u16,
    pub binary_mtime: Option<i64>,
}
