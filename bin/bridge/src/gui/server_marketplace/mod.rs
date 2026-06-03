pub mod hooks;

use crate::config::paths;
use crate::proxy::mcp_probe::McpServerAuth;
use crate::sync::{LastSyncState, read_last_sync};
use serde::{Deserialize, Serialize};
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
    id: String,
    name: String,
    source: &'static str,
    path: String,
    summary: Option<String>,
    readme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    change: Option<ChangeKind>,
    extra: MarketplaceExtra,
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

    let last_sync = paths::bridge_metadata_dir().and_then(|meta| {
        read_last_sync(&meta.join(paths::LAST_SYNC_SENTINEL))
            .ok()
            .flatten()
    });

    let (mut plugins, skills, hooks, mcp, agents) = match loc {
        Some(loc) => {
            let plugins = list_plugins(&loc.path);
            let synthetic = loc.path.join(paths::SYNTHETIC_PLUGIN_NAME);
            let skills = list_skills(&synthetic.join("skills"));
            let agents = list_agents(&synthetic.join("agents"));
            let mcp = list_registry_mcp(mcp_auth);
            let hooks = hooks::list_hooks(&synthetic.join("hooks"));
            (plugins, skills, hooks, mcp, agents)
        },
        None => (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()),
    };

    let last_sync_diff = last_sync
        .as_ref()
        .map_or_else(MarketplaceDiff::default, |state| {
            annotate_plugins_with_diff(&mut plugins, state)
        });

    MarketplaceListing {
        plugins,
        skills,
        hooks,
        mcp,
        agents,
        plugins_dir,
        last_sync_diff,
    }
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
            extra,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
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
