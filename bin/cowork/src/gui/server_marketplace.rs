use crate::config::paths;
use serde::Serialize;
use std::path::{Path, PathBuf};

const README_MAX_BYTES: usize = 32 * 1024;

#[derive(Serialize)]
struct MarketplaceItem {
    id: String,
    name: String,
    source: &'static str,
    path: String,
    summary: Option<String>,
    readme: Option<String>,
    extra: serde_json::Value,
}

#[derive(Serialize)]
pub struct MarketplaceListing {
    plugins: Vec<MarketplaceItem>,
    skills: Vec<MarketplaceItem>,
    hooks: Vec<MarketplaceItem>,
    mcp: Vec<MarketplaceItem>,
    agents: Vec<MarketplaceItem>,
    plugins_dir: Option<String>,
}

pub fn build_listing() -> MarketplaceListing {
    let loc = paths::org_plugins_effective();
    let plugins_dir = loc.as_ref().map(|l| l.path.display().to_string());

    let (plugins, skills, hooks, mcp, agents) = match loc {
        Some(loc) => {
            let plugins = list_plugins(&loc.path);
            let meta = paths::metadata_dir(&loc.path);
            let skills = list_index_items(&meta.join(paths::SKILLS_DIR), "skill");
            let agents = list_index_items(&meta.join(paths::AGENTS_DIR), "agent");
            let mcp = list_managed_mcp(&meta.join(paths::MANAGED_MCP_FRAGMENT));
            let hooks = Vec::new();
            (plugins, skills, hooks, mcp, agents)
        },
        None => (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()),
    };

    MarketplaceListing {
        plugins,
        skills,
        hooks,
        mcp,
        agents,
        plugins_dir,
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
        if !entry.file_type().ok().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let manifest_path = path.join("claude-plugin").join("plugin.json");
        let manifest: Option<serde_json::Value> = std::fs::read(&manifest_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        let summary = manifest
            .as_ref()
            .and_then(|m| m.get("description"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let readme = read_first_existing(&[
            path.join("README.md"),
            path.join("readme.md"),
            path.join("README.txt"),
        ]);
        out.push(MarketplaceItem {
            id: name.to_string(),
            name: manifest
                .as_ref()
                .and_then(|m| m.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or(name)
                .to_string(),
            source: "tenant",
            path: path.display().to_string(),
            summary,
            readme,
            extra: manifest.unwrap_or(serde_json::Value::Null),
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn list_index_items(dir: &Path, kind: &'static str) -> Vec<MarketplaceItem> {
    let index_path = dir.join("index.json");
    let Ok(bytes) = std::fs::read(&index_path) else {
        return Vec::new();
    };
    let Ok(values) = serde_json::from_slice::<Vec<serde_json::Value>>(&bytes) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for v in values {
        let id = v
            .get("id")
            .and_then(|x| x.as_str())
            .or_else(|| v.get("name").and_then(|x| x.as_str()))
            .unwrap_or("(unnamed)")
            .to_string();
        let name = v
            .get("display_name")
            .and_then(|x| x.as_str())
            .or_else(|| v.get("name").and_then(|x| x.as_str()))
            .unwrap_or(&id)
            .to_string();
        let summary = v
            .get("description")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string());
        let item_dir = dir.join(&id);
        let readme = if kind == "skill" {
            read_first_existing(&[item_dir.join("SKILL.md"), item_dir.join("README.md")])
        } else {
            read_first_existing(&[item_dir.join("README.md")])
        };
        out.push(MarketplaceItem {
            id: id.clone(),
            name,
            source: "tenant",
            path: item_dir.display().to_string(),
            summary,
            readme,
            extra: v,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn list_managed_mcp(path: &Path) -> Vec<MarketplaceItem> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let Ok(values) = serde_json::from_slice::<Vec<serde_json::Value>>(&bytes) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for v in values {
        let id = v
            .get("id")
            .and_then(|x| x.as_str())
            .or_else(|| v.get("name").and_then(|x| x.as_str()))
            .unwrap_or("(unnamed)")
            .to_string();
        let name = v
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or(&id)
            .to_string();
        let summary = v
            .get("description")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                v.get("command")
                    .and_then(|x| x.as_str())
                    .map(|s| format!("command: {s}"))
            });
        out.push(MarketplaceItem {
            id: id.clone(),
            name,
            source: "tenant",
            path: path.display().to_string(),
            summary,
            readme: None,
            extra: v,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn read_first_existing(candidates: &[PathBuf]) -> Option<String> {
    for c in candidates {
        if let Ok(meta) = std::fs::metadata(c) {
            if meta.is_file() && meta.len() <= README_MAX_BYTES as u64 {
                if let Ok(text) = std::fs::read_to_string(c) {
                    return Some(text);
                }
            }
        }
    }
    None
}

pub fn listing_to_json(listing: &MarketplaceListing) -> Result<String, serde_json::Error> {
    serde_json::to_string(listing)
}
