use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use systemprompt_database::{CreateServiceInput, ServiceRepository};
use systemprompt_models::AppPaths;

use super::ServiceInfo;
use crate::McpServerConfig;

pub fn get_binary_mtime(binary_path: &Path) -> Option<i64> {
    binary_path
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
}

pub fn get_binary_mtime_for_service(service_name: &str) -> Option<i64> {
    AppPaths::get()
        .ok()
        .and_then(|paths| paths.build().resolve_binary(service_name).ok())
        .and_then(|path| get_binary_mtime(path.as_path()))
}

pub async fn register_service(
    db_pool: &systemprompt_database::DbPool,
    config: &McpServerConfig,
    pid: u32,
    _startup_time: Option<i32>,
) -> Result<String> {
    let repo = ServiceRepository::new(Arc::clone(db_pool));
    let binary_mtime = get_binary_mtime_for_service(&config.name);

    tracing::debug!(
        service = %config.name,
        pid = pid,
        port = config.port,
        binary_mtime = ?binary_mtime,
        "Registering MCP service"
    );

    repo.create_service(CreateServiceInput {
        name: &config.name,
        module_name: "mcp",
        status: "running",
        port: config.port,
        binary_mtime,
    })
    .await
    .inspect_err(|e| {
        tracing::error!(service = %config.name, error = %e, "Failed to create service record");
    })?;

    repo.update_service_pid(&config.name, pid as i32)
        .await
        .inspect_err(|e| {
            tracing::error!(service = %config.name, error = %e, "Failed to update PID for service");
        })?;

    tracing::debug!(service = %config.name, pid = pid, "Service registered in database");
    Ok(config.name.clone())
}

pub async fn unregister_service(
    db_pool: &systemprompt_database::DbPool,
    service_name: &str,
) -> Result<()> {
    let repo = ServiceRepository::new(Arc::clone(db_pool));
    repo.delete_service(service_name).await
}

pub async fn get_service_by_name(
    db_pool: &systemprompt_database::DbPool,
    name: &str,
) -> Result<Option<ServiceInfo>> {
    let repo = ServiceRepository::new(Arc::clone(db_pool));
    let result = repo.get_service_by_name(name).await?;

    Ok(result.map(|r| ServiceInfo {
        name: r.name,
        status: r.status,
        pid: r.pid,
        port: r.port as u16,
        binary_mtime: r.binary_mtime,
    }))
}

pub async fn get_running_servers(
    db_pool: &systemprompt_database::DbPool,
) -> Result<Vec<McpServerConfig>> {
    use crate::services::registry::RegistryManager;

    let repo = ServiceRepository::new(Arc::clone(db_pool));
    let all_services = repo.get_mcp_services().await?;

    RegistryManager::validate()?;
    let mut running_configs = Vec::new();

    for service in all_services {
        if service.status == "running" {
            if let Some(config) = RegistryManager::find_server(&service.name)? {
                running_configs.push(config);
            }
        }
    }

    Ok(running_configs)
}

pub async fn update_service_state(
    db_pool: &systemprompt_database::DbPool,
    name: &str,
    status: &str,
    _pid: Option<u32>,
) -> Result<()> {
    let repo = ServiceRepository::new(Arc::clone(db_pool));
    repo.update_service_status(name, status).await
}

pub async fn register_existing_process(
    db_pool: &systemprompt_database::DbPool,
    config: &McpServerConfig,
    pid: u32,
) -> Result<String> {
    let repo = ServiceRepository::new(Arc::clone(db_pool));

    let binary_mtime = get_binary_mtime_for_service(&config.name);

    repo.create_service(CreateServiceInput {
        name: &config.name,
        module_name: "mcp",
        status: "running",
        port: config.port,
        binary_mtime,
    })
    .await?;

    repo.update_service_pid(&config.name, pid as i32).await?;

    Ok(config.name.clone())
}
