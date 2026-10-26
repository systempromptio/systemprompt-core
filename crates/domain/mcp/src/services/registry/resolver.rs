//! Registry-backed MCP server resolution and its `McpRegistry`,
//! `McpRegistryProvider` and `McpDeploymentProvider` implementations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use systemprompt_identifiers::UserId;
use systemprompt_loader::ExtensionBinaryIndex;
use systemprompt_manifest::Config;
use systemprompt_models::mcp::McpServerType;

use crate::error::{McpDomainError, McpDomainResult};
use crate::services::deployment::DeploymentService;
use async_trait::async_trait;
use std::future::ready;
use systemprompt_identifiers::McpServerId;
use systemprompt_manifest::ServicesConfig;
use systemprompt_manifest::services::{McpDeploymentProvider, ServicesView};
use systemprompt_models::errors::{McpRegistryError, McpRegistryResult};
use systemprompt_models::mcp::{McpRegistry, McpServerState};
use systemprompt_traits::{McpRegistryProvider, McpServerInfo, RegistryError, ServiceOAuthConfig};

#[derive(Debug, Clone)]
pub struct RegistryService {
    owner: UserId,
    services: ServicesView,
}

impl RegistryService {
    #[must_use]
    pub const fn new(owner: UserId, services: ServicesView) -> Self {
        Self { owner, services }
    }

    #[must_use]
    pub const fn services(&self) -> &ServicesView {
        &self.services
    }

    #[must_use]
    pub fn deployments(&self) -> DeploymentService {
        DeploymentService::new(self.services.clone())
    }

    #[must_use]
    pub const fn owner(&self) -> &UserId {
        &self.owner
    }

    pub fn get_enabled_servers_as_config(&self) -> McpDomainResult<Vec<crate::McpServerConfig>> {
        let global_config = Config::get()?;
        let registry = ExtensionBinaryIndex::build(
            Path::new(&global_config.system_path),
            global_config.is_cloud,
            &global_config.bin_path,
        );
        let services_config = self.services.config();
        let mut enabled = Vec::new();

        let mut server_names: Vec<_> = services_config.mcp_servers.keys().collect();
        server_names.sort();

        for server_name in server_names {
            let deployment = &services_config.mcp_servers[server_name];
            if !deployment.enabled {
                continue;
            }
            if deployment.dev_only && global_config.is_cloud {
                continue;
            }

            let crate_path = match (deployment.server_type, deployment.binary.as_deref()) {
                (McpServerType::Internal, Some(binary)) => registry.get_path(binary)?,
                (McpServerType::Internal, None) => {
                    return Err(McpDomainError::Configuration(format!(
                        "{server_name}: internal MCP server declares no binary"
                    )));
                },
                (McpServerType::External, _) => PathBuf::new(),
            };

            let display_name = deployment
                .package
                .clone()
                .or_else(|| deployment.binary.clone())
                .unwrap_or_else(|| server_name.clone());

            let config = crate::McpServerConfig {
                name: server_name.clone(),
                owner: self.owner.clone(),
                server_type: deployment.server_type,
                binary: deployment.binary.clone(),
                enabled: deployment.enabled,
                display_in_web: deployment.display_in_web,
                port: deployment.port,
                crate_path,
                display_name: display_name.clone(),
                description: format!("{display_name} MCP Server"),
                capabilities: vec![],
                schemas: deployment.schemas.clone(),
                oauth: deployment.oauth.clone(),
                tools: deployment.tools.clone(),
                model_config: deployment.model_config.clone(),
                env_vars: deployment.env_vars.clone(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
                host: "0.0.0.0".to_owned(),
                module_name: "mcp".to_owned(),
                protocol: "mcp".to_owned(),
                remote_endpoint: deployment.endpoint.clone().unwrap_or_default(),
                external_auth: deployment.external_auth.clone().or_else(|| {
                    deployment.connector.as_ref().map(|_| {
                        systemprompt_models::mcp::deployment::ExternalAuth {
                            token_endpoint: format!("/api/public/connectors/{server_name}/token"),
                            header: "Authorization".into(),
                            scheme: String::new(),
                        }
                    })
                }),
                headers: deployment.headers.clone(),
            };
            enabled.push(config);
        }

        Ok(enabled)
    }

    pub fn validate(&self) -> McpDomainResult<()> {
        self.services.config().validate()?;
        Ok(())
    }

    pub fn get_enabled_servers(&self) -> McpDomainResult<Vec<crate::McpServerConfig>> {
        self.get_enabled_servers_as_config()
    }

    pub fn get_managed_servers(&self) -> McpDomainResult<Vec<crate::McpServerConfig>> {
        Ok(self
            .get_enabled_servers_as_config()?
            .into_iter()
            .filter(crate::McpServerConfig::is_internal)
            .collect())
    }

    pub fn get_external_servers(&self) -> McpDomainResult<Vec<crate::McpServerConfig>> {
        Ok(self
            .get_enabled_servers_as_config()?
            .into_iter()
            .filter(crate::McpServerConfig::is_external)
            .collect())
    }

    pub fn reload(&self) -> McpDomainResult<()> {
        self.validate()
    }

    pub fn find_server(&self, name: &str) -> McpDomainResult<Option<crate::McpServerConfig>> {
        let servers = self.get_enabled_servers_as_config()?;
        Ok(servers.into_iter().find(|s| s.name == name))
    }

    pub fn get_server(&self, name: &str) -> McpDomainResult<crate::McpServerConfig> {
        self.find_server(name)?.ok_or_else(|| {
            McpDomainError::ServerNotFound(format!("MCP server '{name}' not found in registry"))
        })
    }
}

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

impl McpRegistry for RegistryService {
    fn list_servers(&self) -> impl Future<Output = McpRegistryResult<Vec<McpServerId>>> + Send {
        ready(typed_server_ids(
            self.services.config().mcp_servers.keys().cloned(),
        ))
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
        ready(Ok(self
            .services
            .config()
            .mcp_servers
            .contains_key(name.as_str())))
    }
}

#[derive(Debug, Clone)]
pub struct McpDeploymentProviderImpl {
    deployments: DeploymentService,
}

impl McpDeploymentProviderImpl {
    #[must_use]
    pub const fn new(services: ServicesView) -> Self {
        Self {
            deployments: DeploymentService::new(services),
        }
    }
}

impl McpDeploymentProvider for McpDeploymentProviderImpl {
    fn load_config(&self) -> impl Future<Output = McpRegistryResult<ServicesConfig>> + Send {
        ready(Ok((*self.deployments.load_config()).clone()))
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
