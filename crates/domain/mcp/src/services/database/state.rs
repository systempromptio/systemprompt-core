//! Persisted MCP service-state rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use crate::error::McpDomainResult;
use crate::services::spawn_target::SpawnTarget;
use systemprompt_config::paths::AppPaths;
use systemprompt_database::{CreateServiceInput, ServiceRepository};
use systemprompt_identifiers::ServiceName;
use systemprompt_models::services::{ServiceModule, ServiceStatus};

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

pub fn get_binary_mtime_for_service(paths: &AppPaths, service_name: &str) -> Option<i64> {
    paths
        .build()
        .resolve_binary(service_name)
        .ok()
        .and_then(|path| get_binary_mtime(path.as_path()))
}

pub async fn register_service(
    repo: &ServiceRepository,
    paths: &AppPaths,
    config: &McpServerConfig,
    pid: u32,
) -> McpDomainResult<String> {
    let binary_mtime = get_binary_mtime_for_service(paths, &config.name);

    let port = config.spawn_port()?;
    tracing::debug!(
        service = %config.name,
        pid = pid,
        port,
        binary_mtime = ?binary_mtime,
        "Registering MCP service"
    );

    let service_name = ServiceName::new(config.name.as_str());
    repo.create_service(CreateServiceInput {
        name: &service_name,
        module_name: ServiceModule::Mcp,
        status: ServiceStatus::Running,
        port,
        binary_mtime,
    })
    .await
    .inspect_err(|e| {
        tracing::error!(service = %config.name, error = %e, "Failed to create service record");
    })?;

    repo.update_service_pid(&service_name, pid as i32)
        .await
        .inspect_err(|e| {
            tracing::error!(service = %config.name, error = %e, "Failed to update PID for service");
        })?;

    tracing::debug!(service = %config.name, pid = pid, "Service registered in database");
    Ok(config.name.clone())
}

pub async fn unregister_service(
    repo: &ServiceRepository,
    service_name: &str,
) -> McpDomainResult<()> {
    repo.delete_service(&ServiceName::new(service_name))
        .await
        .map_err(Into::into)
}

pub async fn get_service_by_name(
    repo: &ServiceRepository,
    name: &str,
) -> McpDomainResult<Option<ServiceInfo>> {
    let result = repo.find_service_by_name(&ServiceName::new(name)).await?;

    Ok(result.map(|r| ServiceInfo {
        name: r.name.as_str().to_owned(),
        status: r.status.as_str().to_owned(),
        pid: r.pid,
        port: r.port as u16,
        binary_mtime: r.binary_mtime,
    }))
}

pub async fn get_running_servers(
    repo: &ServiceRepository,
    registry: &crate::services::registry::RegistryService,
) -> McpDomainResult<Vec<McpServerConfig>> {
    let all_services = repo.list_mcp_services().await?;

    registry.validate()?;
    let mut running_configs = Vec::new();

    for service in all_services {
        if service.status == ServiceStatus::Running
            && let Some(config) = registry.find_server(service.name.as_str())?
        {
            running_configs.push(config);
        }
    }

    Ok(running_configs)
}

pub async fn register_existing_process(
    repo: &ServiceRepository,
    paths: &AppPaths,
    config: &McpServerConfig,
    pid: u32,
) -> McpDomainResult<String> {
    let binary_mtime = get_binary_mtime_for_service(paths, &config.name);

    let service_name = ServiceName::new(config.name.as_str());
    repo.create_service(CreateServiceInput {
        name: &service_name,
        module_name: ServiceModule::Mcp,
        status: ServiceStatus::Running,
        port: config.spawn_port()?,
        binary_mtime,
    })
    .await?;

    repo.update_service_pid(&service_name, pid as i32).await?;

    Ok(config.name.clone())
}
