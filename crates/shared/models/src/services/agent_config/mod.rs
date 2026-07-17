//! Agent configuration models — the on-disk YAML shape, the runtime
//! shape, and the lightweight summary projection.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod card;
mod disk;
mod summary;

pub use card::{
    AgentCardConfig, AgentMetadataConfig, AgentProviderInfo, AgentSkillConfig, CapabilitiesConfig,
    OAuthConfig,
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
        if let Some(security_vec) = &self.card.security {
            for security_obj in security_vec {
                if let Some(oauth2_scopes) = security_obj.get("oauth2").and_then(|v| v.as_array()) {
                    let mut permissions = Vec::new();
                    for scope_val in oauth2_scopes {
                        if let Some(scope_str) = scope_val.as_str() {
                            match scope_str {
                                "admin" => permissions.push(Permission::Admin),
                                "user" => permissions.push(Permission::User),
                                "service" => permissions.push(Permission::Service),
                                "a2a" => permissions.push(Permission::A2a),
                                "mcp" => permissions.push(Permission::Mcp),
                                "anonymous" => permissions.push(Permission::Anonymous),
                                _ => {},
                            }
                        }
                    }
                    if !permissions.is_empty() {
                        self.oauth.scopes = permissions;
                        self.oauth.required = true;
                    }
                }
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
