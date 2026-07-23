//! `services` module — see crate-level docs for context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod agent_config;
pub mod ai;
pub mod artifacts;
pub mod external_agent;
pub mod frontmatter;
pub mod hooks;
mod includable;
pub mod marketplace;
pub mod mcp;
pub mod plugin;
pub mod runtime;
pub mod scheduler;
pub mod settings;
pub mod skills;
pub mod slack;
pub mod system_admin;
pub mod teams;
mod validation;

pub use includable::IncludableString;

pub use agent_config::{
    AGENT_CONFIG_FILENAME, AgentCardConfig, AgentConfig, AgentMetadataConfig, AgentProviderInfo,
    AgentSummary, CapabilitiesConfig, DEFAULT_AGENT_SYSTEM_PROMPT_FILE, DiskAgentConfig,
    OAuthConfig,
};
pub use ai::{
    AiConfig, AiProviderConfig, HistoryConfig, McpConfig, ModelCapabilities, ModelDefinition,
    ModelLimits, ModelPricing, ResilienceSettings, SamplingConfig,
};
pub use artifacts::{ARTIFACT_CONFIG_FILENAME, DEFAULT_ARTIFACT_CONTENT_FILE, DiskArtifactConfig};
pub use external_agent::{ExternalAgentConfig, ExternalAgentKind};
pub use frontmatter::{Frontmatter, split_frontmatter, strip_frontmatter};
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
    PluginConfigFile, PluginHooksRef, PluginScript, PluginSummary, PluginVariableDef,
};
pub use runtime::{RuntimeStatus, ServiceType};
pub use scheduler::*;
pub use settings::*;
pub use skills::{
    DEFAULT_SKILL_CONTENT_FILE, DiskSkillConfig, SKILL_CONFIG_FILENAME, SkillConfig, SkillDetail,
    SkillSummary, SkillsConfig,
};
pub use slack::{SlackAppConfig, SlackAuthzConfig};
pub use system_admin::{SystemAdmin, SystemAdminConfig};
pub use systemprompt_provider_contracts::{BrandingConfig, WebConfig};
pub use teams::{TeamsAppConfig, TeamsAuthzConfig};

use crate::errors::ConfigValidationError;
use crate::mcp::Deployment;
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
    #[serde(default)]
    pub slack_apps: HashMap<String, SlackAppConfig>,
    #[serde(default)]
    pub teams_apps: HashMap<String, TeamsAppConfig>,
}

impl ServicesConfig {
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        self.validate_ports()?;
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

        self.validate_single_governance_hook_owner()?;

        for (id, marketplace) in &self.marketplaces {
            marketplace.validate(id.as_str())?;
            self.validate_marketplace_bindings(id.as_str(), marketplace)?;
        }

        self.validate_default_marketplace_selector()?;

        for (name, app) in &self.slack_apps {
            app.validate(name)?;
        }

        for (name, app) in &self.teams_apps {
            app.validate(name)?;
        }

        Ok(())
    }
}
