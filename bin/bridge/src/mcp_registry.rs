//! In-memory registry mapping `ManagedMcpServer` name → upstream URL + headers,
//! consumed by the proxy router for `/mcp/<name>`.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, OnceLock};

use arc_swap::ArcSwap;
use systemprompt_identifiers::ValidatedUrl;

use crate::gateway::manifest::ManagedMcpServer;

#[derive(Clone, Debug)]
pub struct McpUpstream {
    pub url: ValidatedUrl,
    pub headers: BTreeMap<String, String>,
}

pub type McpRegistry = HashMap<String, McpUpstream>;

static REGISTRY: OnceLock<ArcSwap<McpRegistry>> = OnceLock::new();

fn slot() -> &'static ArcSwap<McpRegistry> {
    REGISTRY.get_or_init(|| ArcSwap::from_pointee(HashMap::new()))
}

pub(crate) fn publish(servers: &[ManagedMcpServer]) {
    let mut next: McpRegistry = HashMap::with_capacity(servers.len());
    for s in servers {
        next.insert(
            normalize_key(s.name.as_str()),
            McpUpstream {
                url: s.url.clone(),
                headers: s.headers.clone().unwrap_or_default(),
            },
        );
    }
    slot().store(Arc::new(next));
    tracing::info!(
        target: "bridge::proxy",
        count = servers.len(),
        "managed MCP server registry updated"
    );
}

#[must_use]
pub fn snapshot() -> Arc<McpRegistry> {
    slot().load_full()
}

/// Lets the proxy serve `/mcp/<name>` at boot without waiting for a
/// credentialed sync.
pub fn rehydrate_from_disk() {
    let Some(meta_dir) = crate::config::paths::bridge_metadata_dir() else {
        return;
    };
    let path = meta_dir.join(crate::config::paths::MCP_SERVERS_FRAGMENT);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => {
            tracing::debug!(target: "bridge::proxy", error = %e, path = %path.display(), "mcp registry: read fragment failed");
            return;
        },
    };
    match serde_json::from_slice::<Vec<ManagedMcpServer>>(&bytes) {
        Ok(servers) => publish(&servers),
        Err(e) => {
            tracing::debug!(target: "bridge::proxy", error = %e, path = %path.display(), "mcp registry: parse fragment failed");
        },
    }
}

// Must be deterministic: synthetic-plugin writer and proxy router share this
// key.
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
