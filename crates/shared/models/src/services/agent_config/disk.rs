//! On-disk YAML shape of an agent's `config.yaml` and its projection
//! into the runtime [`super::AgentConfig`] shape.

use serde::Deserialize;
use systemprompt_identifiers::AgentId;

use super::card::{AgentCardConfig, AgentMetadataConfig, OAuthConfig, default_true};
use super::{AgentConfig, DEFAULT_AGENT_SYSTEM_PROMPT_FILE};
use crate::errors::ConfigValidationError;

fn default_version() -> String {
    "1.0.0".to_owned()
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiskAgentConfig {
    #[serde(default)]
    pub id: Option<AgentId>,
    pub name: String,
    pub display_name: String,
    pub description: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub port: u16,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub dev_only: bool,
    #[serde(default)]
    pub is_primary: bool,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub system_prompt_file: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub mcp_servers: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    pub card: AgentCardConfig,
    #[serde(default)]
    pub oauth: OAuthConfig,
}

impl DiskAgentConfig {
    #[must_use]
    pub fn system_prompt_file(&self) -> &str {
        self.system_prompt_file
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_AGENT_SYSTEM_PROMPT_FILE)
    }

    #[must_use]
    pub fn to_agent_config(&self, base_url: &str, system_prompt: Option<String>) -> AgentConfig {
        let endpoint = self.endpoint.clone().unwrap_or_else(|| {
            format!(
                "{}/api/v1/agents/{}",
                base_url.trim_end_matches('/'),
                self.name
            )
        });

        let card_name = self
            .card
            .name
            .clone()
            .unwrap_or_else(|| self.display_name.clone());

        AgentConfig {
            name: self.name.clone(),
            port: self.port,
            endpoint,
            tags: self.tags.clone(),
            enabled: self.enabled,
            dev_only: self.dev_only,
            is_primary: self.is_primary,
            default: self.default,
            card: AgentCardConfig {
                name: Some(card_name),
                ..self.card.clone()
            },
            metadata: AgentMetadataConfig {
                system_prompt,
                mcp_servers: self.mcp_servers.clone(),
                skills: self.skills.clone(),
                provider: self.provider.clone(),
                model: self.model.clone(),
                ..Default::default()
            },
            oauth: self.oauth.clone(),
        }
    }

    pub fn validate(&self, dir_name: &str) -> Result<(), ConfigValidationError> {
        if let Some(id) = &self.id
            && id.as_str() != dir_name
        {
            return Err(ConfigValidationError::invalid_field(format!(
                "Agent config id '{id}' does not match directory name '{dir_name}'"
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

        if self.display_name.is_empty() {
            return Err(ConfigValidationError::required(format!(
                "Agent '{}' display_name must not be empty",
                self.name
            )));
        }

        Ok(())
    }
}
