pub mod agent_config;
pub mod ai;
pub mod runtime;
pub mod scheduler;
pub mod settings;
pub mod skills;
pub mod web;

pub use agent_config::*;
pub use ai::{
    AiConfig, AiProviderConfig, HistoryConfig, McpConfig, ModelCapabilities, ModelDefinition,
    ModelLimits, ModelPricing, SamplingConfig, ToolModelConfig, ToolModelSettings,
};
pub use runtime::{RuntimeStatus, ServiceType};
pub use scheduler::*;
pub use settings::*;
pub use skills::{SkillConfig, SkillsConfig};
pub use web::{BrandingConfig, WebConfig};

use crate::mcp::Deployment;
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub web: WebConfig,
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
