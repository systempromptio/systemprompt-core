//! MCP server registry.
//!
//! Resolves configured servers from the loader config and adapts them onto
//! the `McpRegistry`, `McpToolProvider`, and `McpRegistryProvider` traits.
//!
//! The registry is owner-scoped: every `McpServerConfig` it materialises is
//! attributed to the [`UserId`] passed to [`RegistryManager::new`]. The
//! platform constructs one instance during `AppContext` bootstrap with the
//! resolved system-admin id; callers obtain it via `AppContext::mcp_registry`.

pub mod manager;
pub mod trait_impl;
pub mod validator;

use std::sync::Arc;
use systemprompt_identifiers::UserId;

use crate::error::{McpDomainError, McpDomainResult};
use crate::services::registry::manager::RegistryService;

#[derive(Debug, Clone)]
pub struct RegistryManager {
    service: Arc<RegistryService>,
}

impl RegistryManager {
    #[must_use]
    pub fn new(owner: UserId) -> Self {
        Self {
            service: Arc::new(RegistryService::new(owner)),
        }
    }

    pub fn validate(&self) -> McpDomainResult<()> {
        self.service.validate_registry()
    }

    pub fn get_enabled_servers(&self) -> McpDomainResult<Vec<crate::McpServerConfig>> {
        self.service.get_enabled_servers_as_config()
    }

    pub fn reload(&self) -> McpDomainResult<()> {
        self.service.validate_registry()
    }

    pub fn find_server(&self, name: &str) -> McpDomainResult<Option<crate::McpServerConfig>> {
        let servers = self.service.get_enabled_servers_as_config()?;
        Ok(servers.into_iter().find(|s| s.name == name))
    }

    pub fn get_server(&self, name: &str) -> McpDomainResult<crate::McpServerConfig> {
        self.find_server(name)?.ok_or_else(|| {
            McpDomainError::ServerNotFound(format!("MCP server '{name}' not found in registry"))
        })
    }
}

pub type McpServerRegistry = RegistryManager;
