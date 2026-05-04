//! Lightweight projection of [`AgentConfig`] used by listing endpoints
//! and CLI tables.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::AgentId;

use super::AgentConfig;

/// Lightweight projection of an [`AgentConfig`] suitable for listing
/// endpoints and CLI tables.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentSummary {
    /// Stable typed agent identifier.
    pub agent_id: AgentId,
    /// Stable agent name.
    pub name: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Listening port.
    pub port: u16,
    /// Whether the agent is enabled at startup.
    pub enabled: bool,
    /// Whether the agent is the deployment's primary.
    pub is_primary: bool,
    /// Whether the agent is the deployment's default fallback.
    pub is_default: bool,
    /// Tags for client-side filtering.
    #[serde(default)]
    pub tags: Vec<String>,
}

impl AgentSummary {
    /// Project an [`AgentConfig`] into an [`AgentSummary`] using `name`
    /// as the agent identifier.
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
