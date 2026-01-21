use anyhow::Result;
use tracing::Instrument;

use crate::services::database::state::get_binary_mtime_for_service;
use crate::services::database::{DatabaseManager, ServiceInfo};
use crate::services::process::ProcessManager;
use crate::McpServerConfig;

pub async fn detect_and_handle_orphaned_processes(
    servers: &[McpServerConfig],
    database: &DatabaseManager,
) -> Result<usize> {
    let span = systemprompt_logging::SystemSpan::new("mcp_orchestrator");
    async move {
        let mut killed = 0;
        for server in servers {
            if kill_orphaned_process(server, database).await? {
                killed += 1;
            }
        }
        Ok(killed)
    }
    .instrument(span.into())
    .await
}

async fn kill_orphaned_process(
    server: &McpServerConfig,
    database: &DatabaseManager,
) -> Result<bool> {
    let Some(orphaned_pid) =
        ProcessManager::find_process_on_port_with_name(server.port, &server.name)?
    else {
        return Ok(false);
    };

    if database.get_service_by_name(&server.name).await?.is_some() {
        return Ok(false);
    }

    tracing::info!(
        service = %server.name,
        pid = orphaned_pid,
        port = server.port,
        "Found orphaned process"
    );

    ProcessManager::force_kill(orphaned_pid)?;

    tracing::info!(
        service_name = %server.name,
        pid = orphaned_pid,
        port = server.port,
        "Killed orphaned MCP process, will restart fresh"
    );

    Ok(true)
}

pub async fn detect_and_handle_stale_binaries(
    servers: &[McpServerConfig],
    database: &DatabaseManager,
) -> Result<usize> {
    let span = systemprompt_logging::SystemSpan::new("mcp_orchestrator");
    async move {
        let mut restarted = 0;
        for server in servers {
            if restart_stale_binary(server, database).await? {
                restarted += 1;
            }
        }
        Ok(restarted)
    }
    .instrument(span.into())
    .await
}

async fn restart_stale_binary(
    server: &McpServerConfig,
    database: &DatabaseManager,
) -> Result<bool> {
    let service_info = match database.get_service_by_name(&server.name).await? {
        Some(info) if info.status == "running" => info,
        _ => return Ok(false),
    };

    let Some((stored_mtime, current_mtime)) = get_stale_binary_mtimes(&server.name, &service_info)
    else {
        return Ok(false);
    };

    tracing::info!(
        service = %server.name,
        stored_mtime = stored_mtime,
        current_mtime = current_mtime,
        "Binary rebuilt, restarting"
    );

    kill_and_unregister(server, database, &service_info).await?;

    tracing::info!(
        service_name = %server.name,
        pid = ?service_info.pid,
        stored_mtime = stored_mtime,
        current_mtime = current_mtime,
        "Killed stale binary process, will restart with new binary"
    );

    Ok(true)
}

fn get_stale_binary_mtimes(name: &str, service_info: &ServiceInfo) -> Option<(i64, i64)> {
    let stored_mtime = service_info.binary_mtime?;
    let current_mtime = get_binary_mtime_for_service(name)?;

    (current_mtime != stored_mtime).then_some((stored_mtime, current_mtime))
}

async fn kill_and_unregister(
    server: &McpServerConfig,
    database: &DatabaseManager,
    service_info: &ServiceInfo,
) -> Result<()> {
    if let Some(pid) = service_info.pid {
        ProcessManager::force_kill(pid as u32)?;
    }
    database.unregister_service(&server.name).await
}
