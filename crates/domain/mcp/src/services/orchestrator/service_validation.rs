//! Pre-start validation of MCP service definitions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::McpDomainResult;
use crate::services::spawn_target::SpawnTarget;

use crate::services::client::{validate_connection_by_url, validate_connection_with_auth};
use crate::services::database::DatabaseService;
use crate::services::registry::RegistryService;
use systemprompt_identifiers::ServiceName;
use systemprompt_manifest::services::ServiceStatus;

pub(super) async fn validate_service(
    service_name: &ServiceName,
    database: &DatabaseService,
    registry: &RegistryService,
) -> McpDomainResult<()> {
    let servers = registry.get_enabled_servers()?;
    let server = servers
        .iter()
        .find(|s| service_name.as_str() == s.name)
        .ok_or_else(|| crate::error::McpDomainError::ServerNotFound(service_name.to_string()))?;

    tracing::info!(
        service = %service_name,
        port = server.port,
        endpoint = %server.remote_endpoint,
        enabled = server.enabled,
        oauth_required = server.oauth.required,
        "Validating MCP service"
    );

    if server.is_external() {
        if server.external_auth.is_some() {
            tracing::debug!(
                service = %service_name,
                "Skipping probe for accessor-backed external MCP service; its bearer is minted per-user on demand"
            );
            return Ok(());
        }
        let validation_result =
            validate_connection_by_url(service_name, &server.remote_endpoint).await?;
        log_validation_result(service_name, &validation_result);
        return Ok(());
    }

    let service_info = database.get_service_by_name(service_name).await?;

    let is_running = service_info
        .as_ref()
        .is_some_and(|info| info.status == ServiceStatus::Running);

    if !is_running {
        tracing::warn!(
            service = %service_name,
            "Service is not currently running"
        );
        return Ok(());
    }

    tracing::debug!(service = %service_name, "Connecting to MCP service");

    let validation_result = validate_connection_with_auth(
        service_name,
        "127.0.0.1",
        server.spawn_port()?,
        server.oauth.required,
    )
    .await?;

    log_validation_result(service_name, &validation_result);

    Ok(())
}

fn log_validation_result(
    service_name: &ServiceName,
    validation_result: &crate::services::client::McpConnectionResult,
) {
    if validation_result.success {
        tracing::info!(
            service = %service_name,
            server_name = ?validation_result.server_info.as_ref().map(|s| &s.implementation_name),
            version = ?validation_result.server_info.as_ref().map(|s| &s.version),
            tools_count = ?validation_result.tools_count,
            connection_time_ms = validation_result.connection_time_ms,
            validation_type = %validation_result.validation_type,
            "Successfully connected to MCP service"
        );
    } else {
        let error = validation_result
            .error_message
            .as_deref()
            .filter(|e| !e.is_empty())
            .unwrap_or("[no error message]");
        tracing::error!(
            service = %service_name,
            error = %error,
            "Failed to connect to MCP service"
        );
    }
}
