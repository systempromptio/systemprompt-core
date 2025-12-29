use anyhow::{anyhow, Result};
use systemprompt_loader::ConfigLoader;
use systemprompt_models::ServicesConfig;

use crate::Deployment;

#[derive(Debug, Clone, Copy)]
pub struct DeploymentService;

impl DeploymentService {
    pub fn load_config() -> Result<ServicesConfig> {
        ConfigLoader::load()
    }

    pub fn get_deployment(name: &str) -> Result<Deployment> {
        let config = Self::load_config()?;
        config
            .mcp_servers
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow!("No deployment configuration found for server: {name}"))
    }

    pub fn list_enabled_servers() -> Result<Vec<String>> {
        let config = Self::load_config()?;
        let enabled: Vec<String> = config
            .mcp_servers
            .iter()
            .filter(|(_, deployment)| deployment.enabled)
            .map(|(name, _)| name.clone())
            .collect();

        Ok(enabled)
    }

    pub fn get_server_port(name: &str) -> Result<u16> {
        let config = Self::load_config()?;
        config
            .mcp_servers
            .get(name)
            .map(|d| d.port)
            .ok_or_else(|| anyhow!("No deployment configuration found for server: {name}"))
    }

    pub fn is_server_enabled(name: &str) -> Result<bool> {
        let config = Self::load_config()?;
        config
            .mcp_servers
            .get(name)
            .map(|d| d.enabled)
            .ok_or_else(|| anyhow!("No deployment configuration found for server: {name}"))
    }

    pub fn validate_config() -> Result<()> {
        let config = ConfigLoader::load()?;
        config.validate()?;
        Ok(())
    }

    pub fn get_server_binary(name: &str) -> Result<String> {
        let config = Self::load_config()?;
        config
            .mcp_servers
            .get(name)
            .map(|d| d.binary.clone())
            .ok_or_else(|| anyhow!("No deployment configuration found for server: {name}"))
    }

    pub fn get_server_package(name: &str) -> Result<String> {
        let config = Self::load_config()?;
        config
            .mcp_servers
            .get(name)
            .and_then(|d| d.package.clone())
            .or_else(|| Some(name.to_string()))
            .ok_or_else(|| anyhow!("No deployment configuration found for server: {name}"))
    }
}
