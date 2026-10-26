//! Implementation of `AgentRegistryProvider` trait for the agent module.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_manifest::services::ServicesView;
use systemprompt_traits::{AgentInfo, AgentRegistryProvider, RegistryError, ServiceOAuthConfig};

use super::registry::AgentRegistry;
use crate::error::AgentError;

#[derive(Debug, Clone)]
pub struct AgentRegistryProviderService {
    registry: AgentRegistry,
    is_cloud: bool,
}

impl AgentRegistryProviderService {
    #[must_use]
    pub fn new(services: &ServicesView, is_cloud: bool) -> Self {
        Self {
            registry: AgentRegistry::new(services),
            is_cloud,
        }
    }

    pub const fn from_registry(registry: AgentRegistry, is_cloud: bool) -> Self {
        Self { registry, is_cloud }
    }
}

impl AgentRegistryProvider for AgentRegistryProviderService {
    fn get_agent(&self, name: &str) -> Result<AgentInfo, RegistryError> {
        let agent = self.registry.get_agent(name).map_err(registry_error)?;

        Ok(AgentInfo {
            name: agent.name,
            port: agent.port,
            enabled: agent.enabled,
            oauth: ServiceOAuthConfig {
                required: agent.oauth.required,
                scopes: agent.oauth.scopes.iter().map(ToString::to_string).collect(),
                audience: agent.oauth.audience.to_string(),
                ema: false,
            },
        })
    }

    fn list_enabled_agents(&self) -> Result<Vec<AgentInfo>, RegistryError> {
        let agents = self.registry.list_enabled_agents(self.is_cloud);

        Ok(agents
            .into_iter()
            .map(|agent| AgentInfo {
                name: agent.name,
                port: agent.port,
                enabled: agent.enabled,
                oauth: ServiceOAuthConfig {
                    required: agent.oauth.required,
                    scopes: agent.oauth.scopes.iter().map(ToString::to_string).collect(),
                    audience: agent.oauth.audience.to_string(),
                    ema: false,
                },
            })
            .collect())
    }

    fn get_default_agent(&self) -> Result<AgentInfo, RegistryError> {
        let agent = self
            .registry
            .get_default_agent(self.is_cloud)
            .map_err(registry_error)?;

        Ok(AgentInfo {
            name: agent.name,
            port: agent.port,
            enabled: agent.enabled,
            oauth: ServiceOAuthConfig {
                required: agent.oauth.required,
                scopes: agent.oauth.scopes.iter().map(ToString::to_string).collect(),
                audience: agent.oauth.audience.to_string(),
                ema: false,
            },
        })
    }
}

fn registry_error(error: AgentError) -> RegistryError {
    match error {
        AgentError::NotFound(name) => RegistryError::NotFound(name),
        AgentError::EmptyCorsAllowlist
        | AgentError::InvalidConfig { .. }
        | AgentError::ServicesConfig(_) => RegistryError::Configuration(error.into()),
        other => RegistryError::Unavailable(other.into()),
    }
}
