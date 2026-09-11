//! The tool names each managed MCP server last reported, kept so Claude
//! Desktop's per-tool `toolPolicy` can be written for a server whose manifest
//! entry says "every tool".
//!
//! Claude Desktop has no server-wide switch: `managedMcpServers[].toolPolicy`
//! names tools one by one. The bridge already learns the names through its
//! MCP auth probe (`initialize` → `tools/list`); this file remembers them so
//! a policy write never depends on the server answering at that moment. A
//! server that fails a probe keeps the names it reported last time. An
//! absent file is an empty catalog; an unreadable or corrupt one is an error,
//! never an empty catalog, so a transient read failure cannot wipe every
//! server's names on the next write. Servers that leave the manifest are
//! dropped so a retired server's names never leak into a later policy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::proxy::mcp_probe::{McpAuthState, McpServerAuth};

const FILE: &str = "mcp-tools.json";

pub type ToolCatalog = BTreeMap<String, Vec<String>>;

fn path() -> Option<PathBuf> {
    crate::config::paths::bridge_metadata_dir().map(|dir| dir.join(FILE))
}

pub fn read() -> std::io::Result<ToolCatalog> {
    let Some(path) = path() else {
        return Ok(ToolCatalog::new());
    };
    match std::fs::read_to_string(&path) {
        Ok(body) => serde_json::from_str(&body).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("{}: {e}", path.display()),
            )
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ToolCatalog::new()),
        Err(e) => Err(std::io::Error::new(
            e.kind(),
            format!("{}: {e}", path.display()),
        )),
    }
}

pub fn record(results: &[McpServerAuth]) -> std::io::Result<ToolCatalog> {
    let mut catalog = read()?;
    for result in results {
        if result.state != McpAuthState::Authenticated || result.id.is_empty() {
            continue;
        }
        let mut names: Vec<String> = result.tools.iter().map(|t| t.name.clone()).collect();
        names.sort();
        names.dedup();
        catalog.insert(result.id.clone(), names);
    }
    if let Some(path) = path() {
        let body = serde_json::to_string_pretty(&catalog).map_err(std::io::Error::other)?;
        crate::fsutil::atomic_write_0644(&path, format!("{body}\n").as_bytes())?;
    }
    Ok(catalog)
}

pub fn retain(slugs: &[String]) -> std::io::Result<()> {
    let mut catalog = read()?;
    let before = catalog.len();
    catalog.retain(|slug, _| slugs.contains(slug));
    if catalog.len() == before {
        return Ok(());
    }
    if let Some(path) = path() {
        let body = serde_json::to_string_pretty(&catalog).map_err(std::io::Error::other)?;
        crate::fsutil::atomic_write_0644(&path, format!("{body}\n").as_bytes())?;
    }
    Ok(())
}
