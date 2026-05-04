use crate::Deployment;
use crate::error::{McpDomainError, McpDomainResult};
use systemprompt_loader::ConfigLoader;
use systemprompt_models::ServicesConfig;

fn missing_deployment(name: &str) -> McpDomainError {
    McpDomainError::Configuration(format!(
        "No deployment configuration found for server: {name}"
    ))
}

/// Resolves MCP service deployment configuration from the loader.
#[derive(Debug, Clone, Copy)]
pub struct DeploymentService;

impl DeploymentService {
    /// Load the full services configuration.
    pub fn load_config() -> McpDomainResult<ServicesConfig> {
        ConfigLoader::load().map_err(Into::into)
    }

    /// Look up a single deployment by name.
    pub fn get_deployment(name: &str) -> McpDomainResult<Deployment> {
        let config = Self::load_config()?;
        config
            .mcp_servers
            .get(name)
            .cloned()
            .ok_or_else(|| missing_deployment(name))
    }

    /// List names of all enabled servers.
    pub fn list_enabled_servers() -> McpDomainResult<Vec<String>> {
        let config = Self::load_config()?;
        Ok(config
            .mcp_servers
            .iter()
            .filter(|(_, deployment)| deployment.enabled)
            .map(|(name, _)| name.clone())
            .collect())
    }

    /// Look up the port for a deployment.
    pub fn get_server_port(name: &str) -> McpDomainResult<u16> {
        let config = Self::load_config()?;
        config
            .mcp_servers
            .get(name)
            .map(|d| d.port)
            .ok_or_else(|| missing_deployment(name))
    }

    /// Check whether the named server is enabled.
    pub fn is_server_enabled(name: &str) -> McpDomainResult<bool> {
        let config = Self::load_config()?;
        config
            .mcp_servers
            .get(name)
            .map(|d| d.enabled)
            .ok_or_else(|| missing_deployment(name))
    }

    /// Validate the loaded services configuration.
    pub fn validate_config() -> McpDomainResult<()> {
        let config = ConfigLoader::load()?;
        config.validate()?;
        Ok(())
    }

    /// Look up the binary path for a deployment.
    pub fn get_server_binary(name: &str) -> McpDomainResult<String> {
        let config = Self::load_config()?;
        config
            .mcp_servers
            .get(name)
            .map(|d| d.binary.clone())
            .ok_or_else(|| missing_deployment(name))
    }

    /// Look up the package name for a deployment, falling back to the server
    /// name.
    pub fn get_server_package(name: &str) -> McpDomainResult<String> {
        let config = Self::load_config()?;
        config
            .mcp_servers
            .get(name)
            .and_then(|d| d.package.clone())
            .or_else(|| Some(name.to_string()))
            .ok_or_else(|| missing_deployment(name))
    }
}
