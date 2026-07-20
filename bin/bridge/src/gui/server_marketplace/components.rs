//! Scanners for the per-component marketplace categories: skills, agents,
//! registry MCP servers, and Cowork artifacts. Each returns items sorted by
//! display name.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use crate::proxy::mcp_probe::McpServerAuth;

use super::frontmatter::parse_skill_frontmatter;
use super::{FrontmatterExtra, MarketplaceExtra, MarketplaceItem, McpServerEntry};

pub(super) fn list_skills(dir: &Path) -> Vec<MarketplaceItem> {
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

pub(super) fn list_agents(dir: &Path) -> Vec<MarketplaceItem> {
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

pub(super) fn list_artifacts() -> Vec<MarketplaceItem> {
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

pub(super) fn list_registry_mcp(mcp_auth: &[McpServerAuth]) -> Vec<MarketplaceItem> {
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
