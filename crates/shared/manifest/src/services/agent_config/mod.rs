//! Agent configuration models — the on-disk YAML shape, the runtime
//! shape, and the lightweight summary projection.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod card;
mod disk;
mod json_shaped_schemes;
mod summary;

pub use card::{
    AgentCardConfig, AgentMetadataConfig, AgentProviderInfo, CapabilitiesConfig, OAuthConfig,
    RetiredCardKeyError,
};
pub use disk::DiskAgentConfig;
pub use summary::AgentSummary;

use serde::{Deserialize, Serialize};
use systemprompt_models::auth::{Permission, validate_required_scopes};
use systemprompt_models::errors::ServicesValidationError;
use systemprompt_models::subprocess::SecretGrant;

pub const AGENT_CONFIG_FILENAME: &str = "config.yaml";
pub const DEFAULT_AGENT_SYSTEM_PROMPT_FILE: &str = "system_prompt.md";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secrets: Vec<SecretGrant>,
}

impl AgentConfig {
    pub fn validate(&self, name: &str) -> Result<(), ServicesValidationError> {
        if self.name != name {
            return Err(ServicesValidationError::invalid_field(format!(
                "Agent config key '{}' does not match name field '{}'",
                name, self.name
            )));
        }

        if !self
            .name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(ServicesValidationError::invalid_field(format!(
                "Agent name '{}' must be lowercase alphanumeric with underscores only",
                self.name
            )));
        }

        if self.name.len() < 3 || self.name.len() > 50 {
            return Err(ServicesValidationError::invalid_field(format!(
                "Agent name '{}' must be between 3 and 50 characters",
                self.name
            )));
        }

        if self.port == 0 {
            return Err(ServicesValidationError::invalid_field(format!(
                "Agent '{}' has invalid port {}",
                self.name, self.port
            )));
        }

        validate_agent_grants(&self.name, &self.secrets)?;

        let card_scopes = self.card_oauth_scopes();
        validate_required_scopes(
            &format!("Agent '{}'", self.name),
            card_scopes.as_deref().unwrap_or(&self.oauth.scopes),
            self.oauth.service_only,
        )
    }

    pub fn extract_oauth_scopes_from_card(&mut self) {
        if let Some(permissions) = self.card_oauth_scopes() {
            self.oauth.scopes = permissions;
            self.oauth.required = true;
        }
    }

    fn card_oauth_scopes(&self) -> Option<Vec<Permission>> {
        let security_vec = self.card.security_requirements.as_ref()?;
        security_vec
            .iter()
            .rev()
            .filter_map(|security_obj| security_obj.get("oauth2"))
            .map(|oauth2_scopes| {
                oauth2_scopes
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
                    .collect::<Vec<_>>()
            })
            .find(|permissions| !permissions.is_empty())
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

pub fn validate_agent_grants(
    name: &str,
    secrets: &[SecretGrant],
) -> Result<(), ServicesValidationError> {
    secrets
        .iter()
        .find(|grant| matches!(grant, SecretGrant::Provider(_)))
        .map_or(Ok(()), |grant| {
            Err(ServicesValidationError::invalid_field(format!(
                "Agent '{name}': secrets names the provider key '{grant}'; an agent receives the \
                 keys of the providers its model configuration uses, so remove it"
            )))
        })
}
