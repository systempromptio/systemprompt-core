//! Agent configuration models — the on-disk YAML shape, the runtime
//! shape, and the lightweight summary projection.

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

/// Canonical filename for an agent's on-disk configuration.
pub const AGENT_CONFIG_FILENAME: &str = "config.yaml";
/// Canonical filename for an agent's system-prompt source.
pub const DEFAULT_AGENT_SYSTEM_PROMPT_FILE: &str = "system_prompt.md";

/// Runtime shape of a single agent's configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Stable agent name.
    pub name: String,
    /// Listening port.
    pub port: u16,
    /// Resolved public endpoint URL.
    pub endpoint: String,
    /// Whether the agent is enabled at startup.
    pub enabled: bool,
    /// Whether the agent is restricted to the `dev` profile.
    #[serde(default)]
    pub dev_only: bool,
    /// Whether this is the primary agent for the deployment.
    #[serde(default)]
    pub is_primary: bool,
    /// Whether the agent should be the default fallback.
    #[serde(default)]
    pub default: bool,
    /// Tags for client-side filtering.
    #[serde(default)]
    pub tags: Vec<String>,
    /// The agent card descriptor.
    pub card: AgentCardConfig,
    /// Runtime metadata.
    pub metadata: AgentMetadataConfig,
    /// OAuth scope and audience requirements.
    #[serde(default)]
    pub oauth: OAuthConfig,
}

impl AgentConfig {
    /// Validate the runtime configuration of a single agent.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigValidationError::InvalidField`] when the agent name does
    /// not match its map key, contains illegal characters, falls outside
    /// the supported length range, or specifies an invalid port.
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

    /// Promote any OAuth scopes encoded inside `card.security` into
    /// the dedicated [`OAuthConfig`] block.
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

    /// Build the canonical `<base_url>/api/v1/agents/<name>` URL for this
    /// agent.
    #[must_use]
    pub fn construct_url(&self, base_url: &str) -> String {
        format!(
            "{}/api/v1/agents/{}",
            base_url.trim_end_matches('/'),
            self.name
        )
    }
}
