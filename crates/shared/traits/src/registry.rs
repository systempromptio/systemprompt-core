//! Registry provider traits for agents and MCP servers.
//!
//! `McpRegistryProvider` is dispatched as a trait object
//! (`dyn McpRegistryProvider`), so it uses `#[async_trait]`; native `async fn`
//! in traits is not yet `dyn`-compatible. `AgentRegistryProvider` is only
//! used through concrete types and declares native `async` methods.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::future::Future;

use crate::BoxedSource;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RegistryError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Registry unavailable: {0}")]
    Unavailable(#[source] BoxedSource),

    #[error("Configuration error: {0}")]
    Configuration(#[source] BoxedSource),

    #[error("Internal error: {0}")]
    Internal(#[source] BoxedSource),
}

#[derive(Debug, Clone)]
pub struct ServiceOAuthConfig {
    pub required: bool,
    pub scopes: Vec<String>,
    pub audience: String,
    pub ema: bool,
}

impl Default for ServiceOAuthConfig {
    fn default() -> Self {
        Self {
            required: true,
            scopes: Vec::new(),
            audience: String::new(),
            ema: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub name: String,
    pub port: u16,
    pub enabled: bool,
    pub oauth: ServiceOAuthConfig,
}

#[derive(Debug, Clone)]
pub struct McpServerInfo {
    pub name: String,
    pub port: Option<u16>,
    pub enabled: bool,
    pub oauth: ServiceOAuthConfig,
}

pub trait AgentRegistryProvider: Send + Sync {
    fn get_agent(
        &self,
        name: &str,
    ) -> impl Future<Output = Result<AgentInfo, RegistryError>> + Send;

    fn list_enabled_agents(
        &self,
    ) -> impl Future<Output = Result<Vec<AgentInfo>, RegistryError>> + Send;

    fn get_default_agent(&self) -> impl Future<Output = Result<AgentInfo, RegistryError>> + Send;

    fn agent_exists(&self, name: &str) -> impl Future<Output = Result<bool, RegistryError>> + Send {
        async move {
            match self.get_agent(name).await {
                Ok(_) => Ok(true),
                Err(RegistryError::NotFound(_)) => Ok(false),
                Err(e) => Err(e),
            }
        }
    }
}

#[async_trait]
pub trait McpRegistryProvider: Send + Sync {
    async fn get_server(&self, name: &str) -> Result<McpServerInfo, RegistryError>;

    async fn list_enabled_servers(&self) -> Result<Vec<McpServerInfo>, RegistryError>;

    async fn server_exists(&self, name: &str) -> Result<bool, RegistryError> {
        match self.get_server(name).await {
            Ok(_) => Ok(true),
            Err(RegistryError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }
}
