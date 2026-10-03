//! MCP server restart with clean-state verification.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{LifecycleService, shutdown, startup};
use crate::McpServerConfig;
use crate::error::McpDomainResult;
use crate::services::process::ProcessService;
use crate::services::spawn_target::SpawnTarget;
use systemprompt_manifest::services::ServiceStatus;

pub async fn restart_server(
    lifecycle: &LifecycleService,
    config: &McpServerConfig,
) -> McpDomainResult<()> {
    tracing::info!(service = %config.name, "Restarting service");

    tracing::debug!(service = %config.name, "Stopping current instance");
    shutdown::stop_server(lifecycle, config).await?;

    verify_clean_state(lifecycle, config).await?;

    tracing::debug!(service = %config.name, "Starting new instance");
    startup::start_server(lifecycle, config, None).await?;

    tracing::info!(service = %config.name, "Service restarted successfully");
    Ok(())
}

async fn verify_clean_state(
    lifecycle: &LifecycleService,
    config: &McpServerConfig,
) -> McpDomainResult<()> {
    tracing::debug!(service = %config.name, "Verifying clean state");

    let port = config.spawn_port()?;
    if let Some(&pid) = ProcessService::port_holders(port).await?.first() {
        return Err(crate::error::McpDomainError::PortStillOccupied { port, pid });
    }

    if let Some(service) = lifecycle
        .database()
        .get_service_by_name(&config.service_name())
        .await?
        && service.status == ServiceStatus::Running
    {
        tracing::warn!(service = %config.name, "Database shows service as running, cleaning up");
        lifecycle
            .database()
            .update_service_status(&config.service_name(), ServiceStatus::Stopped)
            .await?;
        lifecycle
            .database()
            .clear_service_pid(&config.service_name())
            .await?;
    }

    tracing::debug!(service = %config.name, "Clean state verified");
    Ok(())
}
