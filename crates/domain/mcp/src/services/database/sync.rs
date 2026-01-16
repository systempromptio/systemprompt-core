use std::sync::Arc;

use crate::{McpServerConfig, ERROR, RUNNING, STOPPED};
use anyhow::Result;
use systemprompt_core_database::ServiceRepository;
use systemprompt_core_scheduler::ProcessCleanup;
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};

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

    let process_alive = pid.is_some_and(|p| ProcessCleanup::process_exists(p as u32));

    port_healthy && process_alive
}

pub async fn cleanup_stale_services(db_pool: &systemprompt_core_database::DbPool) -> Result<()> {
    let repository = ServiceRepository::new(Arc::clone(db_pool));
    let services = repository.get_mcp_services().await?;

    for service in services {
        if service.status == RUNNING {
            let port = service.port as u16;
            if !is_port_listening(port).await {
                repository
                    .update_service_status(&service.name, STOPPED)
                    .await?;
            }
        }
    }

    Ok(())
}

pub async fn delete_crashed_services(db_pool: &systemprompt_core_database::DbPool) -> Result<()> {
    let repository = ServiceRepository::new(Arc::clone(db_pool));
    let services = repository.get_mcp_services().await?;

    for service in services {
        if service.status == ERROR {
            repository.delete_service(&service.name).await?;
        }
    }

    Ok(())
}

pub async fn sync_database_state(
    db_pool: &systemprompt_core_database::DbPool,
    servers: &[McpServerConfig],
) -> Result<()> {
    let repository = ServiceRepository::new(Arc::clone(db_pool));

    for server in servers {
        if let Some(service) = repository.get_service_by_name(&server.name).await? {
            let port = service.port as u16;
            let pid = service.pid;

            if !is_service_healthy(port, pid).await {
                repository.mark_service_crashed(&server.name).await?;
            }
        }
    }

    Ok(())
}

pub async fn reconcile_running_processes(
    db_pool: &systemprompt_core_database::DbPool,
) -> Result<Vec<String>> {
    let repository = ServiceRepository::new(Arc::clone(db_pool));
    let mut discrepancies = Vec::new();

    let running_services = repository.get_mcp_services().await?;

    for service in running_services {
        if service.status == RUNNING {
            let port = service.port as u16;
            let pid = service.pid;

            if !is_service_healthy(port, pid).await {
                let reason = if pid.is_none() {
                    "no PID recorded".to_string()
                } else if !is_port_listening(port).await {
                    format!("port {port} not responding")
                } else {
                    "process not alive".to_string()
                };
                discrepancies.push(format!("{} ({})", service.name, reason));
            }
        }
    }

    Ok(discrepancies)
}

pub async fn repair_database_inconsistencies(
    db_pool: &systemprompt_core_database::DbPool,
) -> Result<()> {
    let repository = ServiceRepository::new(Arc::clone(db_pool));

    let services = repository.get_mcp_services().await?;
    for service in services {
        if service.status == RUNNING && service.pid.is_none() {
            repository
                .update_service_status(&service.name, STOPPED)
                .await?;
        }
    }

    let all_services = repository.get_mcp_services().await?;
    let mut seen_names = std::collections::HashSet::new();
    for service in all_services {
        if !seen_names.insert(service.name.clone()) {
            repository.delete_service(&service.name).await?;
        }
    }

    Ok(())
}

pub async fn delete_disabled_services(
    db_pool: &systemprompt_core_database::DbPool,
    enabled_servers: &[McpServerConfig],
) -> Result<usize> {
    let repository = ServiceRepository::new(Arc::clone(db_pool));
    let enabled_names: std::collections::HashSet<&str> =
        enabled_servers.iter().map(|s| s.name.as_str()).collect();

    let all_services = repository.get_mcp_services().await?;
    let mut deleted_count = 0;

    for service in all_services {
        if !enabled_names.contains(service.name.as_str()) {
            if let Some(pid) = service.pid {
                let pid_u32 = pid as u32;
                if ProcessCleanup::process_exists(pid_u32) {
                    ProcessCleanup::terminate_gracefully(pid_u32, 500).await;
                }
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
