use systemprompt_models::bridge::manifest::{
    AgentEntry, ManagedMcpServer, PluginEntry, SkillEntry,
};

/// Mutable bundle of marketplace items presented to a
/// [`crate::MarketplaceFilter`].
///
/// The filter receives ownership and returns the (possibly reduced) set
/// the gateway should sign and emit. Filters may shrink the vectors,
/// reorder them, or remove individual files from a `PluginEntry` — they
/// must not synthesise items that did not exist in the candidate, since
/// the gateway has already content-hashed them.
#[derive(Debug, Clone, Default)]
pub struct MarketplaceCandidate {
    pub plugins: Vec<PluginEntry>,
    pub skills: Vec<SkillEntry>,
    pub agents: Vec<AgentEntry>,
    pub managed_mcp_servers: Vec<ManagedMcpServer>,
}

impl MarketplaceCandidate {
    #[must_use]
    pub const fn new(
        plugins: Vec<PluginEntry>,
        skills: Vec<SkillEntry>,
        agents: Vec<AgentEntry>,
        managed_mcp_servers: Vec<ManagedMcpServer>,
    ) -> Self {
        Self {
            plugins,
            skills,
            agents,
            managed_mcp_servers,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
            && self.skills.is_empty()
            && self.agents.is_empty()
            && self.managed_mcp_servers.is_empty()
    }
}
