//! Runtime configuration for agent services: connection, runtime, and service
//! settings, plus a builder and validation for assembling them.

use crate::services::shared::error::{AgentServiceError, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use systemprompt_identifiers::AgentId;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ServiceConfiguration {
    pub enabled: bool,
    pub timeout_seconds: u64,
    pub retry_attempts: u32,
    pub retry_delay_milliseconds: u64,
    pub max_connections: usize,
}

impl ServiceConfiguration {
    pub const fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_seconds)
    }

    pub const fn retry_delay(&self) -> Duration {
        Duration::from_millis(self.retry_delay_milliseconds)
    }

    pub fn validate(&self) -> Result<()> {
        if self.retry_attempts == 0 {
            return Err(AgentServiceError::Configuration(
                "ServiceConfiguration".to_owned(),
                "retry_attempts must be at least 1".to_owned(),
            ));
        }
        if self.max_connections == 0 {
            return Err(AgentServiceError::Configuration(
                "ServiceConfiguration".to_owned(),
                "max_connections must be at least 1".to_owned(),
            ));
        }
        Ok(())
    }
}

impl Default for ServiceConfiguration {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout_seconds: 30,
            retry_attempts: 3,
            retry_delay_milliseconds: 500,
            max_connections: 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfiguration {
    pub agent_id: AgentId,
    pub name: String,
    pub port: u16,
    pub host: String,
    pub ssl_enabled: bool,
    pub auth_required: bool,
    pub system_prompt: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RuntimeConfigurationBuilder {
    agent_id: AgentId,
    name: String,
    port: u16,
    host: String,
    ssl_enabled: bool,
    auth_required: bool,
    system_prompt: Option<String>,
}

impl RuntimeConfigurationBuilder {
    pub fn new(agent_id: AgentId, name: String) -> Self {
        Self {
            agent_id,
            name,
            port: 8080,
            host: "localhost".to_owned(),
            ssl_enabled: false,
            auth_required: false,
            system_prompt: None,
        }
    }

    pub const fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn host(mut self, host: String) -> Self {
        self.host = host;
        self
    }

    pub const fn enable_ssl(mut self) -> Self {
        self.ssl_enabled = true;
        self
    }

    pub const fn require_auth(mut self) -> Self {
        self.auth_required = true;
        self
    }

    pub fn system_prompt(mut self, prompt: String) -> Self {
        self.system_prompt = Some(prompt);
        self
    }

    pub fn build(self) -> RuntimeConfiguration {
        RuntimeConfiguration {
            agent_id: self.agent_id,
            name: self.name,
            port: self.port,
            host: self.host,
            ssl_enabled: self.ssl_enabled,
            auth_required: self.auth_required,
            system_prompt: self.system_prompt,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfiguration {
    pub url: String,
    pub timeout_seconds: u64,
    pub keepalive_enabled: bool,
    pub pool_size: usize,
}

impl ConnectionConfiguration {
    pub const fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_seconds)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentServiceConfig {
    pub agent_id: AgentId,
    pub name: String,
    pub description: String,
    pub version: String,
    pub endpoint: String,
    pub port: u16,
    pub is_active: bool,
}

impl AgentServiceConfig {
    pub fn validate(&self) -> Result<()> {
        if self.agent_id.as_str().is_empty() {
            return Err(AgentServiceError::Validation(
                "agent_id".to_owned(),
                "cannot be empty".to_owned(),
            ));
        }
        if self.port == 0 {
            return Err(AgentServiceError::Validation(
                "port".to_owned(),
                "must be greater than 0".to_owned(),
            ));
        }
        if self.name.is_empty() {
            return Err(AgentServiceError::Validation(
                "name".to_owned(),
                "cannot be empty".to_owned(),
            ));
        }
        Ok(())
    }
}

impl Default for AgentServiceConfig {
    fn default() -> Self {
        Self {
            agent_id: AgentId::generate(),
            name: "Default Agent".to_owned(),
            description: "Default agent instance".to_owned(),
            version: "0.1.0".to_owned(),
            endpoint: "http://localhost:8080".to_owned(),
            port: 8080,
            is_active: true,
        }
    }
}
