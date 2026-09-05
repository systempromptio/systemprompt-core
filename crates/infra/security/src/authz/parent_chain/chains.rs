//! Owner-chain enumeration for a [`ParentChainIndex`].
//!
//! Every chain is one path from an entity up to a marketplace. An entity that
//! several marketplaces admit produces one chain each, ordered by marketplace
//! id, and [`ParentChainIndex::resolve`] takes the first that permits — so a
//! member of several marketplaces is admitted when any admits, and the first
//! deny in id order is the one reported when none does.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::MarketplaceId;

use super::ParentChainIndex;
use crate::authz::resolver::ResolveParent;
use crate::authz::types::EntityKind;

impl ParentChainIndex {
    #[must_use]
    pub fn chains_for(&self, kind: EntityKind, id: &str) -> Vec<Vec<ResolveParent<'_>>> {
        match kind {
            EntityKind::Skill => self.skill_chains(id),
            EntityKind::Plugin => self.plugin_chains(id),
            _ => self.member_chains(kind, id),
        }
    }

    fn marketplace_parents<'a>(
        &'a self,
        ids: impl IntoIterator<Item = &'a MarketplaceId>,
    ) -> Vec<ResolveParent<'a>> {
        ids.into_iter()
            .filter_map(|id| self.marketplaces.get(id))
            .map(super::LoadedParent::as_resolve_parent)
            .collect()
    }

    fn member_chains(&self, kind: EntityKind, id: &str) -> Vec<Vec<ResolveParent<'_>>> {
        self.marketplace_parents(self.sources.marketplaces_of(kind, id))
            .into_iter()
            .map(|parent| vec![parent])
            .collect()
    }

    fn plugin_chains(&self, id: &str) -> Vec<Vec<ResolveParent<'_>>> {
        self.marketplace_parents(self.sources.plugin_marketplaces(id))
            .into_iter()
            .map(|parent| vec![parent])
            .collect()
    }

    fn skill_chains(&self, id: &str) -> Vec<Vec<ResolveParent<'_>>> {
        let mut owned: Vec<Vec<ResolveParent<'_>>> = Vec::new();
        for owner in self.sources.skill_owners.get(id).into_iter().flatten() {
            let Some(plugin) = self.plugins.get(owner) else {
                continue;
            };
            let plugin_parent = plugin.as_resolve_parent();
            let marketplaces =
                self.marketplace_parents(self.sources.plugin_marketplaces(owner.as_str()));
            if marketplaces.is_empty() {
                owned.push(vec![plugin_parent]);
                continue;
            }
            for marketplace in marketplaces {
                owned.push(vec![plugin_parent, marketplace]);
            }
        }
        if !owned.is_empty() {
            return owned;
        }
        self.member_chains(EntityKind::Skill, id)
    }
}
