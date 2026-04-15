pub mod agent_config;
pub mod ai;
pub mod content;
pub mod hooks;
pub mod mcp;
pub mod plugin;
pub mod runtime;
pub mod scheduler;
pub mod settings;
pub mod skills;

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

use crate::mcp::{Deployment, McpServerType};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum IncludableString {
    Inline(String),
    Include { path: String },
}

impl<'de> Deserialize<'de> for IncludableString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.strip_prefix("!include ")
            .map_or_else(
                || Self::Inline(s.clone()),
                |path| Self::Include {
                    path: path.trim().to_string(),
                },
            )
            .pipe(Ok)
    }
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}
impl<T> Pipe for T {}

impl IncludableString {
    pub const fn is_include(&self) -> bool {
        matches!(self, Self::Include { .. })
    }

    pub fn as_inline(&self) -> Option<&str> {
        match self {
            Self::Inline(s) => Some(s),
            Self::Include { .. } => None,
        }
    }
}

impl Default for IncludableString {
    fn default() -> Self {
        Self::Inline(String::new())
    }
}

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
}

impl ServicesConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
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
    ) -> anyhow::Result<()> {
        for mcp_ref in &plugin.mcp_servers {
            if !self.mcp_servers.contains_key(mcp_ref) {
                anyhow::bail!(
                    "Plugin '{}': mcp_servers references unknown mcp_server '{}'",
                    plugin_name,
                    mcp_ref
                );
            }
        }

        for agent_ref in &plugin.agents.include {
            if !self.agents.contains_key(agent_ref) {
                anyhow::bail!(
                    "Plugin '{}': agents.include references unknown agent '{}'",
                    plugin_name,
                    agent_ref
                );
            }
        }

        self.validate_skills()?;

        Ok(())
    }

    fn validate_skills(&self) -> anyhow::Result<()> {
        for (key, skill) in &self.skills.skills {
            if !skill.id.as_str().is_empty() && skill.id.as_str() != key.as_str() {
                anyhow::bail!(
                    "Skill map key '{}' does not match skill id '{}'",
                    key,
                    skill.id
                );
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

    fn validate_port_conflicts(&self) -> anyhow::Result<()> {
        let mut seen_ports = HashMap::new();

        for (name, agent) in &self.agents {
            if let Some(existing) = seen_ports.insert(agent.port, ("agent", name.as_str())) {
                anyhow::bail!(
                    "Port conflict: {} used by both {} '{}' and agent '{}'",
                    agent.port,
                    existing.0,
                    existing.1,
                    name
                );
            }
        }

        for (name, mcp) in &self.mcp_servers {
            if mcp.server_type == McpServerType::External {
                continue;
            }
            if let Some(existing) = seen_ports.insert(mcp.port, ("mcp_server", name.as_str())) {
                anyhow::bail!(
                    "Port conflict: {} used by both {} '{}' and mcp_server '{}'",
                    mcp.port,
                    existing.0,
                    existing.1,
                    name
                );
            }
        }

        Ok(())
    }

    fn validate_port_ranges(&self) -> anyhow::Result<()> {
        let (min, max) = self.settings.agent_port_range;

        for (name, agent) in &self.agents {
            if agent.port < min || agent.port > max {
                anyhow::bail!(
                    "Agent '{}' port {} is outside allowed range {}-{}",
                    name,
                    agent.port,
                    min,
                    max
                );
            }
        }

        Ok(())
    }

    fn validate_mcp_port_ranges(&self) -> anyhow::Result<()> {
        let (min, max) = self.settings.mcp_port_range;

        for (name, mcp) in &self.mcp_servers {
            if mcp.server_type == McpServerType::External {
                continue;
            }
            if mcp.port < min || mcp.port > max {
                anyhow::bail!(
                    "MCP server '{}' port {} is outside allowed range {}-{}",
                    name,
                    mcp.port,
                    min,
                    max
                );
            }
        }

        Ok(())
    }

    fn validate_single_default_agent(&self) -> anyhow::Result<()> {
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
            _ => anyhow::bail!(
                "Multiple agents marked as default: {}. Only one agent can have 'default: true'",
                default_agents.join(", ")
            ),
        }
    }
}
