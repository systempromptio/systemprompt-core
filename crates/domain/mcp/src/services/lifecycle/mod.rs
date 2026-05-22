//! MCP server process lifecycle.
//!
//! Start, stop, restart, and health-check flows, coordinating the process,
//! network, database, and monitoring services behind a single handle.

pub mod health;
pub mod restart;
pub mod shutdown;
pub mod startup;

use crate::McpServerConfig;
use crate::error::McpDomainResult;
use crate::services::database::DatabaseService;
use crate::services::monitoring::MonitoringService;
use crate::services::network::NetworkService;
use crate::services::process::ProcessService;
use std::sync::Arc;
use systemprompt_models::AppPaths;
use systemprompt_traits::StartupEventSender;

#[derive(Debug, Clone)]
pub struct LifecycleOrchestrator {
    process: ProcessService,
    network: NetworkService,
    database: DatabaseService,
    monitoring: MonitoringService,
    app_paths: Arc<AppPaths>,
}

impl LifecycleOrchestrator {
    pub const fn new(
        process: ProcessService,
        network: NetworkService,
        database: DatabaseService,
        monitoring: MonitoringService,
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
        startup::start_server(self, config, None).await
    }

    pub async fn start_server_with_events(
        &self,
        config: &McpServerConfig,
        events: Option<&StartupEventSender>,
    ) -> McpDomainResult<()> {
        startup::start_server(self, config, events).await
    }

    pub async fn stop_server(&self, config: &McpServerConfig) -> McpDomainResult<()> {
        shutdown::stop_server(self, config).await
    }

    pub async fn restart_server(&self, config: &McpServerConfig) -> McpDomainResult<()> {
        restart::restart_server(self, config).await
    }

    pub async fn health_check(&self, config: &McpServerConfig) -> McpDomainResult<bool> {
        health::check_server_health(self, config).await
    }

    pub const fn process(&self) -> &ProcessService {
        &self.process
    }

    pub const fn network(&self) -> &NetworkService {
        &self.network
    }

    pub const fn database(&self) -> &DatabaseService {
        &self.database
    }

    pub const fn monitoring(&self) -> &MonitoringService {
        &self.monitoring
    }
}
