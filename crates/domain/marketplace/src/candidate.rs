//! The bundle of catalogue items handed to a [`crate::MarketplaceFilter`].
//!
//! Ownership maps (`skill_owners`, `artifact_owners`) drive the per-plugin
//! access cascade and orphan pruning. They are not serialised as maps, but
//! [`MarketplaceCandidate::into_manifest_parts`] stamps each entry's surviving
//! owners onto `SkillEntry::plugins` / `ArtifactEntry::plugins` so the bridge
//! can group its Marketplace listing without a second request. The same pass
//! narrows each `ManifestMarketplace` to its surviving plugins and drops a
//! marketplace left with none, so a client mirrors exactly the marketplaces
//! that still carry something for this user.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use systemprompt_identifiers::{AgentId, HookId, MarketplaceId, McpServerId};
use systemprompt_models::bridge::ids::{LibraryArtifactId, PluginId, SkillId};
use systemprompt_models::bridge::manifest::{
    AgentEntry, ArtifactEntry, HookEntry, ManagedMcpServer, ManifestMarketplace, PluginEntry,
    SkillEntry,
};

use crate::membership::MarketplaceMembership;

/// Per-kind allow-lists for [`MarketplaceCandidate::retain_entries`].
#[derive(Debug, Clone, Default)]
pub struct EntryKeepSets {
    pub plugins: HashSet<PluginId>,
    pub skills: HashSet<SkillId>,
    pub agents: HashSet<AgentId>,
    pub hooks: HashSet<HookId>,
    pub mcp_servers: HashSet<McpServerId>,
    pub marketplaces: HashSet<MarketplaceId>,
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
    pub marketplaces: Vec<ManifestMarketplace>,
    pub skill_owners: BTreeMap<SkillId, BTreeSet<PluginId>>,
    pub artifact_owners: BTreeMap<LibraryArtifactId, BTreeSet<PluginId>>,
    pub membership: MarketplaceMembership,
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
    pub marketplaces: Vec<ManifestMarketplace>,
    pub diagnostics: Vec<String>,
}

/// Assembly context consumed by filtering, never serialised to the manifest.
#[derive(Debug, Clone, Default)]
pub struct FilterContext {
    pub skill_owners: BTreeMap<SkillId, BTreeSet<PluginId>>,
    pub artifact_owners: BTreeMap<LibraryArtifactId, BTreeSet<PluginId>>,
    pub membership: MarketplaceMembership,
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
            marketplaces,
            skill_owners,
            artifact_owners,
            membership,
            diagnostics,
        } = self;
        // Why: ownership is stamped onto the entries here, at the one point
        // where the final plugin list and the owner maps are both in hand.
        // Intersecting with `surviving` keeps a plugin the access filter
        // removed from surfacing as an owner of an artifact it no longer
        // grants.
        let surviving: BTreeSet<&PluginId> = plugins.iter().map(|p| &p.id).collect();
        let owned = |owners: Option<&BTreeSet<PluginId>>| -> Vec<PluginId> {
            owners.map_or_else(Vec::new, |o| {
                o.iter()
                    .filter(|p| surviving.contains(p))
                    .cloned()
                    .collect()
            })
        };
        let mut skills = skills;
        for skill in &mut skills {
            skill.plugins = owned(skill_owners.get(&skill.id));
        }
        let mut artifacts = artifacts;
        for artifact in &mut artifacts {
            artifact.plugins = owned(artifact_owners.get(&artifact.id));
        }
        // Why: a marketplace is listed only for the plugins it still carries;
        // one whose every plugin was filtered out would tell the client to
        // create an empty host marketplace.
        let mut marketplaces = marketplaces;
        for marketplace in &mut marketplaces {
            marketplace.plugin_ids.retain(|p| surviving.contains(p));
        }
        marketplaces.retain(|m| !m.plugin_ids.is_empty());
        (
            ManifestEntries {
                plugins,
                skills,
                agents,
                hooks,
                managed_mcp_servers,
                artifacts,
                marketplaces,
                diagnostics,
            },
            FilterContext {
                skill_owners,
                artifact_owners,
                membership,
            },
        )
    }

    #[must_use]
    pub fn with_skill_owners(mut self, owners: BTreeMap<SkillId, BTreeSet<PluginId>>) -> Self {
        self.skill_owners = owners;
        self
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
    pub fn with_membership(mut self, membership: MarketplaceMembership) -> Self {
        self.membership = membership;
        self
    }

    // Why: filters shrink entry lists — the listed marketplaces included, since
    // a marketplace denied at its own level must not be mirrored even when a
    // plugin it carries survives through another owner — but never the
    // assembly context: membership, ownership, and diagnostics stay untouched.
    pub fn retain_entries(&mut self, keep: &EntryKeepSets) {
        self.plugins.retain(|p| keep.plugins.contains(&p.id));
        self.marketplaces
            .retain(|m| keep.marketplaces.contains(&m.id));
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
