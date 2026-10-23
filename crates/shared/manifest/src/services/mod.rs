//! The services manifest, as loaded from the services YAML tree and validated
//! at boot.
//!
//! Agents, plugins, hooks, MCP servers, skills, marketplaces, the model
//! catalog (credentials, providers, models, deployments) and the gateway
//! policy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod agent_config;
pub mod ai;
pub mod artifacts;
pub mod bridge_policy;
pub mod bundle;
pub mod catalog;
pub mod enforcement;
pub mod external_agent;
pub mod frontmatter;
pub mod gateway;
pub mod hooks;
mod includable;
pub mod marketplace;
mod marketplace_access;
pub mod marketplace_claude_code;
pub mod marketplace_external;
mod marketplace_external_error;
pub mod marketplace_external_plugin;
mod marketplace_mirror;
pub mod mcp;
pub mod plugin;
pub mod registry;
pub mod rules;
pub mod runtime;
pub mod scheduler;
mod selection;
pub mod settings;
pub mod skill_frontmatter;
pub mod skills;
pub mod slack;
pub mod system_admin;
pub mod teams;
mod validation;

pub use includable::IncludableString;

pub use agent_config::{
    AGENT_CONFIG_FILENAME, AgentCardConfig, AgentConfig, AgentMetadataConfig, AgentProviderInfo,
    AgentSummary, CapabilitiesConfig, DEFAULT_AGENT_SYSTEM_PROMPT_FILE, DiskAgentConfig,
    OAuthConfig, RetiredCardKeyError,
};
pub use ai::{
    AiConfig, HistoryConfig, McpConfig, ModelCapabilities, ModelDefinition, ModelGovernance,
    ModelPricing, ResilienceSettings, SamplingConfig,
};
pub use artifacts::{ARTIFACT_CONFIG_FILENAME, DEFAULT_ARTIFACT_CONTENT_FILE, DiskArtifactConfig};
pub use bridge_policy::BridgePolicyConfig;
pub use bundle::{
    BUNDLE_ALLOWED_DIRS, BUNDLE_FORMAT_VERSION, BUNDLE_MANIFEST_FILE, BUNDLE_MEDIA_TYPE,
    BUNDLE_SIGNATURE_ALG, BundleOwnership, BundleSignature, BundleSourceInfo, BundleSourceState,
    FileEntry, MARKETPLACE_BUNDLE_DIRS, ServicesBundleManifest, ServicesBundleState,
    SignedBundleManifest,
};
pub use catalog::vertex::{
    DiscoveryReport, DocumentedLaunchStage, RETIREMENT_NOTICE_DAYS, VertexRateCard,
    VertexRateCardEntry,
};
pub use catalog::{
    CatalogContext, CatalogError, CatalogInput, CredentialRegistry, DeploymentRegistry,
    ModelRegistry, ModelRequest, ProviderRegistry, ResolvedCatalog,
};
pub use enforcement::{EnforcementMode, UnknownEnforcementMode};
pub use external_agent::{ExternalAgentConfig, ExternalAgentKind};
pub use frontmatter::{Frontmatter, split_frontmatter, strip_frontmatter};
pub use gateway::{
    BridgeReleasesSpec, GatewayConfig, GatewayConfigSpec, GatewayProfileError, GatewayResult,
    GatewayRoute, GatewayState, ModelGlobs, OverrideRuleAction, QuotaFaultMode, ResponseFormatKind,
    RouteAlias, RouteDeployment, RouteMatch, RouteRequirements, SelectionStrategy,
    SystemPromptRule, WeightedDeployment, estimate_input_tokens, slugify_pattern,
    synthesize_route_id,
};
pub use hooks::{
    DiskHookConfig, HOOK_CONFIG_FILENAME, HookAction, HookEventsConfig, HookMatcher, HookType,
};
pub use marketplace::{
    ClaudeCodeMarketplaceConfig, ExternalMarketplace, ExternalMarketplaceSource, MarketplaceAccess,
    MarketplaceAccessRule, MarketplaceConfig, MarketplaceConfigFile, MarketplaceMemberKind,
    MarketplaceRuleAccess, MarketplaceVisibility,
};
pub use marketplace_external_plugin::{ExternalPluginEntry, ExternalPluginSource};
pub use mcp::{McpDeploymentProvider, McpServerSummary};
pub use plugin::{
    PluginAuthor, PluginConfig, PluginConfigFile, PluginScript, PluginSummary, PluginVariableDef,
};
pub use registry::{ServiceModule, ServiceStatus, UnknownServiceModule, UnknownServiceStatus};
pub use rules::{DEFAULT_RULE_CONTENT_FILE, DiskRuleConfig, RULE_CONFIG_FILENAME};
pub use runtime::ServiceType;
pub use scheduler::*;
pub use settings::*;
pub use skills::{
    DEFAULT_SKILL_CONTENT_FILE, DiskSkillConfig, SKILL_CONFIG_FILENAME, SkillConfig, SkillDetail,
    SkillSummary, SkillsConfig,
};
pub use slack::{SlackAppConfig, SlackAuthzConfig};
pub use system_admin::{SystemAdmin, SystemAdminConfig};
pub use systemprompt_provider_contracts::{BrandingConfig, WebConfig};
pub use teams::{TeamsAppConfig, TeamsAuthzConfig, TeamsEndpoints};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use systemprompt_identifiers::{ExternalAgentId, MarketplaceId};
use systemprompt_models::errors::ServicesValidationError;
use systemprompt_models::mcp::{Deployment, McpServerType};

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
    pub credentials: CredentialRegistry,
    #[serde(default)]
    pub providers: ProviderRegistry,
    #[serde(default)]
    pub models: ModelRegistry,
    #[serde(default)]
    pub deployments: DeploymentRegistry,
    #[serde(default)]
    pub gateway: Option<GatewayState>,
}

impl ServicesConfig {
    pub fn apply_port_offset(&mut self, offset: u16) -> Result<(), ServicesValidationError> {
        if offset == 0 {
            return Ok(());
        }

        let shift = |port: u16, what: &str| {
            port.checked_add(offset).ok_or_else(|| {
                ServicesValidationError::invalid_field(format!(
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
            if let Some(port) = mcp.port {
                mcp.port = Some(shift(port, &format!("MCP server '{name}'"))?);
            }
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

    pub fn validate(&self) -> Result<(), ServicesValidationError> {
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
        self.validate_single_judge_hook_owner()?;

        for (id, marketplace) in &self.marketplaces {
            marketplace.validate(id.as_str())?;
            self.validate_marketplace_bindings(id.as_str(), marketplace)?;
            self.validate_marketplace_dependencies(id.as_str(), marketplace)?;
        }

        self.validate_marketplace_selector()?;

        for (name, app) in &self.slack_apps {
            app.validate(name)?;
        }

        for (name, app) in &self.teams_apps {
            app.validate(name)?;
        }

        self.resolve_catalog(&CatalogContext::default())
            .map(drop)
            .map_err(|e| ServicesValidationError::invalid_field_cause("catalog", e))
    }

    pub fn resolve_catalog(
        &self,
        context: &CatalogContext,
    ) -> Result<ResolvedCatalog, CatalogError> {
        let gateway = self.gateway.as_ref().map(GatewayState::config);
        ResolvedCatalog::build(
            CatalogInput {
                credentials: &self.credentials,
                providers: &self.providers,
                models: &self.models,
                deployments: &self.deployments,
                gateway: gateway.as_deref(),
                default_image_model: self.ai.default_image_model.as_ref(),
            },
            context,
        )
    }

    #[must_use]
    pub fn gateway_config(&self) -> Option<&GatewayConfig> {
        self.gateway.as_ref().and_then(GatewayState::resolved)
    }
}
