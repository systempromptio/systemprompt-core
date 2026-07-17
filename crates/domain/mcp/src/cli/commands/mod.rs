//! CLI command entry points for MCP service management.
//!
//! Thin async wrappers that delegate to [`McpOrchestrator`] for starting,
//! stopping, and reporting on managed MCP servers from the `systemprompt`
//! CLI surface.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::McpDomainResult;
use crate::services::McpOrchestrator;

pub async fn start_services(
    manager: &McpOrchestrator,
    service_name: Option<String>,
) -> McpDomainResult<()> {
    manager.start_services(service_name).await
}

pub async fn stop_services(
    manager: &McpOrchestrator,
    service_name: Option<String>,
) -> McpDomainResult<()> {
    manager.stop_services(service_name).await
}

pub async fn show_status(manager: &McpOrchestrator) -> McpDomainResult<()> {
    manager.show_status().await
}

pub async fn list_services(manager: &McpOrchestrator) -> McpDomainResult<()> {
    manager.list_services().await
}
