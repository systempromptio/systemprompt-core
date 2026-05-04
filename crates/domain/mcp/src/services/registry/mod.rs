pub mod manager;
pub mod trait_impl;
pub mod validator;

use crate::error::{McpDomainError, McpDomainResult};
use crate::services::registry::manager::RegistryService;

#[derive(Debug, Clone, Copy)]
pub struct RegistryManager;

impl RegistryManager {
    pub fn validate() -> McpDomainResult<()> {
        RegistryService::validate_registry()
    }

    pub fn get_enabled_servers() -> McpDomainResult<Vec<crate::McpServerConfig>> {
        RegistryService::get_enabled_servers_as_config()
    }

    pub fn reload() -> McpDomainResult<()> {
        RegistryService::validate_registry()
    }

    pub fn find_server(name: &str) -> McpDomainResult<Option<crate::McpServerConfig>> {
        let servers = RegistryService::get_enabled_servers_as_config()?;
        Ok(servers.into_iter().find(|s| s.name == name))
    }

    pub fn get_server(name: &str) -> McpDomainResult<crate::McpServerConfig> {
        Self::find_server(name)?.ok_or_else(|| {
            McpDomainError::ServerNotFound(format!("MCP server '{name}' not found in registry"))
        })
    }
}

pub type McpServerRegistry = RegistryManager;
