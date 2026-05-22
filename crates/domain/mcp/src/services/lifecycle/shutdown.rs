use super::LifecycleOrchestrator;
use crate::McpServerConfig;
use crate::error::McpDomainResult;
use crate::services::network::NetworkService;
use crate::services::process::ProcessService;

pub async fn stop_server(
    manager: &LifecycleOrchestrator,
    config: &McpServerConfig,
) -> McpDomainResult<()> {
    tracing::info!(service = %config.name, "Stopping MCP service");

    let Some(pid) = find_running_process(manager, config).await? else {
        tracing::debug!(service = %config.name, "Service is already stopped");
        cleanup_stale_state(manager, config).await?;
        return Ok(());
    };

    manager
        .database()
        .update_service_status(&config.name, "stopping")
        .await?;

    perform_graceful_shutdown(manager, config, pid).await?;

    finalize_shutdown(manager, config).await?;

    tracing::info!(service = %config.name, "Service stopped successfully");
    Ok(())
}

async fn find_running_process(
    manager: &LifecycleOrchestrator,
    config: &McpServerConfig,
) -> McpDomainResult<Option<u32>> {
    if let Some(db_service) = manager.database().get_service_by_name(&config.name).await? {
        if let Some(db_pid) = db_service.pid {
            if ProcessService::is_running(db_pid as u32) {
                return Ok(Some(db_pid as u32));
            }
        }
    }

    ProcessService::find_pid_by_port(config.port)
}

async fn perform_graceful_shutdown(
    manager: &LifecycleOrchestrator,
    config: &McpServerConfig,
    pid: u32,
) -> McpDomainResult<()> {
    tracing::debug!(service = %config.name, pid = pid, "Performing graceful shutdown");

    ProcessService::terminate_gracefully(pid)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    if ProcessService::is_running(pid) {
        tracing::debug!(service = %config.name, pid = pid, "Force killing process");
        ProcessService::force_kill(pid)?;
    }

    manager.network().wait_for_port_release(config.port).await?;

    Ok(())
}

async fn finalize_shutdown(
    manager: &LifecycleOrchestrator,
    config: &McpServerConfig,
) -> McpDomainResult<()> {
    manager
        .database()
        .update_service_status(&config.name, "stopped")
        .await?;
    manager.database().clear_service_pid(&config.name).await?;

    NetworkService::cleanup_port_resources(config.port);

    Ok(())
}

async fn cleanup_stale_state(
    manager: &LifecycleOrchestrator,
    config: &McpServerConfig,
) -> McpDomainResult<()> {
    tracing::debug!(service = %config.name, "Cleaning up stale database entries");

    if let Some(service) = manager.database().get_service_by_name(&config.name).await? {
        manager.database().unregister_service(&service.name).await?;
        tracing::debug!(service = %config.name, "Cleaned up stale entry");
    }

    Ok(())
}
