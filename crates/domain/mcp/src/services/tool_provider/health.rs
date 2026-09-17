//! Connection and health probes the tool provider runs against managed MCP
//! servers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use tracing::{info, warn};

use crate::services::client::{
    rewrite_url_for_internal_use, validate_connection, validate_connection_by_url,
};
use crate::services::registry::RegistryService;

pub(super) async fn check_server_connection(
    registry: &RegistryService,
    server_name: &str,
    api_server_url: &str,
) {
    let Ok(Some(server_config)) = registry.find_server(server_name) else {
        return;
    };
    let result = if let Some(port) = server_config.port {
        validate_connection(server_name, &server_config.host, port).await
    } else {
        let url = server_config.endpoint(api_server_url);
        let url = rewrite_url_for_internal_use(&url);
        validate_connection_by_url(server_name, &url).await
    };

    match result {
        Ok(result) if result.success => {
            info!(server = server_name, "MCP server connection validated");
        },
        Ok(result) => {
            warn!(
                server = server_name,
                error = result.error_message.as_deref().unwrap_or("[no error]"),
                "MCP server connection validation failed"
            );
        },
        Err(e) => {
            warn!(
                server = server_name,
                error = %e,
                "Failed to validate MCP server connection"
            );
        },
    }
}

pub(super) async fn check_server_health(
    server_name: &str,
    server_port: Option<u16>,
    api_server_url: &str,
) -> bool {
    let url = format!("{}/api/v1/mcp/{}/mcp", api_server_url, server_name);

    let Ok(parsed_url) = url::Url::parse(&url) else {
        return false;
    };

    let host = parsed_url.host_str().unwrap_or("127.0.0.1");
    let actual_port = server_port.unwrap_or_else(|| parsed_url.port().unwrap_or(80));

    validate_connection(server_name, host, actual_port)
        .await
        .is_ok_and(|r| r.success)
}
