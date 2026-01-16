pub mod state;
pub mod sync;

use std::sync::Arc;

use crate::McpServerConfig;
use anyhow::Result;
use systemprompt_core_database::ServiceRepository;

#[derive(Debug, Clone)]
pub struct DatabaseManager {
    db_pool: systemprompt_core_database::DbPool,
}

impl DatabaseManager {
    pub const fn new(db_pool: systemprompt_core_database::DbPool) -> Self {
        Self { db_pool }
    }

    pub async fn register_service(
        &self,
        config: &McpServerConfig,
        pid: u32,
        startup_time: Option<i32>,
    ) -> Result<String> {
        state::register_service(&self.db_pool, config, pid, startup_time).await
    }

    pub async fn unregister_service(&self, service_name: &str) -> Result<()> {
        state::unregister_service(&self.db_pool, service_name).await
    }

    pub async fn get_service_by_name(&self, name: &str) -> Result<Option<ServiceInfo>> {
        state::get_service_by_name(&self.db_pool, name).await
    }

    pub async fn get_running_servers(&self) -> Result<Vec<McpServerConfig>> {
        state::get_running_servers(&self.db_pool).await
    }

    pub async fn update_service_status(&self, name: &str, status: &str) -> Result<()> {
        let repo = ServiceRepository::new(Arc::clone(&self.db_pool));
        repo.update_service_status(name, status).await
    }

    pub async fn clear_service_pid(&self, name: &str) -> Result<()> {
        let repo = ServiceRepository::new(Arc::clone(&self.db_pool));
        repo.clear_service_pid(name).await
    }

    pub async fn cleanup_stale_services(&self) -> Result<()> {
        sync::cleanup_stale_services(&self.db_pool).await
    }

    pub async fn delete_crashed_services(&self) -> Result<()> {
        sync::delete_crashed_services(&self.db_pool).await
    }

    pub async fn sync_state(&self, servers: &[McpServerConfig]) -> Result<()> {
        sync::sync_database_state(&self.db_pool, servers).await
    }

    pub async fn delete_disabled_services(
        &self,
        enabled_servers: &[McpServerConfig],
    ) -> Result<usize> {
        sync::delete_disabled_services(&self.db_pool, enabled_servers).await
    }

    pub async fn register_existing_process(
        &self,
        config: &McpServerConfig,
        pid: u32,
    ) -> Result<String> {
        state::register_existing_process(&self.db_pool, config, pid).await
    }

    pub const fn db_pool(&self) -> &systemprompt_core_database::DbPool {
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
