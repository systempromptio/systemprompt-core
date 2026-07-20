//! Scanners for installed plugins and their child components, plus annotation
//! of the plugin list with the last sync's install/update/remove diff.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::sync::LastSyncState;

use super::frontmatter::parse_skill_frontmatter;
use super::{
    ChangeKind, MarketplaceDiff, MarketplaceExtra, MarketplaceItem, PluginChild, PluginManifest,
};

const README_MAX_BYTES: usize = 32 * 1024;

pub(super) fn plugin_dirs(root: &Path) -> Vec<PathBuf> {
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

pub(super) fn list_plugins(root: &Path) -> Vec<MarketplaceItem> {
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
#[expect(
    clippy::zero_sized_map_values,
    reason = "only the object keys (server names) are read; values are ignored"
)]
struct McpJsonFile {
    #[serde(default, rename = "mcpServers")]
    mcp_servers: BTreeMap<String, serde::de::IgnoredAny>,
}

fn plugin_children(plugin_dir: &Path) -> Vec<PluginChild> {
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

fn mark_shared_mcp(plugin_children: &mut [Vec<PluginChild>]) {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
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

pub(super) fn annotate_plugins_with_diff(
    plugins: &mut Vec<MarketplaceItem>,
    state: &LastSyncState,
) -> MarketplaceDiff {
    let installed: BTreeSet<&str> = state.installed_plugins.iter().map(String::as_str).collect();
    let updated: BTreeSet<&str> = state.updated_plugins.iter().map(String::as_str).collect();

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
