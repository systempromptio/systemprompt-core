use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::PluginId;

use super::hooks::HookEventsConfig;

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ComponentSource {
    #[default]
    Instance,
    Explicit,
}

impl fmt::Display for ComponentSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Instance => write!(f, "instance"),
            Self::Explicit => write!(f, "explicit"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ComponentFilter {
    Enabled,
}

impl fmt::Display for ComponentFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Enabled => write!(f, "enabled"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfigFile {
    pub plugin: PluginConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginVariableDef {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default)]
    pub secret: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub id: PluginId,
    pub name: String,
    pub description: String,
    pub version: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub author: PluginAuthor,
    pub keywords: Vec<String>,
    pub license: String,
    pub category: String,

    pub skills: PluginComponentRef,
    pub agents: PluginComponentRef,
    #[serde(default)]
    pub mcp_servers: Vec<String>,
    #[serde(default)]
    pub content_sources: Vec<String>,
    #[serde(default)]
    pub hooks: HookEventsConfig,
    #[serde(default)]
    pub scripts: Vec<PluginScript>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginComponentRef {
    #[serde(default)]
    pub source: ComponentSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<ComponentFilter>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginScript {
    pub name: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginAuthor {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginSummary {
    pub id: PluginId,
    pub name: String,
    pub display_name: String,
    pub enabled: bool,
    pub skill_count: usize,
    pub agent_count: usize,
}

impl From<&PluginConfig> for PluginSummary {
    fn from(config: &PluginConfig) -> Self {
        Self {
            id: config.id.clone(),
            name: config.name.clone(),
            display_name: config.name.clone(),
            enabled: config.enabled,
            skill_count: config.skills.include.len(),
            agent_count: config.agents.include.len(),
        }
    }
}

impl PluginConfig {
    pub fn validate(&self, key: &str) -> anyhow::Result<()> {
        let id_str = self.id.as_str();
        if id_str.len() < 3 || id_str.len() > 50 {
            anyhow::bail!("Plugin '{}': id must be between 3 and 50 characters", key);
        }

        if !id_str
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            anyhow::bail!(
                "Plugin '{}': id must be lowercase alphanumeric with hyphens only (kebab-case)",
                key
            );
        }

        if self.version.is_empty() {
            anyhow::bail!("Plugin '{}': version must not be empty", key);
        }

        Self::validate_component_ref(&self.skills, key, "skills")?;
        Self::validate_component_ref(&self.agents, key, "agents")?;
        self.hooks.validate()?;

        Ok(())
    }

    fn validate_component_ref(
        component: &PluginComponentRef,
        key: &str,
        field: &str,
    ) -> anyhow::Result<()> {
        if component.source == ComponentSource::Explicit && component.include.is_empty() {
            anyhow::bail!(
                "Plugin '{}': {}.source is 'explicit' but {}.include is empty",
                key,
                field,
                field
            );
        }

        Ok(())
    }
}
