use super::LifecycleManager;
use crate::services::monitoring::health::{perform_health_check, HealthStatus};
use crate::services::network::port_manager::MAX_PORT_CLEANUP_ATTEMPTS;
use crate::services::network::NetworkManager;
use crate::services::process::ProcessManager;
use crate::McpServerConfig;
use anyhow::Result;
use std::time::Duration;
use systemprompt_traits::{StartupEventExt, StartupEventSender};

pub async fn start_server(
    manager: &LifecycleManager,
    config: &McpServerConfig,
    events: Option<&StartupEventSender>,
) -> Result<()> {
    tracing::debug!("Starting MCP server: {} :{}", config.name, config.port);

    if let Some(tx) = events {
        tx.mcp_starting(&config.name, config.port);
    }

    ProcessManager::verify_binary(config)?;

    manager.network().prepare_port(config.port).await?;

    manager
        .network()
        .wait_for_port_release_with_retry(config.port, MAX_PORT_CLEANUP_ATTEMPTS)
        .await?;

    let pid = ProcessManager::spawn_server(config)?;

    let startup_time = wait_for_startup(config, pid, events).await?;

    manager
        .database()
        .register_service(config, pid, startup_time)
        .await?;

    tracing::info!("MCP started: {} :{}", config.name, config.port);

    Ok(())
}

async fn wait_for_startup(
    config: &McpServerConfig,
    expected_pid: u32,
    events: Option<&StartupEventSender>,
) -> Result<Option<i32>> {
    tracing::debug!(service = %config.name, "Waiting for service to become available");

    let start_time = std::time::Instant::now();
    let max_attempts = 15;
    let base_delay = Duration::from_millis(300);

    for attempt in 1..=max_attempts {
        let delay = calculate_delay(attempt, base_delay);
        tokio::time::sleep(delay).await;

        if let Some(tx) = events {
            tx.mcp_health_check(&config.name, attempt as u8, max_attempts as u8);
        }

        if !ProcessManager::is_running(expected_pid) {
            return Err(anyhow::anyhow!(
                "Process {expected_pid} died during startup"
            ));
        }

        if !NetworkManager::is_port_responsive(config.port) {
            continue;
        }

        if let Some(result) =
            check_health_status(config, attempt, max_attempts, start_time, events).await?
        {
            return Ok(Some(result));
        }
    }

    let error_msg = format!(
        "Service {} failed health validation after {} attempts",
        config.name, max_attempts
    );

    if let Some(tx) = events {
        tx.mcp_failed(&config.name, &error_msg);
    }

    Err(anyhow::anyhow!("{}", error_msg))
}

fn calculate_delay(attempt: u32, base_delay: Duration) -> Duration {
    if attempt == 1 {
        Duration::from_millis(500)
    } else {
        base_delay * std::cmp::min(attempt, 5)
    }
}

async fn check_health_status(
    config: &McpServerConfig,
    attempt: u32,
    max_attempts: u32,
    start_time: std::time::Instant,
    events: Option<&StartupEventSender>,
) -> Result<Option<i32>> {
    let health_result = match perform_health_check(config).await {
        Ok(r) => r,
        Err(e) => {
            if attempt >= max_attempts - 5 {
                tracing::trace!(service = %config.name, error = %e, "Health check error");
            }
            return Ok(None);
        },
    };

    let startup_time_ms = start_time.elapsed().as_millis() as i32;

    match health_result.status {
        HealthStatus::Healthy => {
            handle_healthy_status(config, &health_result, startup_time_ms, events);
            Ok(Some(startup_time_ms))
        },
        HealthStatus::Degraded if attempt >= max_attempts - 2 => {
            handle_degraded_status(config, &health_result, startup_time_ms, events);
            Ok(Some(startup_time_ms))
        },
        _ => {
            if let Some(ref err_msg) = health_result.details.error_message {
                tracing::trace!(service = %config.name, error = %err_msg, "Health check not yet healthy");
            }
            Ok(None)
        },
    }
}

fn handle_healthy_status(
    config: &McpServerConfig,
    health_result: &super::super::monitoring::health::HealthCheckResult,
    startup_time_ms: i32,
    events: Option<&StartupEventSender>,
) {
    let tools_count = health_result.details.tools_available;

    if let Some(tx) = events {
        tx.mcp_ready(
            &config.name,
            config.port,
            Duration::from_millis(startup_time_ms as u64),
            tools_count,
        );
    }

    tracing::info!(
        service = %config.name,
        tools = tools_count,
        startup_ms = startup_time_ms,
        requires_auth = health_result.details.requires_auth,
        "MCP service validated"
    );
}

fn handle_degraded_status(
    config: &McpServerConfig,
    health_result: &super::super::monitoring::health::HealthCheckResult,
    startup_time_ms: i32,
    events: Option<&StartupEventSender>,
) {
    let error_msg = health_result
        .details
        .error_message
        .as_deref()
        .filter(|e| !e.is_empty())
        .unwrap_or("[degraded - no error message]");

    tracing::warn!(
        service = %config.name,
        error = error_msg,
        startup_ms = startup_time_ms,
        "Service degraded but accepting connections"
    );

    if let Some(tx) = events {
        tx.mcp_ready(
            &config.name,
            config.port,
            Duration::from_millis(startup_time_ms as u64),
            0,
        );
    }
}
