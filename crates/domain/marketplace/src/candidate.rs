//! The bundle of catalogue items handed to a [`crate::MarketplaceFilter`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::bridge::ids::{LibraryArtifactId, PluginId};
use systemprompt_models::bridge::manifest::{
    AgentEntry, ArtifactEntry, HookEntry, ManagedMcpServer, PluginEntry, SkillEntry,
};
use systemprompt_models::services::MarketplaceAccess;

/// Per-kind allow-lists for [`MarketplaceCandidate::retain_entries`]. MCP
/// servers are keyed by `name`; every other kind by `id`.
#[derive(Debug, Clone, Default)]
pub struct EntryKeepSets {
    pub plugins: HashSet<String>,
    pub skills: HashSet<String>,
    pub agents: HashSet<String>,
    pub hooks: HashSet<String>,
    pub mcp_servers: HashSet<String>,
}

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
    pub artifact_owners: BTreeMap<LibraryArtifactId, BTreeSet<PluginId>>,
    pub marketplace_id: Option<MarketplaceId>,
    pub access: Option<MarketplaceAccess>,
    pub diagnostics: Vec<String>,
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
            artifact_owners: BTreeMap::new(),
            marketplace_id: None,
            access: None,
            diagnostics: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_artifact_owners(
        mut self,
        owners: BTreeMap<LibraryArtifactId, BTreeSet<PluginId>>,
    ) -> Self {
        self.artifact_owners = owners;
        self
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

    // Why: filters shrink entry lists, not the manifest's assembly context —
    // marketplace scope, access, ownership, and diagnostics stay untouched.
    pub fn retain_entries(&mut self, keep: &EntryKeepSets) {
        self.plugins
            .retain(|p| keep.plugins.contains(p.id.as_str()));
        self.skills.retain(|s| keep.skills.contains(s.id.as_str()));
        self.agents.retain(|a| keep.agents.contains(a.id.as_str()));
        self.hooks.retain(|h| keep.hooks.contains(h.id.as_str()));
        self.managed_mcp_servers
            .retain(|m| keep.mcp_servers.contains(m.name.as_str()));
        self.prune_orphaned_artifacts();
    }

    pub fn prune_orphaned_artifacts(&mut self) {
        let surviving: BTreeSet<&PluginId> = self.plugins.iter().map(|p| &p.id).collect();
        let owners = &self.artifact_owners;
        self.artifacts.retain(|a| {
            let kept = owners
                .get(&a.id)
                .is_some_and(|o| o.iter().any(|p| surviving.contains(p)));
            if !kept {
                tracing::warn!(
                    artifact_id = %a.id.as_str(),
                    "marketplace: every plugin shipping this artifact was filtered out; dropping"
                );
            }
            kept
        });
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
