//! `McpRegistry` trait implementation over `RegistryService`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::ready;

use async_trait::async_trait;

use systemprompt_identifiers::McpServerId;
use systemprompt_models::ServicesConfig;
use systemprompt_models::errors::{McpRegistryError, McpRegistryResult};
use systemprompt_models::mcp::{McpDeploymentProvider, McpRegistry, McpServerState};
use systemprompt_traits::{McpRegistryProvider, McpServerInfo, RegistryError, ServiceOAuthConfig};

use super::RegistryService;
use crate::error::McpDomainError;
use crate::services::deployment::DeploymentService;

impl From<McpDomainError> for McpRegistryError {
    fn from(err: McpDomainError) -> Self {
        match err {
            McpDomainError::ServerNotFound(name) => Self::NotFound(name),
            other => Self::Configuration(Box::new(other)),
        }
    }
}

fn registry_error(err: McpDomainError) -> RegistryError {
    match err {
        McpDomainError::ServerNotFound(what) => RegistryError::NotFound(what),
        other => RegistryError::Unavailable(Box::new(other)),
    }
}

fn typed_server_ids(names: impl Iterator<Item = String>) -> McpRegistryResult<Vec<McpServerId>> {
    names
        .map(|name| {
            McpServerId::try_new(name).map_err(|e| McpRegistryError::Configuration(Box::new(e)))
        })
        .collect()
}

fn load_services() -> McpRegistryResult<ServicesConfig> {
    systemprompt_loader::ConfigLoader::load()
        .map_err(|e| McpRegistryError::Configuration(Box::new(e)))
}

impl McpRegistry for RegistryService {
    fn list_servers(&self) -> impl Future<Output = McpRegistryResult<Vec<McpServerId>>> + Send {
        ready(load_services().and_then(|config| typed_server_ids(config.mcp_servers.into_keys())))
    }

    fn find_server(
        &self,
        name: &McpServerId,
    ) -> impl Future<Output = McpRegistryResult<Option<McpServerState>>> + Send {
        ready(
            Self::find_server(self, name.as_str())
                .map_err(McpRegistryError::from)
                .map(|server_config| {
                    server_config.map(|config| McpServerState {
                        name: name.clone(),
                        host: config.host,
                        port: config.port,
                    })
                }),
        )
    }

    fn server_exists(
        &self,
        name: &McpServerId,
    ) -> impl Future<Output = McpRegistryResult<bool>> + Send {
        ready(load_services().map(|config| config.mcp_servers.contains_key(name.as_str())))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct McpDeploymentProviderImpl;

impl McpDeploymentProvider for McpDeploymentProviderImpl {
    fn load_config(&self) -> impl Future<Output = McpRegistryResult<ServicesConfig>> + Send {
        ready(DeploymentService::load_config().map_err(McpRegistryError::from))
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
        let server = Self::get_server(self, name).map_err(registry_error)?;
        Ok(server_info(server))
    }

    async fn list_enabled_servers(&self) -> Result<Vec<McpServerInfo>, RegistryError> {
        let servers = Self::get_enabled_servers(self).map_err(registry_error)?;
        Ok(servers.into_iter().map(server_info).collect())
    }
}
