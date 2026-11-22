//! [`RegistryService`] as the shared [`McpRegistryProvider`] seam.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_traits::{McpRegistryProvider, McpServerInfo, RegistryError};

use super::RegistryService;
use super::resolver::{registry_error, server_info};

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

    async fn managed_servers(&self) -> Result<Vec<McpServerInfo>, RegistryError> {
        let servers = Self::get_managed_servers(self).map_err(registry_error)?;
        Ok(servers.into_iter().map(server_info).collect())
    }

    async fn validate(&self) -> Result<(), RegistryError> {
        Self::validate(self).map_err(registry_error)
    }
}
