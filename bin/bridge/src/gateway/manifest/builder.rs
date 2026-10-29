//! Builder for a `SignedManifest` the bridge assembles itself (tests,
//! fixtures, the dev preview); the gateway's own manifests arrive signed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::bridge::instance::{BridgeVersionWindow, InstanceBranding};

use super::{
    AgentEntry, ApiKeyId, ArtifactEntry, AutoUpdatePolicy, HookEntry,
    MANIFEST_INLINE_SCHEMA_VERSION, ManagedMcpServer, ManifestMarketplace, ManifestVersion,
    PluginEntry, RuleEntry, SignedManifest, SkillEntry, TenantId, UserId, UserInfo,
};

#[derive(Debug)]
pub struct SignedManifestBuilder {
    manifest_version: ManifestVersion,
    issued_at: chrono::DateTime<chrono::Utc>,
    not_before: chrono::DateTime<chrono::Utc>,
    user_id: UserId,
    tenant_id: Option<TenantId>,
    user: Option<UserInfo>,
    plugins: Vec<PluginEntry>,
    skills: Vec<SkillEntry>,
    rules: Vec<RuleEntry>,
    agents: Vec<AgentEntry>,
    hooks: Vec<HookEntry>,
    managed_mcp_servers: Vec<ManagedMcpServer>,
    revocations: Vec<ApiKeyId>,
    enabled_hosts: Vec<HostKind>,
    host_model_protocols: std::collections::BTreeMap<HostKind, Vec<String>>,
    artifacts: Vec<ArtifactEntry>,
    allow_claude_ai_connectors: bool,
    desktop_policy: systemprompt_models::bridge::desktop_policy::DesktopPolicy,
    auto_update: AutoUpdatePolicy,
    marketplaces: Vec<ManifestMarketplace>,
    allow_user_disable: bool,
    branding: Option<InstanceBranding>,
    bridge_versions: Option<BridgeVersionWindow>,
}

impl SignedManifestBuilder {
    #[must_use]
    pub fn new(
        manifest_version: ManifestVersion,
        issued_at: chrono::DateTime<chrono::Utc>,
        not_before: chrono::DateTime<chrono::Utc>,
        user_id: UserId,
    ) -> Self {
        Self {
            manifest_version,
            issued_at,
            not_before,
            user_id,
            tenant_id: None,
            user: None,
            plugins: Vec::new(),
            skills: Vec::new(),
            rules: Vec::new(),
            agents: Vec::new(),
            hooks: Vec::new(),
            managed_mcp_servers: Vec::new(),
            revocations: Vec::new(),
            enabled_hosts: Vec::new(),
            host_model_protocols: std::collections::BTreeMap::new(),
            artifacts: Vec::new(),
            allow_claude_ai_connectors: false,
            desktop_policy: systemprompt_models::bridge::desktop_policy::DesktopPolicy::default(),
            auto_update: AutoUpdatePolicy::default(),
            marketplaces: Vec::new(),
            allow_user_disable: false,
            branding: None,
            bridge_versions: None,
        }
    }

    #[must_use]
    pub const fn with_allow_claude_ai_connectors(mut self, allow: bool) -> Self {
        self.allow_claude_ai_connectors = allow;
        self
    }

    #[must_use]
    pub const fn with_auto_update(mut self, policy: AutoUpdatePolicy) -> Self {
        self.auto_update = policy;
        self
    }

    #[must_use]
    pub fn with_enabled_hosts(mut self, hosts: Vec<HostKind>) -> Self {
        self.enabled_hosts = hosts;
        self
    }

    #[must_use]
    pub fn with_host_model_protocols(
        mut self,
        protocols: std::collections::BTreeMap<HostKind, Vec<String>>,
    ) -> Self {
        self.host_model_protocols = protocols;
        self
    }

    #[must_use]
    pub fn with_tenant_id(mut self, tenant_id: TenantId) -> Self {
        self.tenant_id = Some(tenant_id);
        self
    }

    #[must_use]
    pub fn with_user(mut self, user: UserInfo) -> Self {
        self.user = Some(user);
        self
    }

    #[must_use]
    pub fn with_plugins(mut self, plugins: Vec<PluginEntry>) -> Self {
        self.plugins = plugins;
        self
    }

    #[must_use]
    pub fn with_skills(mut self, skills: Vec<SkillEntry>) -> Self {
        self.skills = skills;
        self
    }

    #[must_use]
    pub fn with_rules(mut self, rules: Vec<RuleEntry>) -> Self {
        self.rules = rules;
        self
    }

    #[must_use]
    pub fn with_agents(mut self, agents: Vec<AgentEntry>) -> Self {
        self.agents = agents;
        self
    }

    #[must_use]
    pub fn with_hooks(mut self, hooks: Vec<HookEntry>) -> Self {
        self.hooks = hooks;
        self
    }

    #[must_use]
    pub fn with_managed_mcp_servers(mut self, servers: Vec<ManagedMcpServer>) -> Self {
        self.managed_mcp_servers = servers;
        self
    }

    #[must_use]
    pub fn with_revocations(mut self, revocations: Vec<ApiKeyId>) -> Self {
        self.revocations = revocations;
        self
    }

    #[must_use]
    pub fn with_artifacts(mut self, artifacts: Vec<ArtifactEntry>) -> Self {
        self.artifacts = artifacts;
        self
    }

    #[must_use]
    pub fn with_marketplaces(mut self, marketplaces: Vec<ManifestMarketplace>) -> Self {
        self.marketplaces = marketplaces;
        self
    }

    #[must_use]
    pub const fn with_allow_user_disable(mut self, allow: bool) -> Self {
        self.allow_user_disable = allow;
        self
    }

    #[must_use]
    pub fn with_branding(mut self, branding: InstanceBranding) -> Self {
        self.branding = Some(branding);
        self
    }

    #[must_use]
    pub fn with_bridge_versions(mut self, window: BridgeVersionWindow) -> Self {
        self.bridge_versions = Some(window);
        self
    }

    #[must_use]
    pub fn build(self) -> SignedManifest {
        SignedManifest {
            min_schema_version: MANIFEST_INLINE_SCHEMA_VERSION,
            min_bridge_version: None,
            manifest_version: self.manifest_version,
            issued_at: self.issued_at,
            not_before: self.not_before,
            user_id: self.user_id,
            tenant_id: self.tenant_id,
            user: self.user,
            plugins: self.plugins,
            skills: self.skills,
            rules: self.rules,
            agents: self.agents,
            hooks: self.hooks,
            managed_mcp_servers: self.managed_mcp_servers,
            revocations: self.revocations,
            enabled_hosts: self.enabled_hosts,
            host_model_protocols: self.host_model_protocols,
            artifacts: self.artifacts,
            allow_claude_ai_connectors: self.allow_claude_ai_connectors,
            desktop_policy: self.desktop_policy,
            auto_update: self.auto_update,
            allow_user_disable: self.allow_user_disable,
            branding: self.branding,
            bridge_versions: self.bridge_versions,
            diagnostics: Vec::new(),
            marketplaces: self.marketplaces,
        }
    }
}
