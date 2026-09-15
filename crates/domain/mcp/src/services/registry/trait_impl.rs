//! `McpRegistry` trait implementation over `RegistryService`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::collections::HashMap;

use systemprompt_identifiers::McpServerId;
use systemprompt_models::ai::tools::McpTool;
use systemprompt_models::errors::{McpRegistryError, McpRegistryResult};
use systemprompt_models::mcp::{
    McpDeploymentProvider, McpRegistry, McpServerState, McpServerStatus, McpToolProvider,
};
use systemprompt_models::{RequestContext, ServicesConfig};
use systemprompt_traits::{McpRegistryProvider, McpServerInfo, RegistryError, ServiceOAuthConfig};

use super::RegistryService;
use crate::error::McpDomainError;
use crate::services::client::McpClient;
use crate::services::deployment::DeploymentService;

impl From<McpDomainError> for McpRegistryError {
    fn from(err: McpDomainError) -> Self {
        match err {
            McpDomainError::ServerNotFound(name) => Self::NotFound(name),
            other => Self::Configuration(other.to_string()),
        }
    }
}

fn typed_server_ids(names: impl Iterator<Item = String>) -> McpRegistryResult<Vec<McpServerId>> {
    names
        .map(|name| {
            McpServerId::try_new(name).map_err(|e| McpRegistryError::Configuration(e.to_string()))
        })
        .collect()
}

#[async_trait]
impl McpRegistry for RegistryService {
    async fn list_servers(&self) -> McpRegistryResult<Vec<McpServerId>> {
        use systemprompt_loader::ConfigLoader;
        let config =
            ConfigLoader::load().map_err(|e| McpRegistryError::Configuration(e.to_string()))?;
        typed_server_ids(config.mcp_servers.keys().cloned())
    }

    async fn find_server(&self, name: &McpServerId) -> McpRegistryResult<Option<McpServerState>> {
        let server_config = Self::find_server(self, name.as_str())?;
        Ok(server_config.map(|config| McpServerState {
            name: name.clone(),
            host: config.host,
            port: config.port,
            status: McpServerStatus::Unknown,
        }))
    }

    async fn server_exists(&self, name: &McpServerId) -> McpRegistryResult<bool> {
        use systemprompt_loader::ConfigLoader;
        let config =
            ConfigLoader::load().map_err(|e| McpRegistryError::Configuration(e.to_string()))?;
        Ok(config.mcp_servers.contains_key(name.as_str()))
    }
}

#[async_trait]
impl McpToolProvider for RegistryService {
    async fn list_tools(
        &self,
        server_name: &McpServerId,
        context: &RequestContext,
    ) -> McpRegistryResult<Vec<McpTool>> {
        let server_config = Self::get_server(self, server_name.as_str())?;
        McpClient::list_tools(&server_config, context)
            .await
            .map_err(|e| McpRegistryError::Transport {
                server: server_name.to_string(),
                message: e.to_string(),
            })
    }

    async fn load_tools_for_servers(
        &self,
        server_names: &[McpServerId],
        context: &RequestContext,
    ) -> McpRegistryResult<HashMap<McpServerId, Vec<McpTool>>> {
        let mut tools_by_server = HashMap::new();

        for server_name in server_names {
            let server_config = match Self::find_server(self, server_name.as_str()) {
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
    async fn load_config(&self) -> McpRegistryResult<ServicesConfig> {
        DeploymentService::load_config().map_err(McpRegistryError::from)
    }

    fn protocol_version(&self) -> &'static str {
        crate::mcp_protocol_version_str()
    }
}

fn server_info(server: crate::McpServerConfig) -> McpServerInfo {
    McpServerInfo {
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
            ema: server.oauth.ema,
        },
    }
}

#[async_trait]
impl McpRegistryProvider for RegistryService {
    async fn get_server(&self, name: &str) -> Result<McpServerInfo, RegistryError> {
        let server =
            Self::get_server(self, name).map_err(|e| RegistryError::NotFound(e.to_string()))?;
        Ok(server_info(server))
    }

    async fn list_enabled_servers(&self) -> Result<Vec<McpServerInfo>, RegistryError> {
        let servers = Self::get_enabled_servers(self)
            .map_err(|e| RegistryError::Unavailable(e.to_string()))?;
        Ok(servers.into_iter().map(server_info).collect())
    }
}
