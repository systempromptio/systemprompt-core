use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::get;
use axum::Router;
use serde::{Deserialize, Serialize};

use crate::services::registry::manager::RegistryService;

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

fn error_response(status: StatusCode, message: String) -> Response {
    (
        status,
        Json(serde_json::json!({
            "error": "internal_error",
            "message": message
        })),
    )
        .into_response()
}

fn collection_response<T: Serialize>(items: Vec<T>) -> Response {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "data": items,
            "meta": {
                "total": items.len()
            }
        })),
    )
        .into_response()
}

pub async fn handle_mcp_registry() -> impl IntoResponse {
    let server_configs = match RegistryService::get_enabled_servers_as_config() {
        Ok(configs) => configs,
        Err(e) => {
            tracing::error!(error = %e, "Failed to load MCP server configs");
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to retrieve MCP registry: {e}"),
            );
        }
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

    collection_response(servers)
}

pub fn router() -> Router {
    Router::new().route("/", get(handle_mcp_registry))
}
