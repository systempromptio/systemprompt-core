//! Scanners for the per-component marketplace categories: skills, agents,
//! registry MCP servers, and Cowork artifacts. Each returns items sorted by
//! display name.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;


use super::frontmatter::parse_skill_frontmatter;
use super::{FrontmatterExtra, MarketplaceExtra, MarketplaceItem, McpServerEntry};

pub(super) fn list_skills(dir: &Path) -> std::io::Result<Vec<MarketplaceItem>> {
    let rd = super::read_dir_optional(dir)?;
    let mut out = Vec::new();
    for entry in rd {
        let name_os = entry.file_name();
        let Some(id) = name_os.to_str() else {
            continue;
        };
        if id.starts_with('.') {
            continue;
        }
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let skill_md = entry.path().join("SKILL.md");
        let body = match super::read_text(&skill_md) {
            Ok(body) => Some(body),
            Err(e) => {
                out.push(MarketplaceItem::failed(id, &entry.path(), &e));
                continue;
            },
        };
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
            version: None,
            author: None,
            homepage: None,
            change: None,
            children: Vec::new(),
            plugins: Vec::new(),
            extra,
            error: None,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub(super) fn list_agents(dir: &Path) -> std::io::Result<Vec<MarketplaceItem>> {
    list_markdown_dir(dir)
}

pub(super) fn list_rules(dir: &Path) -> std::io::Result<Vec<MarketplaceItem>> {
    list_markdown_dir(dir)
}

fn list_markdown_dir(dir: &Path) -> std::io::Result<Vec<MarketplaceItem>> {
    let rd = super::read_dir_optional(dir)?;
    let mut out = Vec::new();
    for entry in rd {
        let path = entry.path();
        if !entry.file_type()?.is_file() {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let body = match super::read_text(&path) {
            Ok(body) => Some(body),
            Err(e) => {
                out.push(MarketplaceItem::failed(stem, &path, &e));
                continue;
            },
        };
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
            version: None,
            author: None,
            homepage: None,
            change: None,
            children: Vec::new(),
            plugins: Vec::new(),
            extra,
            error: None,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
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
                record.name.clone()
            };
            MarketplaceItem {
                id,
                name,
                source: "tenant",
                path: store_path.clone(),
                summary: record.description,
                readme: None,
                version: None,
                author: None,
                homepage: None,
                change: None,
                children: Vec::new(),
                plugins: record.plugins,
                extra: MarketplaceExtra::None,
                error: None,
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub(super) fn list_registry_mcp(
    loopback: &crate::proxy::LoopbackEndpoint,
    registry: &crate::mcp_registry::McpRegistry,
) -> Vec<MarketplaceItem> {
    let mut out = Vec::with_capacity(registry.len());
    for (slug, upstream) in registry {
        let proxy_url = loopback.mcp_url(slug);
        let upstream_url = upstream.url.as_str().to_owned();
        out.push(MarketplaceItem {
            id: slug.clone(),
            name: upstream.display_name.clone(),
            source: "tenant",
            path: upstream_url.clone(),
            summary: None,
            readme: None,
            version: None,
            author: None,
            homepage: None,
            change: None,
            children: Vec::new(),
            plugins: Vec::new(),
            extra: MarketplaceExtra::Mcp(McpServerEntry {
                proxy_url: Some(proxy_url),
                upstream_url: Some(upstream_url),
                command: None,
                args: Vec::new(),
                transport: upstream
                    .transport
                    .clone()
                    .or_else(|| Some("http".to_owned())),
            }),
            error: None,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}
