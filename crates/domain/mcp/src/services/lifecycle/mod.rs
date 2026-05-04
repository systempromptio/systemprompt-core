pub mod health;
pub mod restart;
pub mod shutdown;
pub mod startup;

use crate::McpServerConfig;
use crate::error::McpDomainResult;
use crate::services::database::DatabaseManager;
use crate::services::monitoring::MonitoringManager;
use crate::services::network::NetworkManager;
use crate::services::process::ProcessManager;
use std::sync::Arc;
use systemprompt_models::AppPaths;
use systemprompt_traits::StartupEventSender;

#[derive(Debug, Clone)]
pub struct LifecycleManager {
    process: ProcessManager,
    network: NetworkManager,
    database: DatabaseManager,
    monitoring: MonitoringManager,
    app_paths: Arc<AppPaths>,
}

impl LifecycleManager {
    pub const fn new(
        process: ProcessManager,
        network: NetworkManager,
        database: DatabaseManager,
        monitoring: MonitoringManager,
        app_paths: Arc<AppPaths>,
    ) -> Self {
        Self {
            process,
            network,
            database,
            monitoring,
            app_paths,
        }
    }

    pub fn app_paths(&self) -> &AppPaths {
        &self.app_paths
    }

    pub async fn start_server(&self, config: &McpServerConfig) -> McpDomainResult<()> {
        startup::start_server(self, config, None)
            .await
            .map_err(Into::into)
    }

    pub async fn start_server_with_events(
        &self,
        config: &McpServerConfig,
        events: Option<&StartupEventSender>,
    ) -> McpDomainResult<()> {
        startup::start_server(self, config, events)
            .await
            .map_err(Into::into)
    }

    pub async fn stop_server(&self, config: &McpServerConfig) -> McpDomainResult<()> {
        shutdown::stop_server(self, config)
            .await
            .map_err(Into::into)
    }

    pub async fn restart_server(&self, config: &McpServerConfig) -> McpDomainResult<()> {
        restart::restart_server(self, config)
            .await
            .map_err(Into::into)
    }

    pub async fn health_check(&self, config: &McpServerConfig) -> McpDomainResult<bool> {
        health::check_server_health(self, config)
            .await
            .map_err(Into::into)
    }

    pub const fn process(&self) -> &ProcessManager {
        &self.process
    }

    pub const fn network(&self) -> &NetworkManager {
        &self.network
    }

    pub const fn database(&self) -> &DatabaseManager {
        &self.database
    }

    pub const fn monitoring(&self) -> &MonitoringManager {
        &self.monitoring
    }
}
