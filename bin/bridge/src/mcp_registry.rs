//! In-memory registry mapping `ManagedMcpServer` name → upstream URL + headers,
//! consumed by the proxy router for `/mcp/<name>`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use arc_swap::ArcSwap;
use systemprompt_identifiers::ValidatedUrl;

use crate::gateway::manifest::ManagedMcpServer;

#[derive(Clone, Debug)]
pub struct McpUpstream {
    pub url: ValidatedUrl,
    pub headers: BTreeMap<String, String>,
    pub display_name: String,
    pub transport: Option<String>,
}

pub type McpRegistry = HashMap<String, McpUpstream>;

/// The hot-swappable registry a process owns: the proxy router reads it on
/// every `/mcp/<name>` and sync replaces it wholesale.
pub type McpRegistrySlot = ArcSwap<McpRegistry>;

#[must_use]
pub fn empty_slot() -> Arc<McpRegistrySlot> {
    Arc::new(ArcSwap::from_pointee(HashMap::new()))
}

pub(crate) fn publish(slot: &McpRegistrySlot, servers: &[ManagedMcpServer]) {
    let mut next: McpRegistry = HashMap::with_capacity(servers.len());
    for s in servers {
        next.insert(
            normalize_key(s.name.as_str()),
            McpUpstream {
                url: s.url.clone(),
                headers: s.headers.clone().unwrap_or_default(),
                display_name: s.name.as_str().to_owned(),
                transport: s.transport.clone(),
            },
        );
    }
    slot.store(Arc::new(next));
    tracing::info!(
        target: "bridge::proxy",
        count = servers.len(),
        "managed MCP server registry updated"
    );
}

#[must_use]
pub fn snapshot(slot: &McpRegistrySlot) -> Arc<McpRegistry> {
    slot.load_full()
}

pub fn rehydrate_from_disk(slot: &McpRegistrySlot) -> std::io::Result<()> {
    let meta_dir = crate::config::paths::bridge_metadata_dir()
        .ok_or_else(|| std::io::Error::other("MCP registry metadata path unresolvable"))?;
    let path = meta_dir.join(crate::config::paths::MCP_SERVERS_FRAGMENT);
    let Some(body) = crate::fsutil::read_optional(&path)? else {
        return Ok(());
    };
    let servers = serde_json::from_str::<Vec<ManagedMcpServer>>(&body)
        .map_err(|e| std::io::Error::other(format!("parse {}: {e}", path.display())))?;
    publish(slot, &servers);
    Ok(())
}

#[must_use]
pub fn normalize_key(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_dash = true;
    for c in name.chars() {
        let is_safe = c.is_ascii_alphanumeric() || c == '_';
        if is_safe {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "mcp-server".to_owned()
    } else {
        out
    }
}
