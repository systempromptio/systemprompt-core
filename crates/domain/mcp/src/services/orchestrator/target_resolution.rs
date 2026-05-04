use anyhow::Result;

use super::McpOrchestrator;
use crate::McpServerConfig;
use crate::services::registry::RegistryManager;

impl McpOrchestrator {
    pub(super) async fn get_target_servers(
        &self,
        service_name: Option<String>,
        enabled_only: bool,
    ) -> Result<Vec<McpServerConfig>> {
        match service_name {
            Some(name) if name == "all" => {
                if enabled_only {
                    RegistryManager::get_enabled_servers().map_err(Into::into)
                } else {
                    self.database
                        .get_running_servers()
                        .await
                        .map_err(Into::into)
                }
            },
            Some(name) => {
                let servers = RegistryManager::get_enabled_servers()?;
                Ok(servers.into_iter().filter(|s| s.name == name).collect())
            },
            None => {
                if enabled_only {
                    RegistryManager::get_enabled_servers().map_err(Into::into)
                } else {
                    self.database
                        .get_running_servers()
                        .await
                        .map_err(Into::into)
                }
            },
        }
    }
}
