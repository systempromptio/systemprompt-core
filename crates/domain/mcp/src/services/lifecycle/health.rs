//! MCP server health evaluation and error-state marking.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::LifecycleService;
use crate::McpServerConfig;
use crate::error::McpDomainResult;
use crate::services::monitoring::health::{HealthCheckResult, HealthStatus, perform_health_check};
use crate::services::process::ProcessService;
use crate::services::spawn_target::SpawnTarget;
use systemprompt_models::services::ServiceStatus;

pub async fn check_server_health(
    lifecycle: &LifecycleService,
    config: &McpServerConfig,
) -> McpDomainResult<bool> {
    if !is_process_running(lifecycle, config).await? {
        return Ok(false);
    }

    let health_result = perform_health_check(config).await?;
    let is_healthy = matches!(
        health_result.status,
        HealthStatus::Healthy | HealthStatus::Degraded
    );

    if is_healthy {
        log_healthy_status(config, &health_result);
    } else {
        mark_service_error(lifecycle, config, &health_result).await?;
    }

    Ok(is_healthy)
}

async fn is_process_running(
    lifecycle: &LifecycleService,
    config: &McpServerConfig,
) -> McpDomainResult<bool> {
    let Some(pid) = ProcessService::find_pid_by_port(config.spawn_port()?)? else {
        lifecycle
            .database()
            .update_service_status(&config.service_name(), ServiceStatus::Stopped)
            .await?;
        return Ok(false);
    };

    if !ProcessService::is_running(pid) {
        lifecycle
            .database()
            .update_service_status(&config.service_name(), ServiceStatus::Stopped)
            .await?;
        return Ok(false);
    }

    Ok(true)
}

async fn mark_service_error(
    lifecycle: &LifecycleService,
    config: &McpServerConfig,
    health_result: &HealthCheckResult,
) -> McpDomainResult<()> {
    lifecycle
        .database()
        .update_service_status(&config.service_name(), ServiceStatus::Error)
        .await?;

    if let Some(ref error) = health_result.details.error_message {
        tracing::warn!(
            service = %config.name,
            status = %health_result.status.as_str(),
            error = %error,
            "Service health check warning"
        );
    }

    Ok(())
}

fn log_healthy_status(config: &McpServerConfig, health_result: &HealthCheckResult) {
    if let Some(tools) = health_result
        .details
        .tools_available
        .filter(|count| *count > 0)
    {
        tracing::debug!(
            service = %config.name,
            tools,
            latency_ms = health_result.latency_ms,
            "Service health validated"
        );
    }
}
