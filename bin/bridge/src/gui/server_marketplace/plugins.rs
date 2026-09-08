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

pub(super) fn plugin_dirs(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut dirs = Vec::new();
    for entry in super::read_dir_optional(root)? {
        if entry.file_type()?.is_dir() && !entry.file_name().to_string_lossy().starts_with('.') {
            dirs.push(entry.path());
        }
    }
    dirs.sort();
    Ok(dirs)
}

pub(super) fn list_plugins(root: &Path) -> std::io::Result<Vec<MarketplaceItem>> {
    let rd = super::read_dir_optional(root)?;
    let mut out = Vec::new();
    for entry in rd {
        let name_os = entry.file_name();
        let Some(name) = name_os.to_str() else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        if !entry.file_type()?.is_dir() {
            continue;
        }
        out.push(match read_plugin(name, &path) {
            Ok(item) => item,
            Err(e) => MarketplaceItem::failed(name, &path, &e),
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
    Ok(out)
}

fn read_plugin(name: &str, path: &Path) -> std::io::Result<MarketplaceItem> {
    let manifest: Option<PluginManifest> = read_first_existing(&[
        path.join(".claude-plugin").join("plugin.json"),
        path.join("claude-plugin").join("plugin.json"),
    ])?
    .map(|body| serde_json::from_str(&body))
    .transpose()
    .map_err(std::io::Error::other)?;
    let summary = manifest.as_ref().and_then(|m| m.description.clone());
    let display_name = manifest
        .as_ref()
        .and_then(|m| m.name.clone())
        .unwrap_or_else(|| name.to_owned());
    let readme = read_first_existing(&[
        path.join("README.md"),
        path.join("readme.md"),
        path.join("README.txt"),
    ])?;
    let version = manifest.as_ref().and_then(|m| m.version.clone());
    let author = manifest.as_ref().and_then(|m| m.author.clone());
    let homepage = manifest.as_ref().and_then(|m| m.homepage.clone());
    let extra = manifest.map_or(MarketplaceExtra::None, MarketplaceExtra::Plugin);
    Ok(MarketplaceItem {
        id: name.to_owned(),
        name: display_name,
        source: "tenant",
        path: path.display().to_string(),
        summary,
        readme,
        version,
        author,
        homepage,
        change: None,
        children: plugin_children(path)?,
        plugins: Vec::new(),
        extra,
        error: None,
    })
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

pub fn plugin_children(plugin_dir: &Path) -> std::io::Result<Vec<PluginChild>> {
    let mut out = Vec::new();
    {
        let rd = super::read_dir_optional(&plugin_dir.join("skills"))?;
        for entry in rd {
            let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if id.starts_with('.') || !entry.file_type()?.is_dir() {
                continue;
            }
            let body = Some(super::read_text(&entry.path().join("SKILL.md"))?);
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
    {
        let rd = super::read_dir_optional(&plugin_dir.join("agents"))?;
        for entry in rd {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|s| s.to_str()).map(str::to_owned) else {
                continue;
            };
            let body = Some(super::read_text(&path)?);
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
    if let Some(body) = super::read_optional_text(&plugin_dir.join("hooks").join("hooks.json"))? {
        let file: crate::sync::apply::hooks_schema::HooksFile =
            serde_json::from_str(&body).map_err(std::io::Error::other)?;
        for event in file.hooks.keys() {
            out.push(PluginChild {
                kind: "hooks",
                id: event.clone(),
                name: event.clone(),
                shared: false,
            });
        }
    }
    if let Some(body) = super::read_optional_text(&plugin_dir.join(".mcp.json"))? {
        let file: McpJsonFile = serde_json::from_str(&body).map_err(std::io::Error::other)?;
        for server in file.mcp_servers.keys() {
            out.push(PluginChild {
                kind: "mcp",
                id: server.clone(),
                name: server.clone(),
                shared: false,
            });
        }
    }
    Ok(out)
}

pub fn mark_shared_mcp(plugin_children: &mut [Vec<PluginChild>]) {
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

pub(super) fn annotate_with_diff(items: &mut [MarketplaceItem], state: &LastSyncState) {
    let installed: BTreeSet<&str> = state.installed_plugins.iter().map(String::as_str).collect();
    let updated: BTreeSet<&str> = state.updated_plugins.iter().map(String::as_str).collect();

    for item in items.iter_mut() {
        if installed.contains(item.id.as_str()) {
            item.change = Some(ChangeKind::Installed);
        } else if updated.contains(item.id.as_str()) {
            item.change = Some(ChangeKind::Updated);
        }
    }
}

pub(super) fn annotate_plugins_with_diff(
    plugins: &mut Vec<MarketplaceItem>,
    state: &LastSyncState,
) -> MarketplaceDiff {
    annotate_with_diff(plugins, state);

    let present: BTreeSet<String> = plugins.iter().map(|p| p.id.clone()).collect();
    for removed_id in &state.removed_plugins {
        if !present.contains(removed_id) {
            plugins.push(MarketplaceItem {
                id: removed_id.clone(),
                name: removed_id.clone(),
                source: "tenant",
                path: String::new(),
                summary: None,
                readme: None,
                version: None,
                author: None,
                homepage: None,
                change: Some(ChangeKind::Removed),
                children: Vec::new(),
                plugins: Vec::new(),
                extra: MarketplaceExtra::None,
                error: None,
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

fn read_first_existing(candidates: &[PathBuf]) -> std::io::Result<Option<String>> {
    for path in candidates {
        if let Some(body) = super::read_optional_text(path)? {
            if body.len() > README_MAX_BYTES {
                return Err(std::io::Error::other(format!(
                    "{} exceeds {README_MAX_BYTES} bytes",
                    path.display()
                )));
            }
            return Ok(Some(body));
        }
    }
    Ok(None)
}
