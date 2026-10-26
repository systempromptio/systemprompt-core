//! Read-only access to MCP server deployment configuration.
//!
//! [`DeploymentService`] resolves per-server deployment records (port, binary,
//! enabled state) from the loaded `ServicesConfig`. All operations fail with
//! [`McpDomainError`] when the named server is absent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Deployment;
use crate::error::{McpDomainError, McpDomainResult};
use std::sync::Arc;
use systemprompt_manifest::ServicesConfig;
use systemprompt_manifest::services::ServicesView;

fn missing_deployment(name: &str) -> McpDomainError {
    McpDomainError::Configuration(format!(
        "No deployment configuration found for server: {name}"
    ))
}

#[derive(Debug, Clone)]
pub struct DeploymentService {
    services: ServicesView,
}

impl DeploymentService {
    #[must_use]
    pub const fn new(services: ServicesView) -> Self {
        Self { services }
    }

    #[must_use]
    pub fn load_config(&self) -> Arc<ServicesConfig> {
        self.services.config()
    }

    pub fn get_deployment(&self, name: &str) -> McpDomainResult<Deployment> {
        let config = self.load_config();
        config
            .mcp_servers
            .get(name)
            .cloned()
            .ok_or_else(|| missing_deployment(name))
    }

    #[must_use]
    pub fn list_enabled_servers(&self) -> Vec<String> {
        self.load_config()
            .mcp_servers
            .iter()
            .filter(|(_, deployment)| deployment.enabled)
            .map(|(name, _)| name.clone())
            .collect()
    }


    pub fn is_server_enabled(&self, name: &str) -> McpDomainResult<bool> {
        let config = self.load_config();
        config
            .mcp_servers
            .get(name)
            .map(|d| d.enabled)
            .ok_or_else(|| missing_deployment(name))
    }
}
