//! Agent capability and skill descriptors plus the container
//! [`AgentCardConfig`] published as the agent's public face.

use serde::{Deserialize, Serialize};

use crate::ai::ToolModelOverrides;
use crate::auth::{JwtAudience, Permission};

/// Marketing / capability descriptor exposed to clients via the agent
/// card endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCardConfig {
    /// A2A protocol version exposed in the card.
    pub protocol_version: String,
    /// Optional display name override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Human-readable agent name.
    pub display_name: String,
    /// Free-form description of what the agent does.
    pub description: String,
    /// Semver tag of the agent revision.
    pub version: String,
    /// Default A2A transport binding string (e.g. `JSONRPC`).
    #[serde(default = "default_transport")]
    pub preferred_transport: String,
    /// Optional URL to a logo / icon asset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    /// Optional URL to documentation for the agent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation_url: Option<String>,
    /// Optional provider attribution block.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<AgentProviderInfo>,
    /// Capability flags (streaming, push, history).
    #[serde(default)]
    pub capabilities: CapabilitiesConfig,
    /// Default acceptable input MIME types.
    #[serde(default = "default_input_modes")]
    pub default_input_modes: Vec<String>,
    /// Default produced MIME types.
    #[serde(default = "default_output_modes")]
    pub default_output_modes: Vec<String>,
    /// Free-form security scheme metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_schemes: Option<serde_json::Value>,
    /// Free-form security requirement set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<Vec<serde_json::Value>>,
    /// Skills declared on the card.
    #[serde(default)]
    pub skills: Vec<AgentSkillConfig>,
    /// Whether the agent supports the authenticated extended-card variant.
    #[serde(default)]
    pub supports_authenticated_extended_card: bool,
}

/// A single skill exposed by an agent on its card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSkillConfig {
    /// Stable identifier of the skill.
    pub id: systemprompt_identifiers::SkillId,
    /// Human-readable skill name.
    pub name: String,
    /// Free-form description.
    pub description: String,
    /// Tags for client-side filtering.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Optional example invocations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Vec<String>>,
    /// Optional acceptable input MIME types.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_modes: Option<Vec<String>>,
    /// Optional produced MIME types.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_modes: Option<Vec<String>>,
    /// Optional skill-scoped security requirements.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<Vec<serde_json::Value>>,
}

/// Provider attribution shown on agent cards.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProviderInfo {
    /// Organization that publishes the agent.
    pub organization: String,
    /// Canonical organisation URL.
    pub url: String,
}

/// Boolean capability flags advertised by an agent.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitiesConfig {
    /// Whether the agent supports streaming responses.
    #[serde(default = "default_true")]
    pub streaming: bool,
    /// Whether the agent emits push notifications.
    #[serde(default)]
    pub push_notifications: bool,
    /// Whether the agent records full state-transition history.
    #[serde(default = "default_true")]
    pub state_transition_history: bool,
}

/// Runtime metadata attached to an agent (system prompt, model
/// selection, MCP server bindings).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMetadataConfig {
    /// Resolved system prompt content (after `!include` processing).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Names of MCP servers this agent connects to.
    #[serde(default)]
    pub mcp_servers: Vec<String>,
    /// Names of skills this agent advertises.
    #[serde(default)]
    pub skills: Vec<String>,
    /// Optional provider override (e.g. `anthropic`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Optional model override (e.g. `claude-3-5-sonnet`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Optional max output token cap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    /// Per-tool model selection overrides.
    #[serde(default)]
    pub tool_model_overrides: ToolModelOverrides,
}

/// OAuth scope and audience requirements declared by an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    /// Whether OAuth is required to invoke this agent.
    #[serde(default)]
    pub required: bool,
    /// Permissions accepted as valid scopes.
    #[serde(default)]
    pub scopes: Vec<Permission>,
    /// Audience claim required on accepted JWTs.
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
    "JSONRPC".to_string()
}

pub(super) fn default_input_modes() -> Vec<String> {
    vec!["text/plain".to_string()]
}

pub(super) fn default_output_modes() -> Vec<String> {
    vec!["text/plain".to_string()]
}

pub(super) const fn default_true() -> bool {
    true
}

pub(super) const fn default_audience() -> JwtAudience {
    JwtAudience::A2a
}
