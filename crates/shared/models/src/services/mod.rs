//! `services` module — see crate-level docs for context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod agent_config;
pub mod ai;
pub mod artifacts;
pub mod bridge_policy;
pub mod external_agent;
pub mod frontmatter;
pub mod gateway;
pub mod hooks;
mod includable;
pub mod marketplace;
pub mod mcp;
pub mod plugin;
pub mod providers;
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
    ModelGovernance, ModelLimits, ModelPricing, ResilienceSettings, SamplingConfig,
};
pub use artifacts::{ARTIFACT_CONFIG_FILENAME, DEFAULT_ARTIFACT_CONTENT_FILE, DiskArtifactConfig};
pub use bridge_policy::BridgePolicyConfig;
pub use external_agent::{ExternalAgentConfig, ExternalAgentKind};
pub use frontmatter::{Frontmatter, split_frontmatter, strip_frontmatter};
pub use gateway::{
    BridgeReleasesSpec, GatewayConfig, GatewayConfigSpec, GatewayProfileError, GatewayResult,
    GatewayRoute, GatewayState, OverrideRuleAction, ResponseFormatKind, RouteMatch,
    RouteRequirements, SystemPromptRule, slugify_pattern, synthesize_route_id,
};
pub use hooks::{
    DiskHookConfig, HOOK_CONFIG_FILENAME, HookAction, HookCategory, HookEvent, HookEventsConfig,
    HookMatcher, HookType,
};
pub use marketplace::{
    MarketplaceAccess, MarketplaceAccessRule, MarketplaceConfig, MarketplaceConfigFile,
    MarketplaceMemberKind, MarketplaceRuleAccess, MarketplaceVisibility,
};
pub use mcp::McpServerSummary;
pub use plugin::{
    ComponentFilter, ComponentSource, PluginAuthor, PluginComponentRef, PluginConfig,
    PluginConfigFile, PluginHooksRef, PluginScript, PluginSummary, PluginVariableDef,
};
pub use providers::{
    ApiSurface, ProviderEntry, ProviderModel, ProviderRegistry, ProviderRegistryError,
    ProviderRegistryResult, WireProtocol,
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
use crate::mcp::{Deployment, McpServerType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use systemprompt_identifiers::{ExternalAgentId, MarketplaceId};

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
    #[serde(default)]
    pub bridge_policy: Option<BridgePolicyConfig>,
    #[serde(default)]
    pub providers: ProviderRegistry,
    #[serde(default)]
    pub gateway: Option<GatewayState>,
}

impl ServicesConfig {
    pub fn apply_port_offset(&mut self, offset: u16) -> Result<(), ConfigValidationError> {
        if offset == 0 {
            return Ok(());
        }

        let shift = |port: u16, what: &str| {
            port.checked_add(offset).ok_or_else(|| {
                ConfigValidationError::invalid_field(format!(
                    "{what} port {port} shifted by services.port_offset {offset} exceeds 65535"
                ))
            })
        };

        for (name, agent) in &mut self.agents {
            agent.port = shift(agent.port, &format!("Agent '{name}'"))?;
        }

        for (name, mcp) in &mut self.mcp_servers {
            if mcp.server_type == McpServerType::External {
                continue;
            }
            mcp.port = shift(mcp.port, &format!("MCP server '{name}'"))?;
        }

        self.settings.agent_port_range = (
            shift(self.settings.agent_port_range.0, "agent_port_range lower")?,
            shift(self.settings.agent_port_range.1, "agent_port_range upper")?,
        );
        self.settings.mcp_port_range = (
            shift(self.settings.mcp_port_range.0, "mcp_port_range lower")?,
            shift(self.settings.mcp_port_range.1, "mcp_port_range upper")?,
        );

        Ok(())
    }

    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        self.validate_ports()?;
        self.validate_single_default_agent()?;

        for (name, agent) in &self.agents {
            agent.validate(name)?;
        }

        for (name, mcp) in &self.mcp_servers {
            mcp.validate(name)?;
        }

        self.validate_skills()?;

        for (name, plugin) in &self.plugins {
            plugin.validate(name)?;
            self.validate_plugin_bindings(name, plugin)?;
        }

        self.validate_single_governance_hook_owner()?;

        for (id, marketplace) in &self.marketplaces {
            marketplace.validate(id.as_str())?;
            self.validate_marketplace_bindings(id.as_str(), marketplace)?;
        }

        self.validate_marketplace_selector()?;

        for (name, app) in &self.slack_apps {
            app.validate(name)?;
        }

        for (name, app) in &self.teams_apps {
            app.validate(name)?;
        }

        self.validate_providers_and_gateway()
    }

    // Why: the registry is the authority for connectivity and the gateway only
    // references into it, so both are checked here in that order — a route
    // naming an undeclared provider is a services-tree error, not a boot-time
    // surprise. A gateway still in `Spec` form is validated as it would resolve;
    // the loader stores only the resolved form.
    fn validate_providers_and_gateway(&self) -> Result<(), ConfigValidationError> {
        self.providers
            .validate()
            .map_err(|e| ConfigValidationError::invalid_field(format!("providers: {e}")))?;
        match &self.gateway {
            Some(GatewayState::Resolved(config)) => config.validate(&self.providers),
            Some(GatewayState::Spec(spec)) => spec.clone().resolve().validate(&self.providers),
            None => Ok(()),
        }
        .map_err(|e| ConfigValidationError::invalid_field(format!("gateway: {e}")))
    }

    #[must_use]
    pub fn gateway_config(&self) -> Option<&GatewayConfig> {
        self.gateway.as_ref().and_then(GatewayState::resolved)
    }

    // Why: the manifest is the union of every enabled marketplace — an entity
    // reachable through any one of them is offered, and the parent chain
    // decides who may see it. Ordered by id so every derived list is stable.
    #[must_use]
    pub fn enabled_marketplaces(&self) -> Vec<&MarketplaceConfig> {
        let mut out: Vec<&MarketplaceConfig> =
            self.marketplaces.values().filter(|m| m.enabled).collect();
        out.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        out
    }

    #[must_use]
    pub fn marketplace_plugin_configs(
        &self,
        marketplace: &MarketplaceConfig,
    ) -> Vec<&PluginConfig> {
        self.plugins
            .values()
            .filter(|p| p.enabled)
            .filter(|p| {
                marketplace.plugins.include.is_empty()
                    || marketplace
                        .plugins
                        .include
                        .iter()
                        .any(|inc| inc == p.id.as_str())
            })
            .collect()
    }

    #[must_use]
    pub fn plugin_selected_skill_ids(
        &self,
        plugin: &PluginConfig,
    ) -> std::collections::BTreeSet<String> {
        let mut ids: std::collections::BTreeSet<String> = match plugin.skills.source {
            ComponentSource::Explicit => plugin.skills.include.iter().cloned().collect(),
            ComponentSource::Instance => self
                .skills
                .skills
                .keys()
                .filter(|k| !plugin.skills.exclude.iter().any(|ex| ex == *k))
                .cloned()
                .collect(),
        };

        let selected_agent = |name: &str| match plugin.agents.source {
            ComponentSource::Explicit => plugin.agents.include.iter().any(|inc| inc == name),
            ComponentSource::Instance => !plugin.agents.exclude.iter().any(|ex| ex == name),
        };
        for (name, agent) in &self.agents {
            if selected_agent(name) {
                ids.extend(agent.metadata.skills.include.iter().cloned());
            }
        }

        ids
    }

    #[must_use]
    pub fn marketplace_skill_members(
        &self,
        marketplace: &MarketplaceConfig,
    ) -> std::collections::BTreeSet<String> {
        self.marketplace_plugin_configs(marketplace)
            .into_iter()
            .flat_map(|plugin| self.plugin_selected_skill_ids(plugin))
            .collect()
    }
}
