//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::LifecycleOrchestrator;
use crate::McpServerConfig;
use crate::error::McpDomainResult;
use crate::services::monitoring::health::{HealthCheckResult, HealthStatus, perform_health_check};
use crate::services::process::ProcessService;

pub async fn check_server_health(
    manager: &LifecycleOrchestrator,
    config: &McpServerConfig,
) -> McpDomainResult<bool> {
    if !is_process_running(manager, config).await? {
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
        mark_service_error(manager, config, &health_result).await?;
    }

    Ok(is_healthy)
}

async fn is_process_running(
    manager: &LifecycleOrchestrator,
    config: &McpServerConfig,
) -> McpDomainResult<bool> {
    let Some(pid) = ProcessService::find_pid_by_port(config.port)? else {
        manager
            .database()
            .update_service_status(&config.name, "stopped")
            .await?;
        return Ok(false);
    };

    if !ProcessService::is_running(pid) {
        manager
            .database()
            .update_service_status(&config.name, "stopped")
            .await?;
        return Ok(false);
    }

    Ok(true)
}

async fn mark_service_error(
    manager: &LifecycleOrchestrator,
    config: &McpServerConfig,
    health_result: &HealthCheckResult,
) -> McpDomainResult<()> {
    manager
        .database()
        .update_service_status(&config.name, "error")
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
    if health_result.details.tools_available > 0 {
        tracing::debug!(
            service = %config.name,
            tools = health_result.details.tools_available,
            latency_ms = health_result.latency_ms,
            "Service health validated"
        );
    }
}
