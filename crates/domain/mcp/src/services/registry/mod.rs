pub mod manager;
pub mod trait_impl;
pub mod validator;

use crate::services::registry::manager::RegistryService;
use anyhow::Result;

#[derive(Debug, Clone, Copy)]
pub struct RegistryManager;

impl RegistryManager {
    pub fn validate() -> Result<()> {
        RegistryService::validate_registry()
    }

    pub fn get_enabled_servers() -> Result<Vec<crate::McpServerConfig>> {
        RegistryService::get_enabled_servers_as_config()
    }

    pub fn reload() -> Result<()> {
        RegistryService::validate_registry()
    }

    pub fn find_server(name: &str) -> Result<Option<crate::McpServerConfig>> {
        let servers = RegistryService::get_enabled_servers_as_config()?;
        Ok(servers.into_iter().find(|s| s.name == name))
    }

    pub fn get_server(name: &str) -> Result<crate::McpServerConfig> {
        Self::find_server(name)?
            .ok_or_else(|| anyhow::anyhow!("MCP server '{name}' not found in registry"))
    }
}

pub type McpServerRegistry = RegistryManager;
