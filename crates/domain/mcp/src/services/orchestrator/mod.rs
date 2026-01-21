use anyhow::Result;
use std::sync::Arc;
use systemprompt_runtime::AppContext;
use systemprompt_traits::StartupEventSender;

mod daemon;
pub mod event_bus;
pub mod events;
pub mod handlers;
mod process_cleanup;
mod reconciliation;
mod schema_sync;
mod server_startup;
mod service_validation;

pub use reconciliation::ReconcileParams;

pub use event_bus::EventBus;
pub use events::McpEvent;
pub use handlers::{DatabaseSyncHandler, HealthCheckHandler, LifecycleHandler, MonitoringHandler};

use super::database::DatabaseManager;
use super::lifecycle::LifecycleManager;
use super::monitoring::MonitoringManager;
use super::network::NetworkManager;
use super::process::ProcessManager;
use super::registry::RegistryManager;
use crate::McpServerConfig;

#[derive(Debug)]
pub struct McpOrchestrator {
    event_bus: Arc<EventBus>,
    lifecycle: LifecycleManager,
    database: DatabaseManager,
    monitoring: MonitoringManager,
    app_context: Arc<AppContext>,
}

impl McpOrchestrator {
    pub fn new(app_context: Arc<AppContext>) -> Result<Self> {
        let mut event_bus = EventBus::new(100);

        RegistryManager::validate()?;
        let database = DatabaseManager::new(Arc::clone(app_context.db_pool()));
        let network = NetworkManager::new();
        let process = ProcessManager::new(Arc::clone(&app_context));
        let monitoring = MonitoringManager::new(Arc::clone(&app_context));
        let lifecycle =
            LifecycleManager::new(process, network, database.clone(), monitoring.clone());

        event_bus.register_handler(Arc::new(LifecycleHandler::new(lifecycle.clone())));

        event_bus.register_handler(Arc::new(MonitoringHandler::new(Arc::clone(
            app_context.db_pool(),
        ))));

        event_bus.register_handler(Arc::new(DatabaseSyncHandler::new(database.clone())));

        let health_handler = HealthCheckHandler::new().with_restart_sender(event_bus.sender());
        event_bus.register_handler(Arc::new(health_handler));

        Ok(Self {
            event_bus: Arc::new(event_bus),
            lifecycle,
            database,
            monitoring,
            app_context,
        })
    }

    pub async fn list_services(&self) -> Result<()> {
        let servers = RegistryManager::get_enabled_servers()?;
        let status_data = self.monitoring.get_status_for_all(&servers).await?;
        MonitoringManager::display_status(&servers, &status_data);
        Ok(())
    }

    pub async fn start_services(&self, service_name: Option<String>) -> Result<()> {
        self.start_services_with_events(service_name, None).await
    }

    pub async fn start_services_with_events(
        &self,
        service_name: Option<String>,
        events: Option<&StartupEventSender>,
    ) -> Result<()> {
        let servers = self.get_target_servers(service_name, true).await?;
        let mut failed = Vec::new();

        for server in servers {
            tracing::info!(service = %server.name, "Starting MCP service");

            self.event_bus
                .publish(McpEvent::ServiceStartRequested {
                    service_name: server.name.clone(),
                })
                .await?;

            match self
                .lifecycle
                .start_server_with_events(&server, events)
                .await
            {
                Ok(()) => {
                    if let Ok(Some(service_info)) =
                        self.database.get_service_by_name(&server.name).await
                    {
                        self.event_bus
                            .publish(McpEvent::ServiceStarted {
                                service_name: server.name.clone(),
                                process_id: service_info.pid.unwrap_or(0) as u32,
                                port: server.port,
                            })
                            .await?;
                    }
                },
                Err(e) => {
                    let error_msg = e.to_string();
                    failed.push((server.name.clone(), error_msg.clone()));
                    self.event_bus
                        .publish(McpEvent::ServiceFailed {
                            service_name: server.name,
                            error: error_msg,
                        })
                        .await?;
                },
            }
        }

        if !failed.is_empty() {
            return Err(anyhow::anyhow!(
                "Failed to start {} services: {}",
                failed.len(),
                failed
                    .iter()
                    .map(|(n, e)| format!("{n} ({e})"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        Ok(())
    }

    pub async fn stop_services(&self, service_name: Option<String>) -> Result<()> {
        let servers = self.get_target_servers(service_name, false).await?;

        for server in servers {
            tracing::info!(service = %server.name, "Stopping MCP service");

            match self.lifecycle.stop_server(&server).await {
                Ok(()) => {
                    self.event_bus
                        .publish(McpEvent::ServiceStopped {
                            service_name: server.name,
                            exit_code: None,
                        })
                        .await?;
                },
                Err(e) => {
                    return Err(e);
                },
            }
        }

        Ok(())
    }

    pub async fn restart_services(&self, service_name: Option<String>) -> Result<()> {
        let servers = self.get_target_servers(service_name, false).await?;

        for server in servers {
            tracing::info!(service = %server.name, "Restarting MCP service");

            self.event_bus
                .publish(McpEvent::ServiceRestartRequested {
                    service_name: server.name,
                    reason: "Manual restart".to_string(),
                })
                .await?;
        }

        Ok(())
    }

    pub async fn restart_services_sync(&self, service_name: Option<String>) -> Result<()> {
        let servers = self.get_target_servers(service_name, false).await?;

        for server in servers {
            tracing::info!(service = %server.name, "Restarting MCP service");
            self.lifecycle.restart_server(&server).await?;
        }

        Ok(())
    }

    pub async fn build_and_restart_services(&self, service_name: Option<String>) -> Result<()> {
        let servers = self.get_target_servers(service_name, true).await?;

        for server in servers {
            tracing::info!(service = %server.name, "Building service");
            ProcessManager::build_server(&server)?;

            tracing::info!(service = %server.name, "Restarting service");
            self.lifecycle.restart_server(&server).await?;
        }

        Ok(())
    }

    pub async fn build_services(&self, service_name: Option<String>) -> Result<()> {
        let servers = self.get_target_servers(service_name, true).await?;

        for server in servers {
            tracing::info!(service = %server.name, "Building service");
            ProcessManager::build_server(&server)?;
        }

        tracing::info!("Build completed");
        Ok(())
    }

    pub async fn show_status(&self) -> Result<()> {
        self.list_services().await
    }

    pub async fn sync_database_state(&self) -> Result<()> {
        tracing::info!("Synchronizing service database state");
        let servers = RegistryManager::get_enabled_servers()?;
        self.database.sync_state(&servers).await
    }

    pub async fn reconcile(&self) -> Result<usize> {
        self.reconcile_with_events(None).await
    }

    pub async fn reconcile_with_events(
        &self,
        events: Option<&StartupEventSender>,
    ) -> Result<usize> {
        reconciliation::reconcile(ReconcileParams {
            database: &self.database,
            lifecycle: &self.lifecycle,
            event_bus: &self.event_bus,
            app_context: &self.app_context,
            events,
        })
        .await
    }

    pub async fn validate_service(&self, service_name: &str) -> Result<()> {
        service_validation::validate_service(service_name, &self.database).await
    }

    pub async fn get_running_servers(&self) -> Result<Vec<McpServerConfig>> {
        self.database.get_running_servers().await
    }

    pub async fn get_service_info(
        &self,
        service_name: &str,
    ) -> Result<Option<super::database::ServiceInfo>> {
        self.database.get_service_by_name(service_name).await
    }

    async fn get_target_servers(
        &self,
        service_name: Option<String>,
        enabled_only: bool,
    ) -> Result<Vec<McpServerConfig>> {
        match service_name {
            Some(name) if name == "all" => {
                if enabled_only {
                    RegistryManager::get_enabled_servers()
                } else {
                    self.database.get_running_servers().await
                }
            },
            Some(name) => {
                let servers = RegistryManager::get_enabled_servers()?;
                Ok(servers.into_iter().filter(|s| s.name == name).collect())
            },
            None => {
                if enabled_only {
                    RegistryManager::get_enabled_servers()
                } else {
                    self.database.get_running_servers().await
                }
            },
        }
    }

    pub async fn run_daemon(&self) -> Result<()> {
        daemon::run_daemon(&self.event_bus, &self.lifecycle, &self.database).await
    }

    pub fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<McpEvent> {
        self.event_bus.subscribe()
    }
}
