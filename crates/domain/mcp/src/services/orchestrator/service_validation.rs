use anyhow::Result;

use crate::services::client::validate_connection_with_auth;
use crate::services::database::DatabaseManager;
use crate::services::registry::RegistryManager;

pub async fn validate_service(service_name: &str, database: &DatabaseManager) -> Result<()> {
    let servers = RegistryManager::get_enabled_servers()?;
    let server = servers
        .iter()
        .find(|s| s.name == service_name)
        .ok_or_else(|| anyhow::anyhow!("Service '{service_name}' not found in registry"))?;

    tracing::info!(
        service = %service_name,
        port = server.port,
        enabled = server.enabled,
        oauth_required = server.oauth.required,
        "Validating MCP service"
    );

    let service_info = database.get_service_by_name(service_name).await?;

    let is_running = service_info
        .as_ref()
        .is_some_and(|info| info.status == "running");

    if !is_running {
        tracing::warn!(
            service = %service_name,
            "Service is not currently running"
        );
        return Ok(());
    }

    tracing::debug!(service = %service_name, "Connecting to MCP service");

    let validation_result = validate_connection_with_auth(
        &server.name,
        "127.0.0.1",
        server.port,
        server.oauth.required,
    )
    .await?;

    if validation_result.success {
        tracing::info!(
            service = %service_name,
            server_name = ?validation_result.server_info.as_ref().map(|s| &s.server_name),
            version = ?validation_result.server_info.as_ref().map(|s| &s.version),
            tools_count = validation_result.tools_count,
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

    Ok(())
}
