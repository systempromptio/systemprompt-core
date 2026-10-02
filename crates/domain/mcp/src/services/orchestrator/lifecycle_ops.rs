//! Lifecycle operations for [`McpOrchestrator`]: start/stop/restart/build
//! flows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{McpDomainError, McpDomainResult, ServiceStartFailure, ServiceStartFailures};
use crate::services::spawn_target::SpawnTarget;
use systemprompt_identifiers::ServiceName;
use systemprompt_traits::StartupEventSender;

use super::super::process::ProcessService;
use super::McpOrchestrator;
use super::events::McpEvent;
use crate::services::database::stored_pid;

/// Result of restarting one MCP server: the stop/start pipeline either
/// completed or failed with the error it returned.
#[derive(Debug)]
pub struct McpRestartOutcome {
    pub service_name: ServiceName,
    pub result: McpDomainResult<()>,
}

impl McpRestartOutcome {
    pub const fn is_restarted(&self) -> bool {
        self.result.is_ok()
    }
}

impl McpOrchestrator {
    pub async fn start_services(&self, service_name: Option<ServiceName>) -> McpDomainResult<()> {
        self.start_services_with_events(service_name, None).await
    }

    pub async fn start_services_with_events(
        &self,
        service_name: Option<ServiceName>,
        events: Option<&StartupEventSender>,
    ) -> McpDomainResult<()> {
        let servers = self.list_target_servers(service_name, true).await?;
        let mut failed = Vec::new();

        for server in servers {
            tracing::info!(service = %server.name, "Starting MCP service");
            let name = server.service_name();

            self.event_bus()
                .publish(McpEvent::ServiceStartRequested {
                    service_name: name.clone(),
                })
                .await?;

            match self
                .lifecycle()
                .start_server_with_events(&server, events)
                .await
            {
                Ok(()) => {
                    let service_info = self
                        .database()
                        .get_service_by_name(&name)
                        .await?
                        .ok_or_else(|| McpDomainError::ServiceRowMissing {
                            service: server.name.clone(),
                        })?;
                    self.event_bus()
                        .publish(McpEvent::ServiceStarted {
                            service_name: name,
                            process_id: stored_pid(service_info.pid),
                            port: server.spawn_port()?,
                        })
                        .await?;
                },
                Err(e) => {
                    let error_msg = e.to_string();
                    failed.push(ServiceStartFailure::new(server.name.clone(), e));
                    self.event_bus()
                        .publish(McpEvent::ServiceFailed {
                            service_name: name,
                            error: error_msg,
                        })
                        .await?;
                },
            }
        }

        if !failed.is_empty() {
            return Err(McpDomainError::ServicesFailedToStart(ServiceStartFailures(
                failed,
            )));
        }

        Ok(())
    }

    pub async fn stop_services(&self, service_name: Option<ServiceName>) -> McpDomainResult<()> {
        let servers = self.list_target_servers(service_name, false).await?;

        for server in servers {
            tracing::info!(service = %server.name, "Stopping MCP service");

            match self.lifecycle().stop_server(&server).await {
                Ok(()) => {
                    self.event_bus()
                        .publish(McpEvent::ServiceStopped {
                            service_name: server.service_name(),
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

    pub async fn restart_services(
        &self,
        service_name: Option<ServiceName>,
    ) -> McpDomainResult<Vec<McpRestartOutcome>> {
        let servers = self.list_target_servers(service_name, false).await?;
        let mut outcomes = Vec::with_capacity(servers.len());

        for server in servers {
            tracing::info!(service = %server.name, "Restarting MCP service");
            let result = self.lifecycle().restart_server(&server).await;
            if let Err(e) = &result {
                tracing::error!(service = %server.name, error = %e, "MCP service restart failed");
            }
            outcomes.push(McpRestartOutcome {
                service_name: server.service_name(),
                result,
            });
        }

        Ok(outcomes)
    }

    pub async fn build_and_restart_services(
        &self,
        service_name: Option<ServiceName>,
    ) -> McpDomainResult<usize> {
        let servers = self.list_target_servers(service_name, true).await?;
        let count = servers.len();

        for server in servers {
            tracing::info!(service = %server.name, "Building service");
            ProcessService::build_server(&server)?;

            tracing::info!(service = %server.name, "Restarting service");
            self.lifecycle().restart_server(&server).await?;
        }

        Ok(count)
    }

    pub async fn build_services(&self, service_name: Option<ServiceName>) -> McpDomainResult<()> {
        let servers = self.list_target_servers(service_name, true).await?;

        for server in servers {
            tracing::info!(service = %server.name, "Building service");
            ProcessService::build_server(&server)?;
        }

        tracing::info!("Build completed");
        Ok(())
    }
}
