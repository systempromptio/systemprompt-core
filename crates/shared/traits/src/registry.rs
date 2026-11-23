//! Registry provider traits for agents and MCP servers.
//!
//! `McpRegistryProvider` is dispatched as a trait object
//! (`dyn McpRegistryProvider`), so it uses `#[async_trait]`; native `async fn`
//! in traits is not yet `dyn`-compatible. Its provided `oauth_scopes` reads
//! the scopes a server requires OAuth for (none when OAuth is off or lists no
//! scope); `oauth_scopes_for_resource` does so for an RFC 8707 resource URL
//! naming `/api/v1/mcp/{server}/mcp`. `AgentRegistryProvider` reads an
//! in-memory registry and is synchronous.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_identifiers::McpServerId;

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
    pub version: String,
    pub description: String,
    pub port: Option<u16>,
    pub display_in_web: bool,
    pub enabled: bool,
    pub oauth: ServiceOAuthConfig,
}

pub trait AgentRegistryProvider: Send + Sync {
    fn get_agent(&self, name: &str) -> Result<AgentInfo, RegistryError>;

    fn list_enabled_agents(&self) -> Result<Vec<AgentInfo>, RegistryError>;

    fn get_default_agent(&self) -> Result<AgentInfo, RegistryError>;

    fn agent_exists(&self, name: &str) -> Result<bool, RegistryError> {
        match self.get_agent(name) {
            Ok(_) => Ok(true),
            Err(RegistryError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }
}

#[async_trait]
pub trait McpRegistryProvider: Send + Sync {
    async fn get_server(&self, name: &str) -> Result<McpServerInfo, RegistryError>;

    async fn list_enabled_servers(&self) -> Result<Vec<McpServerInfo>, RegistryError>;

    async fn managed_servers(&self) -> Result<Vec<McpServerInfo>, RegistryError>;

    async fn validate(&self) -> Result<(), RegistryError>;

    async fn server_exists(&self, name: &str) -> Result<bool, RegistryError> {
        match self.get_server(name).await {
            Ok(_) => Ok(true),
            Err(RegistryError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    async fn oauth_scopes(&self, server: &McpServerId) -> Option<Vec<String>> {
        match self.get_server(server.as_str()).await {
            Ok(info) if info.oauth.required && !info.oauth.scopes.is_empty() => {
                Some(info.oauth.scopes)
            },
            _ => None,
        }
    }

    async fn oauth_scopes_for_resource(&self, resource_uri: &str) -> Option<Vec<String>> {
        let uri = http::Uri::try_from(resource_uri).ok()?;
        uri.scheme()?;
        let parts: Vec<&str> = uri.path().split('/').collect();
        if parts.len() < 6 || parts[1] != "api" || parts[3] != "mcp" || parts[5] != "mcp" {
            return None;
        }
        let server = McpServerId::try_new(parts[4]).ok()?;
        self.oauth_scopes(&server).await
    }
}
