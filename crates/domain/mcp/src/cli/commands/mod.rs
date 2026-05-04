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
