use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

use systemprompt_models::ai::tools::McpTool;
use systemprompt_models::mcp::{
    McpDeploymentProvider, McpRegistry, McpServerState, McpToolProvider, ServerManifest,
};
use systemprompt_models::{RequestContext, ServicesConfig};
use systemprompt_traits::{McpRegistryProvider, McpServerInfo, RegistryError, ServiceOAuthConfig};

use super::RegistryManager;
use crate::services::client::McpClient;
use crate::services::deployment::DeploymentService;

#[async_trait]
impl McpRegistry for RegistryManager {
    async fn list_servers(&self) -> Result<Vec<String>> {
        use systemprompt_loader::ConfigLoader;
        let config = ConfigLoader::load()?;
        Ok(config.mcp_servers.keys().cloned().collect())
    }

    async fn get_server_manifest(&self, _name: &str) -> Result<Option<ServerManifest>> {
        Ok(None)
    }

    async fn find_server(&self, name: &str) -> Result<Option<McpServerState>> {
        let server_config = Self::find_server(name)?;
        Ok(server_config.map(|config| McpServerState {
            name: config.name,
            host: config.host,
            port: config.port,
            status: "unknown".to_string(),
        }))
    }

    async fn server_exists(&self, name: &str) -> Result<bool> {
        use systemprompt_loader::ConfigLoader;
        let config = ConfigLoader::load()?;
        Ok(config.mcp_servers.contains_key(name))
    }
}

#[async_trait]
impl McpToolProvider for RegistryManager {
    async fn list_tools(
        &self,
        server_name: &str,
        context: &RequestContext,
    ) -> Result<Vec<McpTool>> {
        McpClient::list_tools(server_name, context).await
    }

    async fn load_tools_for_servers(
        &self,
        server_names: &[String],
        context: &RequestContext,
    ) -> Result<HashMap<String, Vec<McpTool>>> {
        let mut tools_by_server = HashMap::new();

        for server_name in server_names {
            match McpClient::list_tools(server_name, context).await {
                Ok(tools) => {
                    tools_by_server.insert(server_name.clone(), tools);
                },
                Err(e) => {
                    tracing::warn!(
                        server = %server_name,
                        error = %e,
                        "Failed to load tools from MCP server"
                    );
                },
            }
        }

        Ok(tools_by_server)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct McpDeploymentProviderImpl;

#[async_trait]
impl McpDeploymentProvider for McpDeploymentProviderImpl {
    async fn load_config(&self) -> Result<ServicesConfig> {
        DeploymentService::load_config()
    }

    fn protocol_version(&self) -> &'static str {
        "2024-11-05"
    }
}

#[async_trait]
impl McpRegistryProvider for RegistryManager {
    async fn get_server(&self, name: &str) -> std::result::Result<McpServerInfo, RegistryError> {
        let server = Self::get_server(name).map_err(|e| RegistryError::NotFound(e.to_string()))?;

        Ok(McpServerInfo {
            name: server.name,
            port: server.port,
            enabled: server.enabled,
            oauth: ServiceOAuthConfig {
                required: server.oauth.required,
                scopes: server
                    .oauth
                    .scopes
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                audience: server.oauth.audience.to_string(),
            },
        })
    }

    async fn list_enabled_servers(&self) -> std::result::Result<Vec<McpServerInfo>, RegistryError> {
        let servers =
            Self::get_enabled_servers().map_err(|e| RegistryError::Unavailable(e.to_string()))?;

        Ok(servers
            .into_iter()
            .map(|server| McpServerInfo {
                name: server.name,
                port: server.port,
                enabled: server.enabled,
                oauth: ServiceOAuthConfig {
                    required: server.oauth.required,
                    scopes: server
                        .oauth
                        .scopes
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                    audience: server.oauth.audience.to_string(),
                },
            })
            .collect())
    }
}
