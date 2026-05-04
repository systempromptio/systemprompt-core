//! Lightweight projection of [`AgentConfig`] used by listing endpoints
//! and CLI tables.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::AgentId;

use super::AgentConfig;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentSummary {
    pub agent_id: AgentId,
    pub name: String,
    pub display_name: String,
    pub port: u16,
    pub enabled: bool,
    pub is_primary: bool,
    pub is_default: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl AgentSummary {
    #[must_use]
    pub fn from_config(name: &str, config: &AgentConfig) -> Self {
        Self {
            agent_id: AgentId::new(name),
            name: name.to_string(),
            display_name: config.card.display_name.clone(),
            port: config.port,
            enabled: config.enabled,
            is_primary: config.is_primary,
            is_default: config.default,
            tags: config.tags.clone(),
        }
    }
}

impl From<&AgentConfig> for AgentSummary {
    fn from(config: &AgentConfig) -> Self {
        Self {
            agent_id: AgentId::new(config.name.clone()),
            name: config.name.clone(),
            display_name: config.card.display_name.clone(),
            port: config.port,
            enabled: config.enabled,
            is_primary: config.is_primary,
            is_default: config.default,
            tags: config.tags.clone(),
        }
    }
}
