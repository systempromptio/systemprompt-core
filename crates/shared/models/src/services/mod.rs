//! `services` module — see crate-level docs for context.

pub mod agent_config;
pub mod ai;
pub mod external_agent;
pub mod hooks;
mod includable;
pub mod marketplace;
pub mod mcp;
pub mod plugin;
pub mod runtime;
pub mod scheduler;
pub mod settings;
pub mod skills;
pub mod system_admin;

pub use includable::IncludableString;

pub use agent_config::{
    AGENT_CONFIG_FILENAME, AgentCardConfig, AgentConfig, AgentMetadataConfig, AgentProviderInfo,
    AgentSkillConfig, AgentSummary, CapabilitiesConfig, DEFAULT_AGENT_SYSTEM_PROMPT_FILE,
    DiskAgentConfig, OAuthConfig,
};
pub use ai::{
    AiConfig, AiProviderConfig, HistoryConfig, McpConfig, ModelCapabilities, ModelDefinition,
    ModelLimits, ModelPricing, ResilienceSettings, SamplingConfig, ToolModelConfig,
    ToolModelSettings,
};
pub use external_agent::{ExternalAgentConfig, ExternalAgentKind};
pub use hooks::{
    DiskHookConfig, HOOK_CONFIG_FILENAME, HookAction, HookCategory, HookEvent, HookEventsConfig,
    HookMatcher, HookType,
};
pub use marketplace::{
    MarketplaceAccess, MarketplaceConfig, MarketplaceConfigFile, MarketplaceVisibility,
};
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
pub use system_admin::{SystemAdmin, SystemAdminConfig};
pub use systemprompt_provider_contracts::{BrandingConfig, WebConfig};

use crate::errors::ConfigValidationError;
use crate::mcp::{Deployment, McpServerType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use systemprompt_identifiers::{ExternalAgentId, MarketplaceId};

/// The single canonical shape of a services config file.
///
/// A root config file and an include file deserialize into the same struct.
/// `settings` is meaningful only at the root; the loader rejects an include
/// that sets it (`ConfigLoadError::IncludeMustNotSetGlobalSettings`) rather
/// than silently ignoring the value.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServicesConfig {
    #[serde(default)]
    pub includes: Vec<String>,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub agents: HashMap<String, AgentConfig>,
    #[serde(default)]
    pub mcp_servers: HashMap<String, Deployment>,
    #[serde(default)]
    pub scheduler: Option<SchedulerConfig>,
    #[serde(default)]
    pub ai: AiConfig,
    #[serde(default)]
    pub web: Option<WebConfig>,
    #[serde(default)]
    pub plugins: HashMap<String, PluginConfig>,
    #[serde(default)]
    pub marketplaces: HashMap<MarketplaceId, MarketplaceConfig>,
    #[serde(default)]
    pub skills: SkillsConfig,
    #[serde(default)]
    pub external_agents: HashMap<ExternalAgentId, ExternalAgentConfig>,
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

        for (name, mcp) in &self.mcp_servers {
            mcp.validate(name)?;
        }

        for (name, plugin) in &self.plugins {
            plugin.validate(name)?;
            self.validate_plugin_bindings(name, plugin)?;
        }

        for (id, marketplace) in &self.marketplaces {
            marketplace.validate(id.as_str())?;
            self.validate_marketplace_bindings(id.as_str(), marketplace)?;
        }

        Ok(())
    }

    fn validate_marketplace_bindings(
        &self,
        name: &str,
        marketplace: &MarketplaceConfig,
    ) -> Result<(), ConfigValidationError> {
        for plugin_ref in &marketplace.plugins.include {
            if !self.plugins.contains_key(plugin_ref) {
                return Err(ConfigValidationError::unknown_reference(format!(
                    "Marketplace '{name}': plugins.include references unknown plugin \
                     '{plugin_ref}'"
                )));
            }
        }

        for skill_ref in &marketplace.skills.include {
            let exists = self.skills.skills.keys().any(|k| k.as_str() == skill_ref);
            if !exists {
                return Err(ConfigValidationError::unknown_reference(format!(
                    "Marketplace '{name}': skills.include references unknown skill '{skill_ref}'"
                )));
            }
        }

        for mcp_ref in &marketplace.mcp_servers.include {
            if !self.mcp_servers.contains_key(mcp_ref) {
                return Err(ConfigValidationError::unknown_reference(format!(
                    "Marketplace '{name}': mcp_servers.include references unknown mcp_server \
                     '{mcp_ref}'"
                )));
            }
        }

        for agent_ref in &marketplace.agents.include {
            if !self.agents.contains_key(agent_ref) {
                return Err(ConfigValidationError::unknown_reference(format!(
                    "Marketplace '{name}': agents.include references unknown agent '{agent_ref}'"
                )));
            }
        }

        Ok(())
    }

    fn validate_plugin_bindings(
        &self,
        plugin_name: &str,
        plugin: &PluginConfig,
    ) -> Result<(), ConfigValidationError> {
        for mcp_ref in &plugin.mcp_servers.include {
            if !self.mcp_servers.contains_key(mcp_ref) {
                return Err(ConfigValidationError::unknown_reference(format!(
                    "Plugin '{plugin_name}': mcp_servers.include references unknown mcp_server \
                     '{mcp_ref}'"
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

            for agent_ref in &skill.assigned_agents.include {
                if !self.agents.contains_key(agent_ref) {
                    tracing::warn!(
                        skill = %key,
                        agent = %agent_ref,
                        "Skill references agent that is not defined in services config"
                    );
                }
            }

            for mcp_ref in &skill.mcp_servers.include {
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
