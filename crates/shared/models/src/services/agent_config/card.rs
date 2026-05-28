//! Agent capability and skill descriptors plus the container
//! [`AgentCardConfig`] published as the agent's public face.

use serde::{Deserialize, Serialize};

use crate::ai::ToolModelOverrides;
use crate::auth::{JwtAudience, Permission};
use crate::services::plugin::PluginComponentRef;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCardConfig {
    pub protocol_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub display_name: String,
    pub description: String,
    pub version: String,
    #[serde(default = "default_transport")]
    pub preferred_transport: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<AgentProviderInfo>,
    #[serde(default)]
    pub capabilities: CapabilitiesConfig,
    #[serde(default = "default_input_modes")]
    pub default_input_modes: Vec<String>,
    #[serde(default = "default_output_modes")]
    pub default_output_modes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_schemes: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<Vec<serde_json::Value>>,
    /// DEPRECATED: A2A `card.skills` is COMPUTED at A2A serve time by joining
    /// `metadata.skills` against the on-disk skill catalog under
    /// `services/skills/`. Authoring `card.skills` in agent YAML is a no-op for
    /// the A2A endpoint and the bridge marketplace as of the skill-catalog
    /// refactor.
    ///
    /// The field is tolerated (rather than rejected) so downstream repos can
    /// land their YAML cleanup in a follow-up commit without breaking
    /// deserialisation. It is `#[serde(skip_serializing)]` so re-emitted YAML
    /// no longer carries it, and a warning is logged at services-config load
    /// time when the vector is non-empty (see
    /// `infra/loader/src/config_loader/merge.
    /// rs::warn_on_authored_card_skills`).
    ///
    /// To be hard-removed once all downstream YAML has been migrated.
    #[serde(default, skip_serializing)]
    pub skills: Vec<AgentSkillConfig>,
    #[serde(default)]
    pub supports_authenticated_extended_card: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSkillConfig {
    pub id: systemprompt_identifiers::SkillId,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_modes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_modes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProviderInfo {
    pub organization: String,
    pub url: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitiesConfig {
    #[serde(default = "default_true")]
    pub streaming: bool,
    #[serde(default)]
    pub push_notifications: bool,
    #[serde(default = "default_true")]
    pub state_transition_history: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMetadataConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub mcp_servers: PluginComponentRef,
    #[serde(default)]
    pub skills: PluginComponentRef,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(default)]
    pub tool_model_overrides: ToolModelOverrides,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub scopes: Vec<Permission>,
    #[serde(default = "default_audience")]
    pub audience: JwtAudience,
}

impl Default for CapabilitiesConfig {
    fn default() -> Self {
        Self {
            streaming: true,
            push_notifications: false,
            state_transition_history: true,
        }
    }
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            required: false,
            scopes: Vec::new(),
            audience: JwtAudience::A2a,
        }
    }
}

pub(super) fn default_transport() -> String {
    "JSONRPC".to_owned()
}

pub(super) fn default_input_modes() -> Vec<String> {
    vec!["text/plain".to_owned()]
}

pub(super) fn default_output_modes() -> Vec<String> {
    vec!["text/plain".to_owned()]
}

pub(super) const fn default_true() -> bool {
    true
}

pub(super) const fn default_audience() -> JwtAudience {
    JwtAudience::A2a
}
