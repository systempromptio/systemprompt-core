//! Event handler applying lifecycle transitions to MCP services.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::McpDomainResult;
use async_trait::async_trait;

use super::{EventHandler, McpEvent};

#[derive(Debug, Clone, Copy)]
pub struct LifecycleHandler;

#[async_trait]
impl EventHandler for LifecycleHandler {
    async fn handle(&self, event: &McpEvent) -> McpDomainResult<()> {
        match event {
            McpEvent::ServiceStartRequested { service_name } => {
                tracing::info!(service = %service_name, "Service start requested");
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
                    "Service restart requested"
                );
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
