//! Marketplace browsing served to the GUI webview.
//!
//! Assembles the tenant's installed plugins, skills, agents, hooks, MCP
//! servers, and artifacts into a single [`MarketplaceListing`], then merges in
//! items from external [`MarketplaceSource`](source::MarketplaceSource)
//! registrations. Built-in scanners live in the `plugins` and `components`
//! sub-modules; this file owns the serialized data model and the assembly.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod components;
mod frontmatter;
pub mod hooks;
mod plugins;
pub mod source;

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::paths;
use crate::proxy::mcp_probe::McpServerAuth;
use crate::sync::read_last_sync;
use source::{MarketplaceCategory, MarketplaceSourceCtx, MarketplaceSourceRegistration};

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum ChangeKind {
    Installed,
    Updated,
    Removed,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum MarketplaceExtra {
    Plugin(PluginManifest),
    Frontmatter(FrontmatterExtra),
    Mcp(McpServerEntry),
    None,
}

#[derive(Debug, Serialize)]
pub struct MarketplaceItem {
    id: String,
    name: String,
    source: &'static str,
    path: String,
    summary: Option<String>,
    readme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    change: Option<ChangeKind>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<PluginChild>,
    extra: MarketplaceExtra,
}

impl MarketplaceItem {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        summary: Option<String>,
        path: impl Into<String>,
        source: &'static str,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            source,
            path: path.into(),
            summary,
            readme: None,
            change: None,
            children: Vec::new(),
            extra: MarketplaceExtra::None,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct PluginChild {
    pub kind: &'static str,
    pub id: String,
    pub name: String,
    pub shared: bool,
}

#[derive(Debug, Serialize, Default)]
pub struct MarketplaceDiff {
    installed: Vec<String>,
    updated: Vec<String>,
    removed: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_applied_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MarketplaceListing {
    plugins: Vec<MarketplaceItem>,
    skills: Vec<MarketplaceItem>,
    hooks: Vec<MarketplaceItem>,
    mcp: Vec<MarketplaceItem>,
    agents: Vec<MarketplaceItem>,
    artifacts: Vec<MarketplaceItem>,
    plugins_dir: Option<String>,
    last_sync_diff: MarketplaceDiff,
}

#[derive(Debug, Deserialize, Serialize, Default)]
struct PluginManifest {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    homepage: Option<String>,
}

#[derive(Debug, Serialize)]
struct FrontmatterExtra {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
struct McpServerEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transport: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tools: Vec<String>,
}

pub fn build_listing(mcp_auth: &[McpServerAuth]) -> MarketplaceListing {
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
                skills.extend(components::list_skills(&dir.join("skills")));
                agents.extend(components::list_agents(&dir.join("agents")));
                // Every plugin carries the same managed hooks file; one listing
                // suffices.
                if hooks.is_empty() {
                    hooks = hooks::list_hooks(&dir.join("hooks"));
                }
            }
            // A skill/agent shared across plugins appears in each plugin dir;
            // collapse to one listing per id so counts reflect distinct
            // components, not catalogue × plugins.
            let mcp = components::list_registry_mcp(mcp_auth);
            (
                plugins,
                dedup_by_id(skills),
                hooks,
                mcp,
                dedup_by_id(agents),
            )
        },
        None => (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()),
    };

    let last_sync_diff = last_sync
        .as_ref()
        .map_or_else(MarketplaceDiff::default, |state| {
            plugins::annotate_plugins_with_diff(&mut plugins, state)
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

fn dedup_by_id(items: Vec<MarketplaceItem>) -> Vec<MarketplaceItem> {
    let mut seen = BTreeSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(item.id.clone()))
        .collect()
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
