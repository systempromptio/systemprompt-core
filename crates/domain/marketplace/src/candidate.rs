//! The bundle of catalogue items handed to a [`crate::MarketplaceFilter`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use systemprompt_identifiers::{AgentId, HookId, MarketplaceId, McpServerId};
use systemprompt_models::bridge::ids::{LibraryArtifactId, PluginId, SkillId};
use systemprompt_models::bridge::manifest::{
    AgentEntry, ArtifactEntry, HookEntry, ManagedMcpServer, PluginEntry, SkillEntry,
};
use systemprompt_models::services::MarketplaceAccess;

/// Per-kind allow-lists for [`MarketplaceCandidate::retain_entries`].
#[derive(Debug, Clone, Default)]
pub struct EntryKeepSets {
    pub plugins: HashSet<PluginId>,
    pub skills: HashSet<SkillId>,
    pub agents: HashSet<AgentId>,
    pub hooks: HashSet<HookId>,
    pub mcp_servers: HashSet<McpServerId>,
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

/// The entry lists a candidate contributes to the signed wire manifest.
#[derive(Debug, Clone, Default)]
pub struct ManifestEntries {
    pub plugins: Vec<PluginEntry>,
    pub skills: Vec<SkillEntry>,
    pub agents: Vec<AgentEntry>,
    pub hooks: Vec<HookEntry>,
    pub managed_mcp_servers: Vec<ManagedMcpServer>,
    pub artifacts: Vec<ArtifactEntry>,
    pub diagnostics: Vec<String>,
}

/// Assembly context consumed by filtering, never serialised to the manifest.
#[derive(Debug, Clone, Default)]
pub struct FilterContext {
    pub artifact_owners: BTreeMap<LibraryArtifactId, BTreeSet<PluginId>>,
    pub marketplace_id: Option<MarketplaceId>,
    pub access: Option<MarketplaceAccess>,
}

impl MarketplaceCandidate {
    // Why: consumes every field so a new field cannot silently fall on the
    // floor between the wire payload and the filter context.
    #[must_use]
    pub fn into_manifest_parts(self) -> (ManifestEntries, FilterContext) {
        let Self {
            plugins,
            skills,
            agents,
            hooks,
            managed_mcp_servers,
            artifacts,
            artifact_owners,
            marketplace_id,
            access,
            diagnostics,
        } = self;
        (
            ManifestEntries {
                plugins,
                skills,
                agents,
                hooks,
                managed_mcp_servers,
                artifacts,
                diagnostics,
            },
            FilterContext {
                artifact_owners,
                marketplace_id,
                access,
            },
        )
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
        self.plugins.retain(|p| keep.plugins.contains(&p.id));
        self.skills.retain(|s| keep.skills.contains(&s.id));
        self.agents.retain(|a| keep.agents.contains(&a.id));
        self.hooks.retain(|h| keep.hooks.contains(&h.id));
        self.managed_mcp_servers
            .retain(|m| keep.mcp_servers.contains(&m.id));
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
