//! Which enabled marketplaces each catalogue entry belongs to.
//!
//! A single-marketplace instance produces exactly one owner per entry, which is
//! what the assembly assumed before. With several enabled, membership is a set:
//! the manifest is their union, and the authz cascade is handed one parent
//! chain per owning marketplace so any admitting marketplace admits the entry.
//!
//! Plugins come from each marketplace's `plugins.include`; agents and MCP
//! servers from their own include lists, where an empty list means the whole
//! catalogue. Skills and artifacts are not members in their own right — they
//! inherit through the plugins that ship them, and only an entry no plugin
//! claims falls back to every marketplace.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

use systemprompt_identifiers::{AgentId, MarketplaceId, McpServerId, PluginId};
use systemprompt_models::bridge::manifest::{AgentEntry, ManagedMcpServer};
use systemprompt_models::services::{MarketplaceAccess, ServicesConfig};

#[derive(Debug, Clone, Default)]
pub struct MarketplaceMembership {
    pub access: BTreeMap<MarketplaceId, MarketplaceAccess>,
    pub plugins: BTreeMap<PluginId, BTreeSet<MarketplaceId>>,
    pub agents: BTreeMap<AgentId, BTreeSet<MarketplaceId>>,
    pub mcp_servers: BTreeMap<McpServerId, BTreeSet<MarketplaceId>>,
}

impl MarketplaceMembership {
    #[must_use]
    pub fn from_services(
        services: &ServicesConfig,
        agents: &[AgentEntry],
        mcp_servers: &[ManagedMcpServer],
    ) -> Self {
        let mut out = Self::default();
        for marketplace in services.enabled_marketplaces() {
            let id = marketplace.id.clone();
            out.access.insert(id.clone(), marketplace.access.clone());

            for plugin in services.marketplace_plugin_configs(marketplace) {
                out.plugins
                    .entry(plugin.id.clone())
                    .or_default()
                    .insert(id.clone());
            }

            for agent in select(agents, &marketplace.agents.include, |a| a.id.as_str()) {
                out.agents
                    .entry(agent.id.clone())
                    .or_default()
                    .insert(id.clone());
            }

            for server in select(mcp_servers, &marketplace.mcp_servers.include, |m| {
                m.name.as_str()
            }) {
                out.mcp_servers
                    .entry(server.id.clone())
                    .or_default()
                    .insert(id.clone());
            }
        }
        out
    }

    #[must_use]
    pub fn all_ids(&self) -> BTreeSet<MarketplaceId> {
        self.access.keys().cloned().collect()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.access.is_empty()
    }
}

// Why: an empty `include:` means "every entry"; validation rejects an explicit
// component ref with an empty include, so empty here is never "nothing".
fn select<'a, T>(items: &'a [T], include: &[String], id_of: impl Fn(&T) -> &str) -> Vec<&'a T> {
    if include.is_empty() {
        return items.iter().collect();
    }
    items
        .iter()
        .filter(|item| include.iter().any(|inc| inc == id_of(item)))
        .collect()
}
