//! Reconciliation between recorded MCP service state and live processes.
//!
//! Functions here compare the `mcp_services` table against actual port
//! liveness and process existence, marking crashed services and pruning
//! disabled rows so the orchestrator can converge the database on reality at
//! startup.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::McpServerConfig;
use crate::error::McpDomainResult;
use crate::services::process::utils;
use systemprompt_database::ServiceRepository;
use systemprompt_manifest::services::ServiceStatus;
use tokio::net::TcpStream;
use tokio::time::{Duration, timeout};

const HEALTH_CHECK_TIMEOUT_SECS: u64 = 5;

async fn is_port_listening(port: u16) -> bool {
    matches!(
        timeout(
            Duration::from_secs(HEALTH_CHECK_TIMEOUT_SECS),
            TcpStream::connect(format!("127.0.0.1:{port}")),
        )
        .await,
        Ok(Ok(_))
    )
}

async fn is_service_healthy(port: u16, pid: Option<i32>) -> bool {
    let port_healthy = is_port_listening(port).await;

    let process_alive = pid.is_some_and(|p| utils::process_exists(p as u32));

    port_healthy && process_alive
}

pub async fn cleanup_stale_services(repository: &ServiceRepository) -> McpDomainResult<()> {
    let services = repository.list_mcp_services().await?;

    for service in services {
        if service.status == ServiceStatus::Running {
            let port = service.port as u16;
            if !is_port_listening(port).await {
                repository
                    .update_service_status(&service.name, ServiceStatus::Stopped)
                    .await?;
            }
        }
    }

    Ok(())
}

pub async fn delete_crashed_services(repository: &ServiceRepository) -> McpDomainResult<()> {
    let services = repository.list_mcp_services().await?;

    for service in services {
        if service.status == ServiceStatus::Error {
            repository.delete_service(&service.name).await?;
        }
    }

    Ok(())
}

pub async fn sync_database_state(
    repository: &ServiceRepository,
    servers: &[McpServerConfig],
) -> McpDomainResult<()> {
    for server in servers {
        let server_name = server.service_name();
        if let Some(service) = repository.find_service_by_name(&server_name).await? {
            let port = service.port as u16;
            let pid = service.pid;

            if !is_service_healthy(port, pid).await {
                repository.mark_service_crashed(&server_name).await?;
            }
        }
    }

    Ok(())
}

pub async fn delete_disabled_services(
    repository: &ServiceRepository,
    enabled_servers: &[McpServerConfig],
) -> McpDomainResult<usize> {
    let enabled_names: std::collections::HashSet<&str> =
        enabled_servers.iter().map(|s| s.name.as_str()).collect();

    let all_services = repository.list_mcp_services().await?;
    let mut deleted_count = 0;

    for service in all_services {
        if !enabled_names.contains(service.name.as_str()) {
            if let Some(pid) = service.pid {
                crate::services::process::ProcessService::terminate_gracefully_verified(
                    pid as u32,
                    &service.name,
                )
                .await?;
            }

            repository.delete_service(&service.name).await?;
            tracing::info!(
                service_name = %service.name,
                "Deleted disabled service from database"
            );
            deleted_count += 1;
        }
    }

    Ok(deleted_count)
}
