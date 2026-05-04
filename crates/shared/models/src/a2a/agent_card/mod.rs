//! A2A protocol agent card — the JSON document an agent publishes to
//! describe its identity, capabilities, transports, and skills.

mod extension;
mod skill;

pub use extension::{ARTIFACT_RENDERING_URI, AgentCapabilities, AgentExtension};
pub use skill::{AgentCardSignature, AgentInterface, AgentProvider, AgentSkill};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::security::{OAuth2Flow, OAuth2Flows, SecurityScheme};
use super::transport::ProtocolBinding;

/// A2A agent card document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentCard {
    /// Agent name as published.
    pub name: String,
    /// Free-form description.
    pub description: String,
    /// Transports the agent supports.
    pub supported_interfaces: Vec<AgentInterface>,
    /// Semver tag of the agent revision.
    pub version: String,
    /// Optional icon / logo URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    /// Optional provider attribution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<AgentProvider>,
    /// Optional documentation URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation_url: Option<String>,
    /// Capability flags advertised by the agent.
    pub capabilities: AgentCapabilities,
    /// OpenAPI-style security scheme catalogue.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_schemes: Option<HashMap<String, SecurityScheme>>,
    /// OpenAPI-style security requirement set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<Vec<HashMap<String, Vec<String>>>>,
    /// Default acceptable input MIME types.
    pub default_input_modes: Vec<String>,
    /// Default produced MIME types.
    pub default_output_modes: Vec<String>,
    /// Skills declared on the card.
    #[serde(default)]
    pub skills: Vec<AgentSkill>,
    /// Whether the agent supports the authenticated extended-card variant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_authenticated_extended_card: Option<bool>,
    /// Optional JWS signature blocks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signatures: Option<Vec<AgentCardSignature>>,
}

impl AgentCard {
    /// Open a new [`AgentCardBuilder`].
    #[must_use]
    pub fn builder(
        name: String,
        description: String,
        url: String,
        version: String,
    ) -> AgentCardBuilder {
        AgentCardBuilder::new(name, description, url, version)
    }

    /// First supported interface URL, if any.
    #[must_use]
    pub fn url(&self) -> Option<&str> {
        self.supported_interfaces.first().map(|i| i.url.as_str())
    }

    /// True when the card declares the `systemprompt:mcp-tools` extension.
    #[must_use]
    pub fn has_mcp_extension(&self) -> bool {
        self.capabilities
            .extensions
            .as_ref()
            .is_some_and(|exts| exts.iter().any(|ext| ext.uri == "systemprompt:mcp-tools"))
    }

    /// Append the `systemprompt:mcp-tools` extension if absent.
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

/// Builder for [`AgentCard`].
#[derive(Debug)]
pub struct AgentCardBuilder {
    agent_card: AgentCard,
}

impl AgentCardBuilder {
    /// Open a builder seeded with the four mandatory fields.
    #[must_use]
    pub fn new(name: String, description: String, url: String, version: String) -> Self {
        Self {
            agent_card: AgentCard {
                name,
                description,
                supported_interfaces: vec![AgentInterface {
                    url,
                    protocol_binding: ProtocolBinding::JsonRpc,
                    protocol_version: "1.0.0".to_string(),
                }],
                version,
                icon_url: None,
                provider: None,
                documentation_url: None,
                capabilities: AgentCapabilities::default(),
                security_schemes: None,
                security: None,
                default_input_modes: vec!["text/plain".to_string()],
                default_output_modes: vec!["text/plain".to_string()],
                skills: Vec::new(),
                supports_authenticated_extended_card: Some(false),
                signatures: None,
            },
        }
    }

    /// Append one [`AgentSkill`] per `(server_name, display, description,
    /// tags)` tuple and seed the standard set of MCP-related extensions.
    #[must_use]
    pub fn with_mcp_skills(
        mut self,
        mcp_servers: Vec<(String, String, String, Vec<String>)>,
    ) -> Self {
        for (server_name, display_name, description, tags) in mcp_servers {
            let skill = AgentSkill::from_mcp_server(server_name, display_name, description, tags);
            self.agent_card.skills.push(skill);
        }

        let mcp_extension = AgentExtension::mcp_tools_extension();
        let opencode_extension = AgentExtension::opencode_integration_extension();
        let artifact_rendering = AgentExtension::artifact_rendering_extension();

        self.agent_card.capabilities.extensions =
            Some(vec![mcp_extension, opencode_extension, artifact_rendering]);

        self
    }

    /// Mark the card as streaming-capable.
    #[must_use]
    pub const fn with_streaming(mut self) -> Self {
        self.agent_card.capabilities.streaming = Some(true);
        self
    }

    /// Mark the card as push-notification-capable.
    #[must_use]
    pub const fn with_push_notifications(mut self) -> Self {
        self.agent_card.capabilities.push_notifications = Some(true);
        self
    }

    /// Attach provider attribution.
    #[must_use]
    pub fn with_provider(mut self, organization: String, url: String) -> Self {
        self.agent_card.provider = Some(AgentProvider { organization, url });
        self
    }

    /// Register an `OAuth2` authorization-code security scheme on the card.
    #[must_use]
    pub fn with_oauth2_security(
        mut self,
        authorization_url: String,
        token_url: String,
        scopes: HashMap<String, String>,
    ) -> Self {
        let oauth2_flows = OAuth2Flows {
            authorization_code: Some(OAuth2Flow {
                authorization_url: Some(authorization_url),
                token_url: Some(token_url),
                refresh_url: None,
                scopes,
            }),
            implicit: None,
            password: None,
            client_credentials: None,
        };

        let oauth2_scheme = SecurityScheme::OAuth2 {
            flows: Box::new(oauth2_flows),
            description: Some("OAuth 2.0 authorization code flow for secure access".to_string()),
        };

        self.agent_card
            .security_schemes
            .get_or_insert_with(HashMap::new)
            .insert("oauth2".to_string(), oauth2_scheme);

        let mut authentication_requirement = HashMap::new();
        authentication_requirement.insert(
            "oauth2".to_string(),
            vec!["admin".to_string(), "user".to_string()],
        );

        self.agent_card
            .security
            .get_or_insert_with(Vec::new)
            .push(authentication_requirement);

        self
    }

    /// Finalize the builder.
    #[must_use]
    pub fn build(self) -> AgentCard {
        self.agent_card
    }
}
