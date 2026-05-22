use super::{LifecycleOrchestrator, shutdown, startup};
use crate::McpServerConfig;
use crate::error::McpDomainResult;
use crate::services::process::ProcessService;

pub async fn restart_server(
    manager: &LifecycleOrchestrator,
    config: &McpServerConfig,
) -> McpDomainResult<()> {
    tracing::info!(service = %config.name, "Restarting service");

    tracing::debug!(service = %config.name, "Stopping current instance");
    shutdown::stop_server(manager, config).await?;

    tracing::debug!(service = %config.name, "Waiting for clean shutdown");
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    verify_clean_state(manager, config).await?;

    tracing::debug!(service = %config.name, "Starting new instance");
    startup::start_server(manager, config, None).await?;

    tracing::info!(service = %config.name, "Service restarted successfully");
    Ok(())
}

async fn verify_clean_state(
    manager: &LifecycleOrchestrator,
    config: &McpServerConfig,
) -> McpDomainResult<()> {
    tracing::debug!(service = %config.name, "Verifying clean state");

    if let Some(pid) = ProcessService::find_pid_by_port(config.port)? {
        return Err(crate::error::McpDomainError::Internal(format!(
            "Port {} still occupied by PID {}",
            config.port, pid
        )));
    }

    if let Some(service) = manager.database().get_service_by_name(&config.name).await? {
        if service.status == "running" {
            tracing::warn!(service = %config.name, "Database shows service as running, cleaning up");
            manager
                .database()
                .update_service_status(&config.name, "stopped")
                .await?;
            manager.database().clear_service_pid(&config.name).await?;
        }
    }

    tracing::debug!(service = %config.name, "Clean state verified");
    Ok(())
}
