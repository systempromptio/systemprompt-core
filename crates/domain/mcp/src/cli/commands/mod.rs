use crate::services::McpOrchestrator;
use anyhow::Result;

pub async fn start_services(manager: &McpOrchestrator, service_name: Option<String>) -> Result<()> {
    manager.start_services(service_name).await
}

pub async fn stop_services(manager: &McpOrchestrator, service_name: Option<String>) -> Result<()> {
    manager.stop_services(service_name).await
}

pub async fn show_status(manager: &McpOrchestrator) -> Result<()> {
    manager.show_status().await
}

pub async fn list_services(manager: &McpOrchestrator) -> Result<()> {
    manager.list_services().await
}
