use anyhow::Result;
use async_trait::async_trait;

use crate::services::lifecycle::LifecycleManager;
use crate::services::registry::RegistryManager;

use super::{EventHandler, McpEvent};

#[derive(Debug)]
pub struct LifecycleHandler {
    lifecycle: LifecycleManager,
}

impl LifecycleHandler {
    pub const fn new(lifecycle: LifecycleManager) -> Self {
        Self { lifecycle }
    }
}

#[async_trait]
impl EventHandler for LifecycleHandler {
    async fn handle(&self, event: &McpEvent) -> Result<()> {
        match event {
            McpEvent::ServiceStartRequested { service_name } => {
                let config = RegistryManager::get_server(service_name)?;
                tracing::info!(service = %service_name, "Starting MCP service");
                self.lifecycle.start_server(&config).await?;
            },
            McpEvent::ServiceStopped {
                service_name,
                exit_code,
            } => {
                tracing::info!(
                    service = %service_name,
                    exit_code = ?exit_code,
                    "Service stopped"
                );
            },
            McpEvent::ServiceRestartRequested {
                service_name,
                reason,
            } => {
                tracing::info!(
                    service = %service_name,
                    reason = %reason,
                    "Restarting service"
                );
                let config = RegistryManager::get_server(service_name)?;
                self.lifecycle.restart_server(&config).await?;
            },
            _ => {},
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "lifecycle"
    }

    fn handles(&self, event: &McpEvent) -> bool {
        matches!(
            event,
            McpEvent::ServiceStartRequested { .. }
                | McpEvent::ServiceStopped { .. }
                | McpEvent::ServiceRestartRequested { .. }
        )
    }
}
