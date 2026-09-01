//! Marketplace browsing served to the GUI webview.
//!
//! Assembles the tenant's installed plugins, skills, agents, hooks, MCP
//! servers, and artifacts into a single [`MarketplaceListing`], then merges in
//! items from external [`MarketplaceSource`](source::MarketplaceSource)
//! registrations. Built-in scanners live in the `plugins` and `components`
//! sub-modules; the data model lives in `types`, the assembly here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod components;
mod frontmatter;
pub mod hooks;
mod plugins;
pub mod source;
mod types;

pub use plugins::{mark_shared_mcp, plugin_children};
pub(crate) use types::{
    ChangeKind, FrontmatterExtra, MarketplaceExtra, McpServerEntry, PluginManifest,
};
pub use types::{MarketplaceDiff, MarketplaceItem, MarketplaceListing, PluginChild};

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;


use crate::config::paths;
use crate::proxy::mcp_probe::McpServerAuth;
use crate::sync::read_last_sync;
use source::{MarketplaceCategory, MarketplaceSourceCtx, MarketplaceSourceRegistration};

pub fn build_listing(
    loopback: &crate::proxy::LoopbackEndpoint,
    registry: &crate::mcp_registry::McpRegistry,
    mcp_auth: &[McpServerAuth],
) -> MarketplaceListing {
    let loc = paths::org_plugins_effective();
    let plugins_dir = loc.as_ref().map(|l| l.path.display().to_string());
    let plugins_root: Option<PathBuf> = loc.as_ref().map(|l| l.path.clone());

    let last_sync = paths::bridge_metadata_dir().and_then(|meta| {
        read_last_sync(&meta.join(paths::LAST_SYNC_SENTINEL))
            .ok()
            .flatten()
    });

    let (mut plugins, skills, hooks, mcp, agents) = match loc {
        Some(loc) => {
            let plugins = plugins::list_plugins(&loc.path);
            let mut skills = Vec::new();
            let mut agents = Vec::new();
            let mut hooks = Vec::new();
            for dir in plugins::plugin_dirs(&loc.path) {
                // Why: the dir name is the plugin id. Stamped here, before
                // `dedup_by_id` collapses an item shipped by two plugins —
                // first-plugin-wins would otherwise discard the second owner
                // and file the item under one plugin only.
                let owner = dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_owned();
                let own = |items: Vec<MarketplaceItem>| -> Vec<MarketplaceItem> {
                    items
                        .into_iter()
                        .map(|mut i| {
                            i.plugins.push(owner.clone());
                            i
                        })
                        .collect()
                };
                skills.extend(own(components::list_skills(&dir.join("skills"))));
                agents.extend(own(components::list_agents(&dir.join("agents"))));
                hooks.extend(own(hooks::list_hooks(&dir.join("hooks"))));
            }
            let mcp = components::list_registry_mcp(loopback, registry);
            (
                plugins,
                dedup_by_id(skills),
                dedup_by_id(hooks),
                mcp,
                dedup_by_id(agents),
            )
        },
        None => (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()),
    };

    let mut skills = skills;
    let mut hooks = hooks;
    let mut mcp = mcp;
    let mut agents = agents;
    let last_sync_diff = last_sync
        .as_ref()
        .map_or_else(MarketplaceDiff::default, |state| {
            let diff = plugins::annotate_plugins_with_diff(&mut plugins, state);
            for items in [&mut skills, &mut hooks, &mut mcp, &mut agents] {
                plugins::annotate_with_diff(items, state);
            }
            diff
        });

    let ctx = MarketplaceSourceCtx {
        plugins_root: plugins_root.as_deref(),
        mcp_auth,
    };

    MarketplaceListing {
        plugins: merge_external(plugins, MarketplaceCategory::Plugins, &ctx),
        skills: merge_external(skills, MarketplaceCategory::Skills, &ctx),
        hooks: merge_external(hooks, MarketplaceCategory::Hooks, &ctx),
        mcp: merge_external(mcp, MarketplaceCategory::Mcp, &ctx),
        agents: merge_external(agents, MarketplaceCategory::Agents, &ctx),
        artifacts: merge_external(
            components::list_artifacts(),
            MarketplaceCategory::Artifacts,
            &ctx,
        ),
        plugins_dir,
        last_sync_diff,
    }
}

// Why: keeps the first item per id, but unions the owner lists rather than
// dropping the loser's — an item two plugins ship belongs to both, and the
// grouped listing must show it under each.
fn dedup_by_id(items: Vec<MarketplaceItem>) -> Vec<MarketplaceItem> {
    let mut out: Vec<MarketplaceItem> = Vec::new();
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    for item in items {
        if let Some(&at) = index.get(&item.id) {
            let kept: &mut MarketplaceItem = &mut out[at];
            for owner in item.plugins {
                if !kept.plugins.contains(&owner) {
                    kept.plugins.push(owner);
                }
            }
            continue;
        }
        index.insert(item.id.clone(), out.len());
        out.push(item);
    }
    out
}

fn external_items(
    category: MarketplaceCategory,
    ctx: &MarketplaceSourceCtx<'_>,
) -> Vec<MarketplaceItem> {
    let mut regs: Vec<&'static MarketplaceSourceRegistration> =
        inventory::iter::<MarketplaceSourceRegistration>()
            .filter(|r| r.source.category() == category)
            .collect();
    regs.sort_by_key(|r| std::cmp::Reverse(r.priority));
    regs.into_iter().flat_map(|r| r.source.items(ctx)).collect()
}

fn merge_external(
    builtin: Vec<MarketplaceItem>,
    category: MarketplaceCategory,
    ctx: &MarketplaceSourceCtx<'_>,
) -> Vec<MarketplaceItem> {
    let mut merged = external_items(category, ctx);
    merged.extend(builtin);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    merged.retain(|item| seen.insert(item.id.clone()));
    merged.sort_by(|a, b| a.name.cmp(&b.name));
    merged
}

pub fn listing_to_value(
    listing: &MarketplaceListing,
) -> Result<serde_json::Value, serde_json::Error> {
    serde_json::to_value(listing)
}
