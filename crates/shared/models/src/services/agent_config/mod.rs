//! Agent configuration models — the on-disk YAML shape, the runtime
//! shape, and the lightweight summary projection.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod card;
mod disk;
mod summary;

pub use card::{
    AgentCardConfig, AgentMetadataConfig, AgentProviderInfo, CapabilitiesConfig, OAuthConfig,
};
pub use disk::DiskAgentConfig;
pub use summary::AgentSummary;

use crate::auth::Permission;
use crate::errors::ConfigValidationError;
use serde::{Deserialize, Serialize};

pub const AGENT_CONFIG_FILENAME: &str = "config.yaml";
pub const DEFAULT_AGENT_SYSTEM_PROMPT_FILE: &str = "system_prompt.md";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub port: u16,
    pub endpoint: String,
    pub enabled: bool,
    #[serde(default)]
    pub dev_only: bool,
    #[serde(default)]
    pub is_primary: bool,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    pub card: AgentCardConfig,
    pub metadata: AgentMetadataConfig,
    #[serde(default)]
    pub oauth: OAuthConfig,
}

impl AgentConfig {
    pub fn validate(&self, name: &str) -> Result<(), ConfigValidationError> {
        if self.name != name {
            return Err(ConfigValidationError::invalid_field(format!(
                "Agent config key '{}' does not match name field '{}'",
                name, self.name
            )));
        }

        if !self
            .name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(ConfigValidationError::invalid_field(format!(
                "Agent name '{}' must be lowercase alphanumeric with underscores only",
                self.name
            )));
        }

        if self.name.len() < 3 || self.name.len() > 50 {
            return Err(ConfigValidationError::invalid_field(format!(
                "Agent name '{}' must be between 3 and 50 characters",
                self.name
            )));
        }

        if self.port == 0 {
            return Err(ConfigValidationError::invalid_field(format!(
                "Agent '{}' has invalid port {}",
                self.name, self.port
            )));
        }

        Ok(())
    }

    pub fn extract_oauth_scopes_from_card(&mut self) {
        let Some(security_vec) = &self.card.security else {
            return;
        };
        for security_obj in security_vec {
            let Some(oauth2_scopes) = security_obj.get("oauth2") else {
                continue;
            };
            let permissions: Vec<Permission> = oauth2_scopes
                .iter()
                .filter_map(|scope| match scope.as_str() {
                    "admin" => Some(Permission::Admin),
                    "user" => Some(Permission::User),
                    "service" => Some(Permission::Service),
                    "a2a" => Some(Permission::A2a),
                    "mcp" => Some(Permission::Mcp),
                    "anonymous" => Some(Permission::Anonymous),
                    _ => None,
                })
                .collect();
            if !permissions.is_empty() {
                self.oauth.scopes = permissions;
                self.oauth.required = true;
            }
        }
    }

    #[must_use]
    pub fn construct_url(&self, base_url: &str) -> String {
        format!(
            "{}/api/v1/agents/{}",
            base_url.trim_end_matches('/'),
            self.name
        )
    }
}
