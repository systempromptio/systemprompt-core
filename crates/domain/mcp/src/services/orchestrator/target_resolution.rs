use crate::error::McpDomainResult;

use super::McpOrchestrator;
use crate::McpServerConfig;

impl McpOrchestrator {
    pub(super) async fn get_target_servers(
        &self,
        service_name: Option<String>,
        enabled_only: bool,
    ) -> McpDomainResult<Vec<McpServerConfig>> {
        match service_name {
            Some(name) if name == "all" => {
                if enabled_only {
                    self.registry().managed_servers()
                } else {
                    self.database().get_running_servers().await
                }
            },
            Some(name) => {
                let servers = self.registry().managed_servers()?;
                Ok(servers.into_iter().filter(|s| s.name == name).collect())
            },
            None => {
                if enabled_only {
                    self.registry().managed_servers()
                } else {
                    self.database().get_running_servers().await
                }
            },
        }
    }
}
