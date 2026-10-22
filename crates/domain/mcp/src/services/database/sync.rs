//! Reconciliation between recorded MCP service state and live processes.
//!
//! Functions here compare the `services` rows against process identity —
//! never a port probe, which cannot tell our child from a stranger on the
//! same port — marking crashed services and pruning disabled rows so the
//! orchestrator can converge the database on reality at startup. Stale rows
//! are reaped by [`ServiceRepository::reap_stale`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::McpServerConfig;
use crate::error::McpDomainResult;
use crate::services::database::stored_pid;
use crate::services::process::ProcessService;
use systemprompt_database::{Reaped, ServiceRepository};
use systemprompt_loader::subprocess::SubprocessLiveness;
use systemprompt_manifest::services::ServiceStatus;

pub async fn reap_stale_services(repository: &ServiceRepository) -> McpDomainResult<Reaped> {
    repository
        .reap_stale(&SubprocessLiveness)
        .await
        .map_err(Into::into)
}

pub async fn sync_database_state(
    repository: &ServiceRepository,
    servers: &[McpServerConfig],
) -> McpDomainResult<()> {
    for server in servers {
        let server_name = server.service_name();
        if let Some(service) = repository.find_service_by_name(&server_name).await? {
            let owned = match stored_pid(service.pid) {
                Some(pid) => ProcessService::owns(pid, &server_name).await,
                None => false,
            };
            if service.status == ServiceStatus::Running && !owned {
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
            if let Some(pid) = stored_pid(service.pid) {
                ProcessService::stop(pid, &service.name).await?;
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
