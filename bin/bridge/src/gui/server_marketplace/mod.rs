//! Marketplace browsing endpoints served to the GUI webview.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod hooks;
pub mod source;

use crate::config::paths;
use crate::proxy::mcp_probe::McpServerAuth;
use crate::sync::{LastSyncState, read_last_sync};
use serde::{Deserialize, Serialize};
use source::{MarketplaceCategory, MarketplaceSourceCtx, MarketplaceSourceRegistration};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const README_MAX_BYTES: usize = 32 * 1024;

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
    pub(crate) id: String,
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
    /// Build a plain item for an external [`source::MarketplaceSource`]. The
    /// `source` label appears in the GUI; `extra` is `None` and `children`
    /// empty (external contributions are leaf entries).
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

fn dedup_by_id(items: Vec<MarketplaceItem>) -> Vec<MarketplaceItem> {
    let mut seen = std::collections::HashSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(item.id.clone()))
        .collect()
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
            let plugins = list_plugins(&loc.path);
            let mut skills = Vec::new();
            let mut agents = Vec::new();
            let mut hooks = Vec::new();
            for dir in plugin_dirs(&loc.path) {
                skills.extend(list_skills(&dir.join("skills")));
                agents.extend(list_agents(&dir.join("agents")));
                // Every plugin carries the same managed hooks file; one listing
                // suffices.
                if hooks.is_empty() {
                    hooks = hooks::list_hooks(&dir.join("hooks"));
                }
            }
            // A skill/agent shared across plugins appears in each plugin dir;
            // collapse to one listing per id so category counts reflect distinct
            // components, not catalogue × plugins. (Hooks are already deduped by
            // the guard above; MCP by `mark_shared_mcp`.)
            let mcp = list_registry_mcp(mcp_auth);
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
            annotate_plugins_with_diff(&mut plugins, state)
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
        artifacts: merge_external(list_artifacts(), MarketplaceCategory::Artifacts, &ctx),
        plugins_dir,
        last_sync_diff,
    }
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

fn plugin_dirs(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| e.file_name().to_str().is_some_and(|n| !n.starts_with('.')))
        .map(|e| e.path())
        .collect();
    dirs.sort();
    dirs
}

fn list_artifacts() -> Vec<MarketplaceItem> {
    use crate::integration::cowork_artifacts::{emit, sink};

    let Some(dir) = emit::resolve_artifacts_dir() else {
        return Vec::new();
    };
    let store_path = dir.join(sink::LIBRARY_STORE_FILE).display().to_string();
    let mut out: Vec<MarketplaceItem> = sink::read_library_store(&dir)
        .into_iter()
        .map(|(id, record)| {
            let name = if record.name.is_empty() {
                id.clone()
            } else {
                record.name
            };
            MarketplaceItem {
                id,
                name,
                source: "tenant",
                path: store_path.clone(),
                summary: record.description,
                readme: None,
                change: None,
                children: Vec::new(),
                extra: MarketplaceExtra::None,
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn annotate_plugins_with_diff(
    plugins: &mut Vec<MarketplaceItem>,
    state: &LastSyncState,
) -> MarketplaceDiff {
    let installed: BTreeSet<&str> = state.installed_plugins.iter().map(String::as_str).collect();
    let updated: BTreeSet<&str> = state.updated_plugins.iter().map(String::as_str).collect();
    let _removed: BTreeSet<&str> = state.removed_plugins.iter().map(String::as_str).collect();

    for item in plugins.iter_mut() {
        if installed.contains(item.id.as_str()) {
            item.change = Some(ChangeKind::Installed);
        } else if updated.contains(item.id.as_str()) {
            item.change = Some(ChangeKind::Updated);
        }
    }

    let present: BTreeSet<String> = plugins.iter().map(|p| p.id.clone()).collect();
    for removed_id in &state.removed_plugins {
        if !present.contains(removed_id) {
            plugins.push(MarketplaceItem {
                id: removed_id.clone(),
                name: removed_id.clone(),
                source: "tenant",
                path: String::new(),
                summary: Some("Removed in last sync".to_owned()),
                readme: None,
                change: Some(ChangeKind::Removed),
                children: Vec::new(),
                extra: MarketplaceExtra::None,
            });
        }
    }

    plugins.sort_by(|a, b| a.name.cmp(&b.name));

    MarketplaceDiff {
        installed: state.installed_plugins.clone(),
        updated: state.updated_plugins.clone(),
        removed: state.removed_plugins.clone(),
        last_applied_at: state.last_applied_at.clone(),
    }
}

fn list_plugins(root: &Path) -> Vec<MarketplaceItem> {
    let Ok(rd) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let name_os = entry.file_name();
        let Some(name) = name_os.to_str() else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        if !entry.file_type().ok().is_some_and(|t| t.is_dir()) {
            continue;
        }
        let manifest_path = first_existing(&[
            path.join(".claude-plugin").join("plugin.json"),
            path.join("claude-plugin").join("plugin.json"),
        ]);
        let manifest: Option<PluginManifest> = manifest_path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok());
        let summary = manifest.as_ref().and_then(|m| m.description.clone());
        let display_name = manifest
            .as_ref()
            .and_then(|m| m.name.clone())
            .unwrap_or_else(|| name.to_owned());
        let readme = read_first_existing(&[
            path.join("README.md"),
            path.join("readme.md"),
            path.join("README.txt"),
        ]);
        let extra = manifest.map_or(MarketplaceExtra::None, MarketplaceExtra::Plugin);
        out.push(MarketplaceItem {
            id: name.to_owned(),
            name: display_name,
            source: "tenant",
            path: path.display().to_string(),
            summary,
            readme,
            change: None,
            children: plugin_children(&path),
            extra,
        });
    }
    let mut children: Vec<Vec<PluginChild>> = out
        .iter_mut()
        .map(|p| std::mem::take(&mut p.children))
        .collect();
    mark_shared_mcp(&mut children);
    for (plugin, kids) in out.iter_mut().zip(children) {
        plugin.children = kids;
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[derive(Deserialize)]
struct McpJsonFile {
    #[serde(default, rename = "mcpServers")]
    mcp_servers: std::collections::BTreeMap<String, serde::de::IgnoredAny>,
}

pub fn plugin_children(plugin_dir: &Path) -> Vec<PluginChild> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(plugin_dir.join("skills")) {
        for entry in rd.flatten() {
            let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if id.starts_with('.') || !entry.file_type().ok().is_some_and(|t| t.is_dir()) {
                continue;
            }
            let body = std::fs::read_to_string(entry.path().join("SKILL.md")).ok();
            let (name, _) = body
                .as_deref()
                .map_or((None, None), parse_skill_frontmatter);
            out.push(PluginChild {
                kind: "skills",
                name: name.unwrap_or_else(|| id.clone()),
                id,
                shared: false,
            });
        }
    }
    if let Ok(rd) = std::fs::read_dir(plugin_dir.join("agents")) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|s| s.to_str()).map(str::to_owned) else {
                continue;
            };
            let body = std::fs::read_to_string(&path).ok();
            let (name, _) = body
                .as_deref()
                .map_or((None, None), parse_skill_frontmatter);
            out.push(PluginChild {
                kind: "agents",
                name: name.unwrap_or_else(|| id.clone()),
                id,
                shared: false,
            });
        }
    }
    if let Ok(bytes) = std::fs::read(plugin_dir.join("hooks").join("hooks.json"))
        && let Ok(file) =
            serde_json::from_slice::<crate::sync::apply::hooks_schema::HooksFile>(&bytes)
    {
        for event in file.hooks.keys() {
            out.push(PluginChild {
                kind: "hooks",
                id: event.clone(),
                name: event.clone(),
                shared: false,
            });
        }
    }
    if let Ok(bytes) = std::fs::read(plugin_dir.join(".mcp.json"))
        && let Ok(file) = serde_json::from_slice::<McpJsonFile>(&bytes)
    {
        for server in file.mcp_servers.keys() {
            out.push(PluginChild {
                kind: "mcp",
                id: server.clone(),
                name: server.clone(),
                shared: false,
            });
        }
    }
    out
}

pub fn mark_shared_mcp(plugin_children: &mut [Vec<PluginChild>]) {
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for children in plugin_children.iter() {
        for child in children.iter().filter(|c| c.kind == "mcp") {
            *counts.entry(child.id.clone()).or_insert(0) += 1;
        }
    }
    for children in plugin_children.iter_mut() {
        for child in children.iter_mut() {
            if child.kind == "mcp" && counts.get(&child.id).copied().unwrap_or(0) > 1 {
                child.shared = true;
            }
        }
    }
}

fn list_skills(dir: &Path) -> Vec<MarketplaceItem> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let name_os = entry.file_name();
        let Some(id) = name_os.to_str() else {
            continue;
        };
        if id.starts_with('.') {
            continue;
        }
        if !entry.file_type().ok().is_some_and(|t| t.is_dir()) {
            continue;
        }
        let skill_md = entry.path().join("SKILL.md");
        let body = std::fs::read_to_string(&skill_md).ok();
        let (frontmatter_name, summary) = body
            .as_deref()
            .map_or((None, None), parse_skill_frontmatter);
        let extra = MarketplaceExtra::Frontmatter(FrontmatterExtra {
            id: id.to_owned(),
            name: frontmatter_name.clone(),
            description: summary.clone(),
        });
        out.push(MarketplaceItem {
            id: id.to_owned(),
            name: frontmatter_name.unwrap_or_else(|| id.to_owned()),
            source: "tenant",
            path: entry.path().display().to_string(),
            summary,
            readme: body,
            change: None,
            children: Vec::new(),
            extra,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn list_agents(dir: &Path) -> Vec<MarketplaceItem> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let path = entry.path();
        if !entry.file_type().ok().is_some_and(|t| t.is_file()) {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let body = std::fs::read_to_string(&path).ok();
        let (frontmatter_name, summary) = body
            .as_deref()
            .map_or((None, None), parse_skill_frontmatter);
        let extra = MarketplaceExtra::Frontmatter(FrontmatterExtra {
            id: stem.to_owned(),
            name: frontmatter_name.clone(),
            description: summary.clone(),
        });
        out.push(MarketplaceItem {
            id: stem.to_owned(),
            name: frontmatter_name.unwrap_or_else(|| stem.to_owned()),
            source: "tenant",
            path: path.display().to_string(),
            summary,
            readme: body,
            change: None,
            children: Vec::new(),
            extra,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn parse_skill_frontmatter(body: &str) -> (Option<String>, Option<String>) {
    let trimmed = body.trim_start_matches('\u{feff}');
    let Some(rest) = trimmed.strip_prefix("---") else {
        return (None, None);
    };
    let rest = rest.strip_prefix('\n').unwrap_or(rest);
    let Some(end) = rest.find("\n---") else {
        return (None, None);
    };
    let block = &rest[..end];
    let mut name = None;
    let mut description = None;
    for line in block.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("name:") {
            name = Some(unquote(v.trim()).to_owned());
        } else if let Some(v) = line.strip_prefix("description:") {
            description = Some(unquote(v.trim()).to_owned());
        }
    }
    (name, description)
}

fn unquote(s: &str) -> &str {
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
        || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2)
    {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

fn list_registry_mcp(mcp_auth: &[McpServerAuth]) -> Vec<MarketplaceItem> {
    let registry = crate::mcp_registry::snapshot();
    let mut out = Vec::with_capacity(registry.len());
    for (slug, upstream) in registry.iter() {
        let proxy_url = crate::proxy::mcp_url(slug);
        let tools = mcp_auth
            .iter()
            .find(|s| s.id == *slug)
            .map(|s| s.tools.clone())
            .unwrap_or_default();
        out.push(MarketplaceItem {
            id: slug.clone(),
            name: slug.clone(),
            source: "tenant",
            path: upstream.url.as_str().to_owned(),
            summary: Some(proxy_url.clone()),
            readme: None,
            change: None,
            children: Vec::new(),
            extra: MarketplaceExtra::Mcp(McpServerEntry {
                url: Some(proxy_url),
                command: None,
                args: Vec::new(),
                transport: Some("http".to_owned()),
                tools,
            }),
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn first_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|c| c.is_file()).cloned()
}

fn read_first_existing(candidates: &[PathBuf]) -> Option<String> {
    for c in candidates {
        if let Ok(meta) = std::fs::metadata(c)
            && meta.is_file()
            && meta.len() <= README_MAX_BYTES as u64
            && let Ok(text) = std::fs::read_to_string(c)
        {
            return Some(text);
        }
    }
    None
}

pub fn listing_to_value(
    listing: &MarketplaceListing,
) -> Result<serde_json::Value, serde_json::Error> {
    serde_json::to_value(listing)
}
