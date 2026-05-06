//! `services` module — see crate-level docs for context.

pub mod agent_config;
pub mod ai;
pub mod content;
pub mod hooks;
pub mod host_agent;
mod includable;
pub mod mcp;
pub mod plugin;
pub mod runtime;
pub mod scheduler;
pub mod settings;
pub mod skills;

pub use includable::IncludableString;

pub use agent_config::{
    AGENT_CONFIG_FILENAME, AgentCardConfig, AgentConfig, AgentMetadataConfig, AgentProviderInfo,
    AgentSkillConfig, AgentSummary, CapabilitiesConfig, DEFAULT_AGENT_SYSTEM_PROMPT_FILE,
    DiskAgentConfig, OAuthConfig,
};
pub use ai::{
    AiConfig, AiProviderConfig, HistoryConfig, McpConfig, ModelCapabilities, ModelDefinition,
    ModelLimits, ModelPricing, SamplingConfig, ToolModelConfig, ToolModelSettings,
};
pub use content::ContentConfig;
pub use hooks::{
    DiskHookConfig, HOOK_CONFIG_FILENAME, HookAction, HookCategory, HookEvent, HookEventsConfig,
    HookMatcher, HookType,
};
pub use host_agent::{HostAgentConfig, HostAgentKind};
pub use mcp::McpServerSummary;
pub use plugin::{
    ComponentFilter, ComponentSource, PluginAuthor, PluginComponentRef, PluginConfig,
    PluginConfigFile, PluginScript, PluginSummary, PluginVariableDef,
};
pub use runtime::{RuntimeStatus, ServiceType};
pub use scheduler::*;
pub use settings::*;
pub use skills::{
    DEFAULT_SKILL_CONTENT_FILE, DiskSkillConfig, SKILL_CONFIG_FILENAME, SkillConfig, SkillDetail,
    SkillSummary, SkillsConfig, strip_frontmatter,
};
pub use systemprompt_provider_contracts::{BrandingConfig, WebConfig};

use crate::errors::ConfigValidationError;
use crate::mcp::{Deployment, McpServerType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartialServicesConfig {
    #[serde(default)]
    pub agents: HashMap<String, AgentConfig>,
    #[serde(default)]
    pub mcp_servers: HashMap<String, Deployment>,
    #[serde(default)]
    pub scheduler: Option<SchedulerConfig>,
    #[serde(default)]
    pub ai: Option<AiConfig>,
    #[serde(default)]
    pub web: Option<WebConfig>,
    #[serde(default)]
    pub plugins: HashMap<String, PluginConfig>,
    #[serde(default)]
    pub skills: SkillsConfig,
    #[serde(default)]
    pub content: ContentConfig,
    #[serde(default)]
    pub host_agents: HashMap<String, HostAgentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServicesConfig {
    #[serde(default)]
    pub agents: HashMap<String, AgentConfig>,
    #[serde(default)]
    pub mcp_servers: HashMap<String, Deployment>,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub scheduler: Option<SchedulerConfig>,
    #[serde(default)]
    pub ai: AiConfig,
    #[serde(default)]
    pub web: Option<WebConfig>,
    #[serde(default)]
    pub plugins: HashMap<String, PluginConfig>,
    #[serde(default)]
    pub skills: SkillsConfig,
    #[serde(default)]
    pub content: ContentConfig,
    #[serde(default)]
    pub host_agents: HashMap<String, HostAgentConfig>,
}

impl ServicesConfig {
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        self.validate_port_conflicts()?;
        self.validate_port_ranges()?;
        self.validate_mcp_port_ranges()?;
        self.validate_single_default_agent()?;

        for (name, agent) in &self.agents {
            agent.validate(name)?;
        }

        for (name, plugin) in &self.plugins {
            plugin.validate(name)?;
            self.validate_plugin_bindings(name, plugin)?;
        }

        Ok(())
    }

    fn validate_plugin_bindings(
        &self,
        plugin_name: &str,
        plugin: &PluginConfig,
    ) -> Result<(), ConfigValidationError> {
        for mcp_ref in &plugin.mcp_servers {
            if !self.mcp_servers.contains_key(mcp_ref) {
                return Err(ConfigValidationError::unknown_reference(format!(
                    "Plugin '{plugin_name}': mcp_servers references unknown mcp_server '{mcp_ref}'"
                )));
            }
        }

        for agent_ref in &plugin.agents.include {
            if !self.agents.contains_key(agent_ref) {
                return Err(ConfigValidationError::unknown_reference(format!(
                    "Plugin '{plugin_name}': agents.include references unknown agent '{agent_ref}'"
                )));
            }
        }

        self.validate_skills()?;

        Ok(())
    }

    fn validate_skills(&self) -> Result<(), ConfigValidationError> {
        for (key, skill) in &self.skills.skills {
            if !skill.id.as_str().is_empty() && skill.id.as_str() != key.as_str() {
                return Err(ConfigValidationError::invalid_field(format!(
                    "Skill map key '{}' does not match skill id '{}'",
                    key, skill.id
                )));
            }

            for agent_ref in &skill.assigned_agents {
                if !self.agents.contains_key(agent_ref) {
                    tracing::warn!(
                        skill = %key,
                        agent = %agent_ref,
                        "Skill references agent that is not defined in services config"
                    );
                }
            }

            for mcp_ref in &skill.mcp_servers {
                if !self.mcp_servers.contains_key(mcp_ref) {
                    tracing::warn!(
                        skill = %key,
                        mcp_server = %mcp_ref,
                        "Skill references MCP server that is not defined in services config"
                    );
                }
            }
        }

        Ok(())
    }

    fn validate_port_conflicts(&self) -> Result<(), ConfigValidationError> {
        let mut seen_ports = HashMap::new();

        for (name, agent) in &self.agents {
            if let Some(existing) = seen_ports.insert(agent.port, ("agent", name.as_str())) {
                return Err(ConfigValidationError::port_conflict(format!(
                    "Port conflict: {} used by both {} '{}' and agent '{}'",
                    agent.port, existing.0, existing.1, name
                )));
            }
        }

        for (name, mcp) in &self.mcp_servers {
            if mcp.server_type == McpServerType::External {
                continue;
            }
            if let Some(existing) = seen_ports.insert(mcp.port, ("mcp_server", name.as_str())) {
                return Err(ConfigValidationError::port_conflict(format!(
                    "Port conflict: {} used by both {} '{}' and mcp_server '{}'",
                    mcp.port, existing.0, existing.1, name
                )));
            }
        }

        Ok(())
    }

    fn validate_port_ranges(&self) -> Result<(), ConfigValidationError> {
        let (min, max) = self.settings.agent_port_range;

        for (name, agent) in &self.agents {
            if agent.port < min || agent.port > max {
                return Err(ConfigValidationError::invalid_field(format!(
                    "Agent '{}' port {} is outside allowed range {}-{}",
                    name, agent.port, min, max
                )));
            }
        }

        Ok(())
    }

    fn validate_mcp_port_ranges(&self) -> Result<(), ConfigValidationError> {
        let (min, max) = self.settings.mcp_port_range;

        for (name, mcp) in &self.mcp_servers {
            if mcp.server_type == McpServerType::External {
                continue;
            }
            if mcp.port < min || mcp.port > max {
                return Err(ConfigValidationError::invalid_field(format!(
                    "MCP server '{}' port {} is outside allowed range {}-{}",
                    name, mcp.port, min, max
                )));
            }
        }

        Ok(())
    }

    fn validate_single_default_agent(&self) -> Result<(), ConfigValidationError> {
        let default_agents: Vec<&str> = self
            .agents
            .iter()
            .filter_map(|(name, agent)| {
                if agent.default {
                    Some(name.as_str())
                } else {
                    None
                }
            })
            .collect();

        match default_agents.len() {
            0 | 1 => Ok(()),
            _ => Err(ConfigValidationError::business_rule(format!(
                "Multiple agents marked as default: {}. Only one agent can have 'default: true'",
                default_agents.join(", ")
            ))),
        }
    }
}
