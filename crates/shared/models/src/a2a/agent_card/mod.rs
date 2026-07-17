//! A2A protocol agent card — the JSON document an agent publishes to
//! describe its identity, capabilities, transports, and skills.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod extension;
mod skill;

pub use extension::{ARTIFACT_RENDERING_URI, AgentCapabilities, AgentExtension};
pub use skill::{AgentCardSignature, AgentInterface, AgentProvider, AgentSkill};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::security::SecurityScheme;
use super::transport::ProtocolBinding;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentCard {
    pub name: String,
    pub description: String,
    pub supported_interfaces: Vec<AgentInterface>,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<AgentProvider>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation_url: Option<String>,
    pub capabilities: AgentCapabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_schemes: Option<HashMap<String, SecurityScheme>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<Vec<HashMap<String, Vec<String>>>>,
    pub default_input_modes: Vec<String>,
    pub default_output_modes: Vec<String>,
    #[serde(default)]
    pub skills: Vec<AgentSkill>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_authenticated_extended_card: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signatures: Option<Vec<AgentCardSignature>>,
}

impl AgentCard {
    #[must_use]
    pub fn builder(
        name: String,
        description: String,
        url: String,
        version: String,
    ) -> AgentCardBuilder {
        AgentCardBuilder::new(name, description, url, version)
    }

    #[must_use]
    pub fn url(&self) -> Option<&str> {
        self.supported_interfaces.first().map(|i| i.url.as_str())
    }

    #[must_use]
    pub fn has_mcp_extension(&self) -> bool {
        self.capabilities
            .extensions
            .as_ref()
            .is_some_and(|exts| exts.iter().any(|ext| ext.uri == "systemprompt:mcp-tools"))
    }

    pub fn ensure_mcp_extension(&mut self) {
        if self.has_mcp_extension() {
            return;
        }

        self.capabilities
            .extensions
            .get_or_insert_with(Vec::new)
            .push(AgentExtension::mcp_tools_extension());
    }
}

#[derive(Debug)]
pub struct AgentCardBuilder {
    agent_card: AgentCard,
}

impl AgentCardBuilder {
    #[must_use]
    pub fn new(name: String, description: String, url: String, version: String) -> Self {
        Self {
            agent_card: AgentCard {
                name,
                description,
                supported_interfaces: vec![AgentInterface {
                    url,
                    protocol_binding: ProtocolBinding::JsonRpc,
                    protocol_version: "1.0.0".to_owned(),
                }],
                version,
                icon_url: None,
                provider: None,
                documentation_url: None,
                capabilities: AgentCapabilities::default(),
                security_schemes: None,
                security: None,
                default_input_modes: vec!["text/plain".to_owned()],
                default_output_modes: vec!["text/plain".to_owned()],
                skills: Vec::new(),
                supports_authenticated_extended_card: Some(false),
                signatures: None,
            },
        }
    }

    #[must_use]
    pub const fn with_streaming(mut self) -> Self {
        self.agent_card.capabilities.streaming = Some(true);
        self
    }

    #[must_use]
    pub const fn with_push_notifications(mut self) -> Self {
        self.agent_card.capabilities.push_notifications = Some(true);
        self
    }

    #[must_use]
    pub fn with_provider(mut self, organization: String, url: String) -> Self {
        self.agent_card.provider = Some(AgentProvider { organization, url });
        self
    }

    #[must_use]
    pub fn build(self) -> AgentCard {
        self.agent_card
    }
}
