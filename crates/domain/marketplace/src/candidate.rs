//! The bundle of catalogue items handed to a [`crate::MarketplaceFilter`].

use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::bridge::manifest::{
    AgentEntry, ArtifactEntry, HookEntry, ManagedMcpServer, PluginEntry, SkillEntry,
};
use systemprompt_models::services::MarketplaceAccess;

/// Filters may shrink, reorder, or drop entries, but must not synthesise items
/// absent from the candidate: every entry is already content-hashed, so an
/// unknown item would fail signature verification.
#[derive(Debug, Clone, Default)]
pub struct MarketplaceCandidate {
    pub plugins: Vec<PluginEntry>,
    pub skills: Vec<SkillEntry>,
    pub agents: Vec<AgentEntry>,
    pub hooks: Vec<HookEntry>,
    pub managed_mcp_servers: Vec<ManagedMcpServer>,
    pub artifacts: Vec<ArtifactEntry>,
    pub marketplace_id: Option<MarketplaceId>,
    pub access: Option<MarketplaceAccess>,
}

impl MarketplaceCandidate {
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "one parameter per parallel manifest content section; a wrapper struct would \
                  only relocate the same fan-in"
    )]
    pub const fn new(
        plugins: Vec<PluginEntry>,
        skills: Vec<SkillEntry>,
        agents: Vec<AgentEntry>,
        hooks: Vec<HookEntry>,
        managed_mcp_servers: Vec<ManagedMcpServer>,
        artifacts: Vec<ArtifactEntry>,
    ) -> Self {
        Self {
            plugins,
            skills,
            agents,
            hooks,
            managed_mcp_servers,
            artifacts,
            marketplace_id: None,
            access: None,
        }
    }

    #[must_use]
    pub fn with_marketplace(
        mut self,
        id: MarketplaceId,
        access: Option<MarketplaceAccess>,
    ) -> Self {
        self.marketplace_id = Some(id);
        self.access = access;
        self
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.plugins.is_empty()
            && self.skills.is_empty()
            && self.agents.is_empty()
            && self.hooks.is_empty()
            && self.managed_mcp_servers.is_empty()
            && self.artifacts.is_empty()
    }
}
