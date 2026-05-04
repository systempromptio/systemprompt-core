//! Lifecycle operations for [`McpOrchestrator`]: start/stop/restart/build
//! flows.

use crate::McpServerConfig;
use crate::error::{McpDomainError, McpDomainResult};
use systemprompt_traits::StartupEventSender;

use super::super::process::ProcessManager;
use super::McpOrchestrator;
use super::events::McpEvent;

impl McpOrchestrator {
    /// Start one or all enabled internal MCP services.
    pub async fn start_services(&self, service_name: Option<String>) -> McpDomainResult<()> {
        self.start_services_with_events(service_name, None).await
    }

    /// Start services and forward progress to the supplied event channel.
    pub async fn start_services_with_events(
        &self,
        service_name: Option<String>,
        events: Option<&StartupEventSender>,
    ) -> McpDomainResult<()> {
        let servers: Vec<_> = self
            .get_target_servers(service_name, true)
            .await?
            .into_iter()
            .filter(McpServerConfig::is_internal)
            .collect();
        let mut failed = Vec::new();

        for server in servers {
            tracing::info!(service = %server.name, "Starting MCP service");

            self.event_bus()
                .publish(McpEvent::ServiceStartRequested {
                    service_name: server.name.clone(),
                })
                .await?;

            match self
                .lifecycle()
                .start_server_with_events(&server, events)
                .await
            {
                Ok(()) => {
                    if let Ok(Some(service_info)) =
                        self.database().get_service_by_name(&server.name).await
                    {
                        self.event_bus()
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
                    self.event_bus()
                        .publish(McpEvent::ServiceFailed {
                            service_name: server.name,
                            error: error_msg,
                        })
                        .await?;
                },
            }
        }

        if !failed.is_empty() {
            return Err(McpDomainError::Internal(format!(
                "Failed to start {} services: {}",
                failed.len(),
                failed
                    .iter()
                    .map(|(n, e)| format!("{n} ({e})"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }

        Ok(())
    }

    /// Stop one or all running internal MCP services.
    pub async fn stop_services(&self, service_name: Option<String>) -> McpDomainResult<()> {
        let servers: Vec<_> = self
            .get_target_servers(service_name, false)
            .await?
            .into_iter()
            .filter(McpServerConfig::is_internal)
            .collect();

        for server in servers {
            tracing::info!(service = %server.name, "Stopping MCP service");

            match self.lifecycle().stop_server(&server).await {
                Ok(()) => {
                    self.event_bus()
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

    /// Asynchronously request restart of one or all services via the event bus.
    pub async fn restart_services(&self, service_name: Option<String>) -> McpDomainResult<()> {
        let servers: Vec<_> = self
            .get_target_servers(service_name, false)
            .await?
            .into_iter()
            .filter(McpServerConfig::is_internal)
            .collect();

        for server in servers {
            tracing::info!(service = %server.name, "Restarting MCP service");

            self.event_bus()
                .publish(McpEvent::ServiceRestartRequested {
                    service_name: server.name,
                    reason: "Manual restart".to_string(),
                })
                .await?;
        }

        Ok(())
    }

    /// Synchronously restart services (block until each restart completes).
    pub async fn restart_services_sync(&self, service_name: Option<String>) -> McpDomainResult<()> {
        let servers: Vec<_> = self
            .get_target_servers(service_name, false)
            .await?
            .into_iter()
            .filter(McpServerConfig::is_internal)
            .collect();

        for server in servers {
            tracing::info!(service = %server.name, "Restarting MCP service");
            self.lifecycle().restart_server(&server).await?;
        }

        Ok(())
    }

    /// Build (cargo) and restart each targeted service.
    pub async fn build_and_restart_services(
        &self,
        service_name: Option<String>,
    ) -> McpDomainResult<()> {
        let servers: Vec<_> = self
            .get_target_servers(service_name, true)
            .await?
            .into_iter()
            .filter(McpServerConfig::is_internal)
            .collect();

        for server in servers {
            tracing::info!(service = %server.name, "Building service");
            ProcessManager::build_server(&server)?;

            tracing::info!(service = %server.name, "Restarting service");
            self.lifecycle().restart_server(&server).await?;
        }

        Ok(())
    }

    /// Build (cargo) each targeted service without restarting.
    pub async fn build_services(&self, service_name: Option<String>) -> McpDomainResult<()> {
        let servers: Vec<_> = self
            .get_target_servers(service_name, true)
            .await?
            .into_iter()
            .filter(McpServerConfig::is_internal)
            .collect();

        for server in servers {
            tracing::info!(service = %server.name, "Building service");
            ProcessManager::build_server(&server)?;
        }

        tracing::info!("Build completed");
        Ok(())
    }
}
