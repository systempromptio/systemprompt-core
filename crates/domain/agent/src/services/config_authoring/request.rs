//! Requests the authoring service applies: [`AgentCreateRequest`] for a new
//! agent and [`AgentEditRequest`] for changes to an existing one, each
//! composed of the field groups the agent config is written in.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, Clone, Default)]
pub struct AgentPlacement {
    pub endpoint: Option<String>,
    pub dev_only: bool,
    pub is_primary: bool,
    pub default: bool,
}

#[derive(Debug, Clone, Default)]
pub struct AgentCardLinks {
    pub version: Option<String>,
    pub icon_url: Option<String>,
    pub documentation_url: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AgentCardEdit {
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub links: AgentCardLinks,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AgentCapabilityFlags {
    pub streaming: Option<bool>,
    pub push_notifications: Option<bool>,
    pub extended_agent_card: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct AgentModelChoice {
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SystemPromptSource {
    pub inline: Option<String>,
    pub file: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MembershipEdit {
    pub add: Vec<String>,
    pub remove: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AgentEditRequest {
    pub enable: bool,
    pub disable: bool,
    pub port: Option<u16>,
    pub placement: AgentPlacement,
    pub card: AgentCardEdit,
    pub capabilities: AgentCapabilityFlags,
    pub model: AgentModelChoice,
    pub system_prompt: SystemPromptSource,
    pub mcp_servers: MembershipEdit,
    pub skills: MembershipEdit,
    pub set_values: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct AgentCreateRequest {
    pub name: String,
    pub port: u16,
    pub display_name: String,
    pub description: String,
    pub system_prompt: String,
    pub enabled: bool,
    pub placement: AgentPlacement,
    pub card_links: AgentCardLinks,
    pub capabilities: AgentCapabilityFlags,
    pub model: AgentModelChoice,
    pub mcp_servers: Vec<String>,
    pub skills: Vec<String>,
}
