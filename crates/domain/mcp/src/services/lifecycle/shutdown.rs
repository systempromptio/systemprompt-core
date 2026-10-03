//! MCP server shutdown: graceful termination and stale-state cleanup.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::LifecycleService;
use crate::McpServerConfig;
use crate::error::McpDomainResult;
use crate::services::process::ProcessService;
use crate::services::spawn_target::SpawnTarget;
use systemprompt_manifest::services::ServiceStatus;

pub async fn stop_server(
    lifecycle: &LifecycleService,
    config: &McpServerConfig,
) -> McpDomainResult<()> {
    tracing::info!(service = %config.name, "Stopping MCP service");

    let Some(pid) = find_running_process(lifecycle, config).await? else {
        tracing::debug!(service = %config.name, "Service is already stopped");
        cleanup_stale_state(lifecycle, config).await?;
        return Ok(());
    };

    lifecycle
        .database()
        .update_service_status(&config.service_name(), ServiceStatus::Stopping)
        .await?;

    perform_graceful_shutdown(lifecycle, config, pid).await?;

    finalize_shutdown(lifecycle, config).await?;

    tracing::info!(service = %config.name, "Service stopped successfully");
    Ok(())
}

async fn find_running_process(
    lifecycle: &LifecycleService,
    config: &McpServerConfig,
) -> McpDomainResult<Option<u32>> {
    if let Some(db_service) = lifecycle
        .database()
        .get_service_by_name(&config.service_name())
        .await?
        && let Some(db_pid) = db_service.pid
        && ProcessService::is_running(db_pid as u32)
    {
        return Ok(Some(db_pid as u32));
    }

    ProcessService::find_pid_by_port(config.spawn_port()?)
}

async fn perform_graceful_shutdown(
    lifecycle: &LifecycleService,
    config: &McpServerConfig,
    pid: u32,
) -> McpDomainResult<()> {
    tracing::debug!(service = %config.name, pid = pid, "Performing graceful shutdown");

    ProcessService::terminate_gracefully_verified(pid, &config.service_name()).await?;

    lifecycle
        .network()
        .wait_for_port_release(config.spawn_port()?)
        .await?;

    Ok(())
}

async fn finalize_shutdown(
    lifecycle: &LifecycleService,
    config: &McpServerConfig,
) -> McpDomainResult<()> {
    lifecycle
        .database()
        .update_service_status(&config.service_name(), ServiceStatus::Stopped)
        .await?;
    lifecycle
        .database()
        .clear_service_pid(&config.service_name())
        .await?;

    Ok(())
}

async fn cleanup_stale_state(
    lifecycle: &LifecycleService,
    config: &McpServerConfig,
) -> McpDomainResult<()> {
    tracing::debug!(service = %config.name, "Cleaning up stale database entries");

    if let Some(service) = lifecycle
        .database()
        .get_service_by_name(&config.service_name())
        .await?
    {
        lifecycle
            .database()
            .unregister_service(&service.name)
            .await?;
        tracing::debug!(service = %config.name, "Cleaned up stale entry");
    }

    Ok(())
}
