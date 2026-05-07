//! In-memory registry mapping `ManagedMcpServer` name → upstream URL + manifest
//! headers.
//!
//! Cross-cutting bridge state. Populated by `sync::apply` after a successful
//! manifest apply; consumed by `proxy::forward` (route `/mcp/<name>` to the
//! registered upstream) and by `install::mdm::*` (emit the managed-server JSON
//! for the platform MDM channel). Empty until a manifest is applied; an unknown
//! name yields a 404 from the proxy.

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

pub fn publish(servers: &[ManagedMcpServer]) {
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

// Must be stable + deterministic so the synthetic plugin writer and the proxy
// router agree on the key for `/mcp/<slug>` routing.
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
        "mcp-server".to_string()
    } else {
        out
    }
}
