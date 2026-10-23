//! Agent capability and skill descriptors plus the container
//! [`AgentCardConfig`] published as the agent's public face.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};

use systemprompt_models::a2a::SecurityScheme;
use systemprompt_models::ai::ToolModelOverrides;
use systemprompt_models::auth::{JwtAudience, Permission};
use systemprompt_models::plugin::PluginComponentRef;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "AgentCardConfigWire")]
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
    #[serde(skip_serializing_if = "Option::is_none", with = "json_shaped_schemes")]
    pub security_schemes: Option<HashMap<String, SecurityScheme>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_requirements: Option<Vec<HashMap<String, Vec<String>>>>,
}

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
#[error("agent card key `{retired}` was removed in A2A 1.0; use `{replacement}`")]
pub struct RetiredCardKeyError {
    pub retired: &'static str,
    pub replacement: &'static str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentCardConfigWire {
    protocol_version: String,
    #[serde(default)]
    name: Option<String>,
    display_name: String,
    description: String,
    version: String,
    #[serde(default = "default_transport")]
    preferred_transport: String,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    documentation_url: Option<String>,
    #[serde(default)]
    provider: Option<AgentProviderInfo>,
    #[serde(default)]
    capabilities: CapabilitiesConfig,
    #[serde(default = "default_input_modes")]
    default_input_modes: Vec<String>,
    #[serde(default = "default_output_modes")]
    default_output_modes: Vec<String>,
    #[serde(default, with = "json_shaped_schemes")]
    security_schemes: Option<HashMap<String, SecurityScheme>>,
    #[serde(default)]
    security_requirements: Option<Vec<HashMap<String, Vec<String>>>>,
    #[serde(default)]
    security: Option<IgnoredAny>,
    #[serde(default)]
    supports_authenticated_extended_card: Option<IgnoredAny>,
}

impl TryFrom<AgentCardConfigWire> for AgentCardConfig {
    type Error = RetiredCardKeyError;

    fn try_from(wire: AgentCardConfigWire) -> Result<Self, Self::Error> {
        if wire.security.is_some() {
            return Err(RetiredCardKeyError {
                retired: "card.security",
                replacement: "card.securityRequirements",
            });
        }
        if wire.supports_authenticated_extended_card.is_some() {
            return Err(RetiredCardKeyError {
                retired: "card.supportsAuthenticatedExtendedCard",
                replacement: "card.capabilities.extendedAgentCard",
            });
        }
        Ok(Self {
            protocol_version: wire.protocol_version,
            name: wire.name,
            display_name: wire.display_name,
            description: wire.description,
            version: wire.version,
            preferred_transport: wire.preferred_transport,
            icon_url: wire.icon_url,
            documentation_url: wire.documentation_url,
            provider: wire.provider,
            capabilities: wire.capabilities,
            default_input_modes: wire.default_input_modes,
            default_output_modes: wire.default_output_modes,
            security_schemes: wire.security_schemes,
            security_requirements: wire.security_requirements,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProviderInfo {
    pub organization: String,
    pub url: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "CapabilitiesConfigWire")]
pub struct CapabilitiesConfig {
    pub streaming: bool,
    pub push_notifications: bool,
    pub extended_agent_card: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CapabilitiesConfigWire {
    #[serde(default = "default_true")]
    streaming: bool,
    #[serde(default)]
    push_notifications: bool,
    #[serde(default)]
    extended_agent_card: bool,
    #[serde(default)]
    state_transition_history: Option<IgnoredAny>,
}

impl TryFrom<CapabilitiesConfigWire> for CapabilitiesConfig {
    type Error = RetiredCardKeyError;

    fn try_from(wire: CapabilitiesConfigWire) -> Result<Self, Self::Error> {
        if wire.state_transition_history.is_some() {
            return Err(RetiredCardKeyError {
                retired: "card.capabilities.stateTransitionHistory",
                replacement: "nothing: A2A 1.0 always records task history; delete the key",
            });
        }
        Ok(Self {
            streaming: wire.streaming,
            push_notifications: wire.push_notifications,
            extended_agent_card: wire.extended_agent_card,
        })
    }
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
    // JSON: an agent-runs `RunRequest` template, opaque here. When set, the
    // A2A server queues each message as a headless agent run instead of
    // answering inline; the message becomes the template's `prompt`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_run: Option<serde_json::Value>,
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
            extended_agent_card: false,
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

// Why: serde_yaml 0.9 reads an externally tagged enum only as a `!tag`, while
// the A2A 1.0 security scheme is a single-key object; routing through JSON
// keeps the spec's `apiKeySecurityScheme: {...}` form in agent YAML.
mod json_shaped_schemes {
    use std::collections::HashMap;

    use serde::de::Error as _;
    use serde::ser::Error as _;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use systemprompt_models::a2a::SecurityScheme;

    #[derive(Debug, thiserror::Error)]
    #[error("securitySchemes.{name}: {source}")]
    struct InvalidSecuritySchemeError {
        name: String,
        #[source]
        source: serde_json::Error,
    }

    #[expect(
        clippy::ref_option,
        reason = "serde `with` fixes the serializer signature to &Option<T>"
    )]
    pub(super) fn serialize<S: Serializer>(
        schemes: &Option<HashMap<String, SecurityScheme>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        // JSON: the spec's single-key object form of each scheme.
        let shaped: Option<HashMap<&String, serde_json::Value>> = schemes
            .as_ref()
            .map(|map| {
                map.iter()
                    .map(|(name, scheme)| serde_json::to_value(scheme).map(|v| (name, v)))
                    .collect::<Result<_, _>>()
            })
            .transpose()
            .map_err(S::Error::custom)?;
        shaped.serialize(serializer)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<HashMap<String, SecurityScheme>>, D::Error> {
        // JSON: each scheme is read as its spec object, then typed.
        let raw = Option::<HashMap<String, serde_json::Value>>::deserialize(deserializer)?;
        raw.map(|map| {
            map.into_iter()
                .map(|(name, value)| match serde_json::from_value(value) {
                    Ok(scheme) => Ok((name, scheme)),
                    Err(source) => Err(D::Error::custom(InvalidSecuritySchemeError {
                        name,
                        source,
                    })),
                })
                .collect()
        })
        .transpose()
    }
}
