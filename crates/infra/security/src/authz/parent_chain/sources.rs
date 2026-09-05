//! Pure membership data behind a [`super::ParentChainIndex`]: which plugins
//! each enabled marketplace parents, which plugins select each skill, and
//! which agents and MCP servers each marketplace names directly. Derived from
//! a [`ServicesConfig`] by [`ChainSources::from_services`], or assembled by a
//! caller that already holds the resolved catalogue.
//!
//! Membership is many-to-many: a plugin listed by two enabled marketplaces
//! belongs to both, and the resolver is handed one chain per owner so any
//! admitting marketplace admits the entity.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

use systemprompt_identifiers::{MarketplaceId, PluginId, SkillId};
use systemprompt_models::services::{MarketplaceConfig, MarketplaceMemberKind, ServicesConfig};

use crate::authz::types::EntityKind;

#[derive(Debug, Clone)]
pub struct MarketplaceSource {
    pub id: MarketplaceId,
    pub fallback_default_included: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct ChainSources {
    pub marketplaces: BTreeMap<MarketplaceId, MarketplaceSource>,
    pub plugins: BTreeMap<PluginId, BTreeSet<MarketplaceId>>,
    pub skill_owners: BTreeMap<SkillId, BTreeSet<PluginId>>,
    pub marketplace_members: BTreeMap<EntityKind, BTreeMap<String, BTreeSet<MarketplaceId>>>,
}

impl ChainSources {
    #[must_use]
    pub fn from_services(services: &ServicesConfig) -> Self {
        let mut out = Self::default();
        for marketplace in services.enabled_marketplaces() {
            out.absorb(services, marketplace);
        }
        out
    }

    fn absorb(&mut self, services: &ServicesConfig, marketplace: &MarketplaceConfig) {
        let id = marketplace.id.clone();
        self.marketplaces.insert(
            id.clone(),
            MarketplaceSource {
                id: id.clone(),
                fallback_default_included: Some(marketplace.access.default_included),
            },
        );

        for plugin in services.marketplace_plugin_configs(marketplace) {
            self.plugins
                .entry(plugin.id.clone())
                .or_default()
                .insert(id.clone());
            for skill in services.plugin_selected_skill_ids(plugin) {
                self.skill_owners
                    .entry(SkillId::new(skill))
                    .or_default()
                    .insert(plugin.id.clone());
            }
        }

        for (kind, member_kind, catalogue) in [
            (
                EntityKind::Agent,
                MarketplaceMemberKind::Agents,
                services.agents.keys().cloned().collect::<Vec<String>>(),
            ),
            (
                EntityKind::McpServer,
                MarketplaceMemberKind::McpServers,
                services
                    .mcp_servers
                    .keys()
                    .cloned()
                    .collect::<Vec<String>>(),
            ),
        ] {
            // Why: an empty `include:` means "every member of that catalogue",
            // the same rule the manifest scoper applies — validation rejects an
            // explicit ref with an empty include, so empty here is never
            // "nothing".
            let include = &marketplace.members(member_kind).include;
            let members: Vec<String> = if include.is_empty() {
                catalogue
            } else {
                include.clone()
            };
            let band = self.marketplace_members.entry(kind).or_default();
            for member in members {
                band.entry(member).or_default().insert(id.clone());
            }
        }
    }

    #[must_use]
    pub fn plugin_ids_to_load(&self) -> Vec<String> {
        let mut ids: BTreeSet<&str> = self.plugins.keys().map(PluginId::as_str).collect();
        for owners in self.skill_owners.values() {
            ids.extend(owners.iter().map(PluginId::as_str));
        }
        ids.into_iter().map(str::to_owned).collect()
    }

    #[must_use]
    pub fn marketplace_ids_to_load(&self) -> Vec<String> {
        self.marketplaces
            .keys()
            .map(|id| id.as_str().to_owned())
            .collect()
    }

    #[must_use]
    pub fn marketplaces_of(&self, kind: EntityKind, id: &str) -> &BTreeSet<MarketplaceId> {
        static NONE: std::sync::OnceLock<BTreeSet<MarketplaceId>> = std::sync::OnceLock::new();
        self.marketplace_members
            .get(&kind)
            .and_then(|band| band.get(id))
            .unwrap_or_else(|| NONE.get_or_init(BTreeSet::new))
    }

    #[must_use]
    pub fn plugin_marketplaces(&self, id: &str) -> &BTreeSet<MarketplaceId> {
        static NONE: std::sync::OnceLock<BTreeSet<MarketplaceId>> = std::sync::OnceLock::new();
        self.plugins
            .get(&PluginId::new(id))
            .unwrap_or_else(|| NONE.get_or_init(BTreeSet::new))
    }

    #[must_use]
    pub fn is_marketplace_member(&self, kind: EntityKind, id: &str) -> bool {
        !self.marketplaces_of(kind, id).is_empty()
    }
}
