use crate::config::paths;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const README_MAX_BYTES: usize = 32 * 1024;

#[derive(Serialize)]
#[serde(untagged)]
enum MarketplaceExtra {
    Plugin(PluginManifest),
    Frontmatter(FrontmatterExtra),
    Mcp(McpServerEntry),
    None,
}

#[derive(Serialize)]
struct MarketplaceItem {
    id: String,
    name: String,
    source: &'static str,
    path: String,
    summary: Option<String>,
    readme: Option<String>,
    extra: MarketplaceExtra,
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

#[derive(Deserialize, Serialize, Default)]
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

#[derive(Serialize)]
struct FrontmatterExtra {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

#[derive(Deserialize, Serialize, Default)]
struct McpServerEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transport: Option<String>,
}

#[derive(Deserialize)]
struct McpRoot {
    #[serde(default)]
    #[serde(rename = "mcpServers")]
    mcp_servers: std::collections::BTreeMap<String, McpServerEntry>,
}

pub fn build_listing() -> MarketplaceListing {
    let loc = paths::org_plugins_effective();
    let plugins_dir = loc.as_ref().map(|l| l.path.display().to_string());

    let (plugins, skills, hooks, mcp, agents) = match loc {
        Some(loc) => {
            let plugins = list_plugins(&loc.path);
            let synthetic = loc.path.join(paths::SYNTHETIC_PLUGIN_NAME);
            let skills = list_skills(&synthetic.join("skills"));
            let agents = list_agents(&synthetic.join("agents"));
            let mcp = list_managed_mcp(&synthetic.join(".mcp.json"));
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
            .unwrap_or_else(|| name.to_string());
        let readme = read_first_existing(&[
            path.join("README.md"),
            path.join("readme.md"),
            path.join("README.txt"),
        ]);
        let extra = match manifest {
            Some(m) => MarketplaceExtra::Plugin(m),
            None => MarketplaceExtra::None,
        };
        out.push(MarketplaceItem {
            id: name.to_string(),
            name: display_name,
            source: "tenant",
            path: path.display().to_string(),
            summary,
            readme,
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
        if !entry.file_type().ok().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let skill_md = entry.path().join("SKILL.md");
        let body = std::fs::read_to_string(&skill_md).ok();
        let (frontmatter_name, summary) = body
            .as_deref()
            .map(parse_skill_frontmatter)
            .unwrap_or((None, None));
        let extra = MarketplaceExtra::Frontmatter(FrontmatterExtra {
            id: id.to_string(),
            name: frontmatter_name.clone(),
            description: summary.clone(),
        });
        out.push(MarketplaceItem {
            id: id.to_string(),
            name: frontmatter_name.unwrap_or_else(|| id.to_string()),
            source: "tenant",
            path: entry.path().display().to_string(),
            summary,
            readme: body,
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
        if !entry.file_type().ok().map(|t| t.is_file()).unwrap_or(false) {
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
            .map(parse_skill_frontmatter)
            .unwrap_or((None, None));
        let extra = MarketplaceExtra::Frontmatter(FrontmatterExtra {
            id: stem.to_string(),
            name: frontmatter_name.clone(),
            description: summary.clone(),
        });
        out.push(MarketplaceItem {
            id: stem.to_string(),
            name: frontmatter_name.unwrap_or_else(|| stem.to_string()),
            source: "tenant",
            path: path.display().to_string(),
            summary,
            readme: body,
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
            name = Some(unquote(v.trim()).to_string());
        } else if let Some(v) = line.strip_prefix("description:") {
            description = Some(unquote(v.trim()).to_string());
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

fn list_managed_mcp(path: &Path) -> Vec<MarketplaceItem> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let Ok(root) = serde_json::from_slice::<McpRoot>(&bytes) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (name, entry) in root.mcp_servers {
        let summary = entry.url.clone();
        out.push(MarketplaceItem {
            id: name.clone(),
            name,
            source: "tenant",
            path: path.display().to_string(),
            summary,
            readme: None,
            extra: MarketplaceExtra::Mcp(entry),
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

pub fn listing_to_json(listing: &MarketplaceListing) -> Result<String, serde_json::Error> {
    serde_json::to_string(listing)
}

pub fn listing_to_value(
    listing: &MarketplaceListing,
) -> Result<serde_json::Value, serde_json::Error> {
    serde_json::to_value(listing)
}
