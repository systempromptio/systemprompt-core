use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use systemprompt_mcp::services::RegistryManager;
use systemprompt_models::{ApiError, CollectionResponse};

#[derive(Debug, Serialize, Deserialize)]
pub struct McpRegistryServer {
    pub name: String,
    pub version: String,
    pub description: String,
    pub port: u16,
    pub enabled: bool,
    pub display_in_web: bool,
    pub oauth_required: bool,
    pub oauth_scopes: Vec<String>,
    pub endpoint: String,
    pub status: String,
}

pub async fn handle_mcp_registry() -> impl IntoResponse {
    let server_configs = match RegistryManager::get_enabled_servers() {
        Ok(configs) => configs,
        Err(e) => {
            tracing::error!(error = %e, "Failed to load MCP server configs");
            return ApiError::internal_error(format!("Failed to retrieve MCP registry: {e}"))
                .into_response();
        },
    };

    let servers: Vec<McpRegistryServer> = server_configs
        .iter()
        .map(|config| McpRegistryServer {
            name: config.name.clone(),
            version: config.version.clone(),
            description: config.description.clone(),
            port: config.port,
            enabled: config.enabled,
            display_in_web: config.display_in_web,
            oauth_required: config.oauth.required,
            oauth_scopes: config
                .oauth
                .scopes
                .iter()
                .map(ToString::to_string)
                .collect(),
            endpoint: format!("/api/v1/mcp/{}/mcp", config.name),
            status: if config.enabled {
                "enabled".to_string()
            } else {
                "disabled".to_string()
            },
        })
        .collect();

    CollectionResponse::new(servers).into_response()
}
