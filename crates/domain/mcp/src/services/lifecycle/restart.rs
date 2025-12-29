use super::{shutdown, startup, LifecycleManager};
use crate::services::process::ProcessManager;
use crate::McpServerConfig;
use anyhow::Result;

pub async fn restart_server(manager: &LifecycleManager, config: &McpServerConfig) -> Result<()> {
    tracing::info!(service = %config.name, "Restarting service");

    tracing::debug!(service = %config.name, "Stopping current instance");
    shutdown::stop_server(manager, config).await?;

    tracing::debug!(service = %config.name, "Waiting for clean shutdown");
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

    verify_clean_state(manager, config).await?;

    tracing::debug!(service = %config.name, "Starting new instance");
    startup::start_server(manager, config, None).await?;

    tracing::info!(service = %config.name, "Service restarted successfully");
    Ok(())
}

async fn verify_clean_state(manager: &LifecycleManager, config: &McpServerConfig) -> Result<()> {
    tracing::debug!(service = %config.name, "Verifying clean state");

    if let Some(pid) = ProcessManager::find_pid_by_port(config.port)? {
        return Err(anyhow::anyhow!(
            "Port {} still occupied by PID {}",
            config.port,
            pid
        ));
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
