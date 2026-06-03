//! In-memory registry mapping `ManagedMcpServer` name → upstream URL + manifest
//! headers.
//!
//! Populated by `sync::apply`; consumed by `proxy::forward` (routes
//! `/mcp/<name>`) and `install::mdm::*`. An unknown name yields a 404.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, OnceLock};

use arc_swap::ArcSwap;
use systemprompt_identifiers::ValidatedUrl;

use crate::gateway::manifest::ManagedMcpServer;

#[derive(Clone, Debug)]
pub(crate) struct McpUpstream {
    pub url: ValidatedUrl,
    pub headers: BTreeMap<String, String>,
}

pub(crate) type McpRegistry = HashMap<String, McpUpstream>;

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
pub(crate) fn snapshot() -> Arc<McpRegistry> {
    slot().load_full()
}

// Must be deterministic: the synthetic plugin writer and proxy router rely on
// the same key for `/mcp/<slug>` routing.
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
