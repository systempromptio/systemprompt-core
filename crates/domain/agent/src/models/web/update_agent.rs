use crate::models::a2a::AgentCard;
use serde::{Deserialize, Serialize};
use systemprompt_runtime::AppContext;

use super::card_input::AgentCardInput;
use super::validation::{extract_port_from_url, is_valid_version, list_available_mcp_servers};

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateAgentRequestRaw {
    pub card: AgentCardInput,
    pub is_active: Option<bool>,
    pub system_prompt: Option<String>,
    pub mcp_servers: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateAgentRequest {
    pub card: AgentCard,
    pub is_active: Option<bool>,
    pub system_prompt: Option<String>,
    pub mcp_servers: Option<Vec<String>>,
}

impl<'de> Deserialize<'de> for UpdateAgentRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = UpdateAgentRequestRaw::deserialize(deserializer)?;

        let url = raw
            .card
            .url
            .unwrap_or_else(|| format!("http://placeholder/api/v1/agents/{}", raw.card.name));

        let card = AgentCard {
            protocol_version: raw.card.protocol_version,
            name: raw.card.name,
            description: raw.card.description,
            url,
            version: raw.card.version,
            preferred_transport: raw.card.preferred_transport,
            additional_interfaces: None,
            icon_url: None,
            provider: None,
            documentation_url: None,
            capabilities: raw.card.capabilities.normalize(),
            security_schemes: raw.card.security_schemes,
            security: raw.card.security,
            default_input_modes: if raw.card.default_input_modes.is_empty() {
                vec!["text/plain".to_string()]
            } else {
                raw.card.default_input_modes
            },
            default_output_modes: if raw.card.default_output_modes.is_empty() {
                vec!["text/plain".to_string()]
            } else {
                raw.card.default_output_modes
            },
            skills: raw.card.skills,
            supports_authenticated_extended_card: None,
            signatures: None,
        };

        Ok(Self {
            card,
            is_active: raw.is_active,
            system_prompt: raw.system_prompt,
            mcp_servers: raw.mcp_servers,
        })
    }
}

impl UpdateAgentRequest {
    pub fn from_raw(raw: UpdateAgentRequestRaw, ctx: &AppContext) -> Self {
        let url = raw.card.url.unwrap_or_else(|| {
            let host = &ctx.config().api_server_url;
            format!("{}/api/v1/agents/{}", host, raw.card.name)
        });

        let card = AgentCard {
            protocol_version: raw.card.protocol_version,
            name: raw.card.name,
            description: raw.card.description,
            url,
            version: raw.card.version,
            preferred_transport: raw.card.preferred_transport,
            additional_interfaces: None,
            icon_url: None,
            provider: None,
            documentation_url: None,
            capabilities: raw.card.capabilities.normalize(),
            security_schemes: raw.card.security_schemes,
            security: raw.card.security,
            default_input_modes: if raw.card.default_input_modes.is_empty() {
                vec!["text/plain".to_string()]
            } else {
                raw.card.default_input_modes
            },
            default_output_modes: if raw.card.default_output_modes.is_empty() {
                vec!["text/plain".to_string()]
            } else {
                raw.card.default_output_modes
            },
            skills: raw.card.skills,
            supports_authenticated_extended_card: None,
            signatures: None,
        };

        Self {
            card,
            is_active: raw.is_active,
            system_prompt: raw.system_prompt,
            mcp_servers: raw.mcp_servers,
        }
    }

    pub async fn validate(&self) -> Result<(), String> {
        if self.card.name.trim().is_empty() {
            return Err("Name is required".to_string());
        }

        if self.card.url.trim().is_empty() {
            return Err("Endpoint is required".to_string());
        }

        if !self.card.url.starts_with("http://") && !self.card.url.starts_with("https://") {
            return Err("Endpoint must be a valid HTTP or HTTPS URL".to_string());
        }

        if !is_valid_version(&self.card.version) {
            return Err("Version must be in semantic version format (e.g., 1.0.0)".to_string());
        }

        if let Some(ref mcp_servers) = self.mcp_servers {
            if !mcp_servers.is_empty() {
                let available_servers = list_available_mcp_servers().await?;
                let mut invalid_servers = Vec::new();

                for server in mcp_servers {
                    if !available_servers.contains(server) {
                        invalid_servers.push(server.clone());
                    }
                }

                if !invalid_servers.is_empty() {
                    return Err(format!(
                        "Invalid MCP server(s): {}. Available servers: {}",
                        invalid_servers.join(", "),
                        if available_servers.is_empty() {
                            "(none)".to_string()
                        } else {
                            available_servers.join(", ")
                        }
                    ));
                }
            }
        }

        Ok(())
    }

    pub fn is_active(&self) -> bool {
        self.is_active.unwrap_or(true)
    }

    pub fn extract_port(&self) -> u16 {
        extract_port_from_url(&self.card.url).unwrap_or(80)
    }
}
