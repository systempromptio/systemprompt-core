//! The MCP server registry listing.
//!
//! Each enabled server is projected with the endpoint synthesised from its
//! name; the configured remote endpoint of an external server is never
//! listed. A registry that cannot be read answers the fixed 5xx body with its
//! cause logged, never an empty list.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use systemprompt_extension::router::ApiHttpError;
use systemprompt_models::CollectionResponse;
use systemprompt_models::modules::ApiPaths;
use systemprompt_traits::McpServerInfo;

use super::McpRegistryState;

#[derive(Debug, Serialize, Deserialize)]
pub struct McpRegistryServer {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    pub enabled: bool,
    pub display_in_web: bool,
    pub oauth_required: bool,
    pub oauth_scopes: Vec<String>,
    pub endpoint: String,
    pub status: String,
}

impl From<McpServerInfo> for McpRegistryServer {
    fn from(server: McpServerInfo) -> Self {
        Self {
            endpoint: ApiPaths::mcp_server_endpoint(&server.name),
            status: if server.enabled {
                "enabled"
            } else {
                "disabled"
            }
            .to_owned(),
            name: server.name,
            version: server.version,
            description: server.description,
            port: server.port,
            enabled: server.enabled,
            display_in_web: server.display_in_web,
            oauth_required: server.oauth.required,
            oauth_scopes: server.oauth.scopes,
        }
    }
}

pub async fn handle_mcp_registry(State(state): State<McpRegistryState>) -> Response {
    match state.registry.list_enabled_servers().await {
        Ok(servers) => Json(CollectionResponse::new(
            servers
                .into_iter()
                .map(McpRegistryServer::from)
                .collect::<Vec<_>>(),
        ))
        .into_response(),
        Err(e) => ApiHttpError::internal("Failed to retrieve MCP registry", e).into_response(),
    }
}
