use anyhow::Result;
use async_trait::async_trait;
use systemprompt_core_database::DbPool;

use super::{EventHandler, McpEvent};

#[derive(Debug, Clone, Copy)]
pub struct MonitoringHandler;

impl MonitoringHandler {
    pub fn new(_db_pool: DbPool) -> Self {
        Self
    }
}

#[async_trait]
impl EventHandler for MonitoringHandler {
    async fn handle(&self, event: &McpEvent) -> Result<()> {
        let _guard = systemprompt_core_logging::SystemSpan::new("mcp_orchestrator").enter();
        match event {
            McpEvent::ServiceStarted {
                service_name,
                process_id,
                port,
            } => {
                tracing::info!(service_name = %service_name, pid = process_id, port = port, "MCP service started");
            },
            McpEvent::ServiceFailed {
                service_name,
                error,
            } => {
                tracing::error!(service_name = %service_name, error = %error, "MCP service failed");
            },
            McpEvent::ServiceStopped {
                service_name,
                exit_code,
            } => {
                tracing::info!(service_name = %service_name, exit_code = ?exit_code, "MCP service stopped");
            },
            McpEvent::HealthCheckFailed {
                service_name,
                reason,
            } => {
                tracing::warn!(service_name = %service_name, reason = %reason, "Health check failed");
            },
            _ => {},
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "monitoring"
    }
}
