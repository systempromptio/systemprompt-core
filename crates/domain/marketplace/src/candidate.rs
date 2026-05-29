use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::bridge::manifest::{
    AgentEntry, HookEntry, ManagedMcpServer, PluginEntry, SkillEntry,
};
use systemprompt_models::services::MarketplaceAccess;

/// Bundle of marketplace items handed to a [`crate::MarketplaceFilter`].
///
/// Filters may shrink, reorder, or drop entries, but must not synthesise
/// items absent from the candidate: the gateway has already content-hashed
/// every entry, so an unknown item would fail signature verification.
#[derive(Debug, Clone, Default)]
pub struct MarketplaceCandidate {
    pub plugins: Vec<PluginEntry>,
    pub skills: Vec<SkillEntry>,
    pub agents: Vec<AgentEntry>,
    pub hooks: Vec<HookEntry>,
    pub managed_mcp_servers: Vec<ManagedMcpServer>,
    /// Owning marketplace of these entries, when assembly was scoped to one.
    ///
    /// Carried so an RBAC filter can resolve the marketplace-level grant and
    /// cascade it to members without re-reading the config; `None` under the
    /// unscoped global fallback (no active marketplace).
    pub marketplace_id: Option<MarketplaceId>,
    /// Access block of the owning marketplace, paired with `marketplace_id`.
    ///
    /// Lets a filter decide a marketplace-wide allow/deny once and apply it to
    /// every member entry rather than evaluating each id independently.
    pub access: Option<MarketplaceAccess>,
}

impl MarketplaceCandidate {
    #[must_use]
    pub const fn new(
        plugins: Vec<PluginEntry>,
        skills: Vec<SkillEntry>,
        agents: Vec<AgentEntry>,
        hooks: Vec<HookEntry>,
        managed_mcp_servers: Vec<ManagedMcpServer>,
    ) -> Self {
        Self {
            plugins,
            skills,
            agents,
            hooks,
            managed_mcp_servers,
            marketplace_id: None,
            access: None,
        }
    }

    /// Attach the owning marketplace's id and access block to the candidate.
    ///
    /// Exists so an RBAC filter can resolve the marketplace-level grant and
    /// cascade it to member entries; the manifest assembler calls this after
    /// scoping to the active marketplace.
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
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
            && self.skills.is_empty()
            && self.agents.is_empty()
            && self.hooks.is_empty()
            && self.managed_mcp_servers.is_empty()
    }
}
