//! Pure membership data behind a [`super::ParentChainIndex`]: which plugins a
//! marketplace parents, which plugins select each skill, and which agents
//! and MCP servers the marketplace names directly. Derived from a
//! [`ServicesConfig`] by [`ChainSources::from_services`], or assembled by a
//! caller that already holds the resolved catalogue.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

use systemprompt_identifiers::{MarketplaceId, PluginId, SkillId};
use systemprompt_models::services::{MarketplaceMemberKind, ServicesConfig};

use crate::authz::marketplace_floor::active_marketplace;
use crate::authz::types::EntityKind;

#[derive(Debug, Clone)]
pub struct MarketplaceSource {
    pub id: MarketplaceId,
    pub fallback_default_included: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct ChainSources {
    pub marketplace: Option<MarketplaceSource>,
    pub plugins: BTreeSet<PluginId>,
    pub skill_owners: BTreeMap<SkillId, BTreeSet<PluginId>>,
    pub marketplace_members: BTreeMap<EntityKind, BTreeSet<String>>,
}

impl ChainSources {
    #[must_use]
    pub fn from_services(services: &ServicesConfig) -> Self {
        let Some(marketplace) = active_marketplace(services) else {
            return Self::default();
        };
        let plugins = services.marketplace_plugin_configs(marketplace);

        let mut skill_owners: BTreeMap<SkillId, BTreeSet<PluginId>> = BTreeMap::new();
        for plugin in &plugins {
            for skill in services.plugin_selected_skill_ids(plugin) {
                skill_owners
                    .entry(SkillId::new(skill))
                    .or_default()
                    .insert(plugin.id.clone());
            }
        }

        let marketplace_members = [
            (EntityKind::Agent, MarketplaceMemberKind::Agents),
            (EntityKind::McpServer, MarketplaceMemberKind::McpServers),
        ]
        .into_iter()
        .map(|(kind, member_kind)| {
            (
                kind,
                marketplace
                    .members(member_kind)
                    .include
                    .iter()
                    .cloned()
                    .collect(),
            )
        })
        .collect();

        Self {
            marketplace: Some(MarketplaceSource {
                id: marketplace.id.clone(),
                fallback_default_included: Some(marketplace.access.default_included),
            }),
            plugins: plugins.iter().map(|plugin| plugin.id.clone()).collect(),
            skill_owners,
            marketplace_members,
        }
    }

    #[must_use]
    pub fn plugin_ids_to_load(&self) -> Vec<String> {
        let mut ids: BTreeSet<&str> = self.plugins.iter().map(PluginId::as_str).collect();
        for owners in self.skill_owners.values() {
            ids.extend(owners.iter().map(PluginId::as_str));
        }
        ids.into_iter().map(str::to_owned).collect()
    }

    #[must_use]
    pub fn is_marketplace_member(&self, kind: EntityKind, id: &str) -> bool {
        self.marketplace_members
            .get(&kind)
            .is_some_and(|members| members.contains(id))
    }
}
