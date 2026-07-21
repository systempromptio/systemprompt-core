//! The bundle of catalogue items handed to a [`crate::MarketplaceFilter`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

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
    /// Artifact id to the plugins that ship it. Artifacts carry no access rule
    /// of their own, so a filter gates them through this map: an artifact
    /// survives only while at least one owning plugin does.
    pub artifact_owners: BTreeMap<String, BTreeSet<String>>,
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
            artifact_owners: BTreeMap::new(),
            marketplace_id: None,
            access: None,
        }
    }

    #[must_use]
    pub fn with_artifact_owners(mut self, owners: BTreeMap<String, BTreeSet<String>>) -> Self {
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

    /// Drops artifacts whose every owning plugin was filtered away. Enforced
    /// centrally after filtering rather than left to each `MarketplaceFilter`,
    /// so a filter that only removes plugins cannot leak their artifacts.
    pub fn prune_orphaned_artifacts(&mut self) {
        let surviving: BTreeSet<&str> = self.plugins.iter().map(|p| p.id.as_str()).collect();
        let owners = &self.artifact_owners;
        self.artifacts.retain(|a| {
            let kept = owners
                .get(a.id.as_str())
                .is_some_and(|o| o.iter().any(|p| surviving.contains(p.as_str())));
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
