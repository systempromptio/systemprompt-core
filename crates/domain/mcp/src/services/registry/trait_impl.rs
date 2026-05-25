use async_trait::async_trait;
use std::collections::HashMap;

use systemprompt_models::ai::tools::McpTool;
use systemprompt_models::errors::ProviderResult;
use systemprompt_models::mcp::{
    McpDeploymentProvider, McpRegistry, McpServerState, McpToolProvider,
};
use systemprompt_models::{RequestContext, ServicesConfig};
use systemprompt_traits::{McpRegistryProvider, McpServerInfo, RegistryError, ServiceOAuthConfig};

use super::RegistryService;
use crate::services::client::McpClient;
use crate::services::deployment::DeploymentService;

#[async_trait]
impl McpRegistry for RegistryService {
    async fn list_servers(&self) -> ProviderResult<Vec<String>> {
        use systemprompt_loader::ConfigLoader;
        let config = ConfigLoader::load()
            .map_err(|e| Box::<dyn std::error::Error + Send + Sync>::from(e.to_string()))?;
        Ok(config.mcp_servers.keys().cloned().collect())
    }

    async fn find_server(&self, name: &str) -> ProviderResult<Option<McpServerState>> {
        let server_config = Self::find_server(self, name)
            .map_err(|e| Box::<dyn std::error::Error + Send + Sync>::from(e.to_string()))?;
        Ok(server_config.map(|config| McpServerState {
            name: config.name,
            host: config.host,
            port: config.port,
            status: "unknown".to_string(),
        }))
    }

    async fn server_exists(&self, name: &str) -> ProviderResult<bool> {
        use systemprompt_loader::ConfigLoader;
        let config = ConfigLoader::load()
            .map_err(|e| Box::<dyn std::error::Error + Send + Sync>::from(e.to_string()))?;
        Ok(config.mcp_servers.contains_key(name))
    }
}

#[async_trait]
impl McpToolProvider for RegistryService {
    async fn list_tools(
        &self,
        server_name: &str,
        context: &RequestContext,
    ) -> ProviderResult<Vec<McpTool>> {
        let server_config = Self::get_server(self, server_name)
            .map_err(|e| Box::<dyn std::error::Error + Send + Sync>::from(e.to_string()))?;
        McpClient::list_tools(&server_config, context)
            .await
            .map_err(|e| Box::<dyn std::error::Error + Send + Sync>::from(e.to_string()))
    }

    async fn load_tools_for_servers(
        &self,
        server_names: &[String],
        context: &RequestContext,
    ) -> ProviderResult<HashMap<String, Vec<McpTool>>> {
        let mut tools_by_server = HashMap::new();

        for server_name in server_names {
            let server_config = match Self::find_server(self, server_name) {
                Ok(Some(cfg)) => cfg,
                Ok(None) => {
                    tracing::warn!(
                        server = %server_name,
                        "MCP server not found in registry"
                    );
                    continue;
                },
                Err(e) => {
                    tracing::warn!(
                        server = %server_name,
                        error = %e,
                        "Failed to resolve MCP server"
                    );
                    continue;
                },
            };
            match McpClient::list_tools(&server_config, context).await {
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
    async fn load_config(&self) -> ProviderResult<ServicesConfig> {
        DeploymentService::load_config()
            .map_err(|e| Box::<dyn std::error::Error + Send + Sync>::from(e.to_string()))
    }

    fn protocol_version(&self) -> &'static str {
        crate::mcp_protocol_version_str()
    }
}

#[async_trait]
impl McpRegistryProvider for RegistryService {
    async fn get_server(&self, name: &str) -> Result<McpServerInfo, RegistryError> {
        let server =
            Self::get_server(self, name).map_err(|e| RegistryError::NotFound(e.to_string()))?;

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

    async fn list_enabled_servers(&self) -> Result<Vec<McpServerInfo>, RegistryError> {
        let servers = Self::get_enabled_servers(self)
            .map_err(|e| RegistryError::Unavailable(e.to_string()))?;

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
