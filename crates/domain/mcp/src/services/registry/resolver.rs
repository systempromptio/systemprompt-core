use std::path::{Path, PathBuf};
use systemprompt_identifiers::UserId;
use systemprompt_loader::ExtensionRegistry;
use systemprompt_models::Config;
use systemprompt_models::mcp::McpServerType;

use crate::error::{McpDomainError, McpDomainResult};
use crate::services::deployment::DeploymentService;

#[derive(Debug, Clone)]
pub struct RegistryResolver {
    owner: UserId,
}

impl RegistryResolver {
    #[must_use]
    pub const fn new(owner: UserId) -> Self {
        Self { owner }
    }

    #[must_use]
    pub const fn owner(&self) -> &UserId {
        &self.owner
    }

    pub fn get_enabled_servers_as_config(&self) -> McpDomainResult<Vec<crate::McpServerConfig>> {
        use systemprompt_loader::ConfigLoader;

        let global_config = Config::get()?;
        let registry = ExtensionRegistry::build(
            Path::new(&global_config.system_path),
            global_config.is_cloud,
            &global_config.bin_path,
        );
        let services_config = ConfigLoader::load()?;
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

            let crate_path = match deployment.server_type {
                McpServerType::Internal => registry.get_path(&deployment.binary)?,
                McpServerType::External => PathBuf::new(),
            };

            let display_name = deployment
                .package
                .clone()
                .unwrap_or_else(|| deployment.binary.clone());

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
            };
            enabled.push(config);
        }

        Ok(enabled)
    }

    /// Exposes deployment-config validation through the registry handle so
    /// callers don't reach into `DeploymentService` directly.
    #[expect(
        clippy::unused_self,
        reason = "registry-scoped entry-point over DeploymentService::validate_config"
    )]
    pub fn validate(&self) -> McpDomainResult<()> {
        DeploymentService::validate_config()?;
        Ok(())
    }

    pub fn get_enabled_servers(&self) -> McpDomainResult<Vec<crate::McpServerConfig>> {
        self.get_enabled_servers_as_config()
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
