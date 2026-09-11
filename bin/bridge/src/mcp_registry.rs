//! In-memory registry mapping `ManagedMcpServer` name → upstream URL + headers,
//! consumed by the proxy router for `/mcp/<name>`.
//!
//! Every entry belongs to the gateway whose manifest delivered it. The on-disk
//! fragment records that gateway, and a fragment written for another gateway is
//! never re-hydrated: forwarding a fresh token to the previous gateway's
//! upstreams is how a switched account used to sign itself out. A fragment in
//! the pre-stamp array shape is likewise left alone until a sync rewrites it.
//! Two gateways are the same when scheme, host and port agree; trailing
//! slashes and host case do not make a second gateway.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use arc_swap::ArcSwap;
use systemprompt_identifiers::ValidatedUrl;

use crate::gateway::manifest::ManagedMcpServer;

/// One managed upstream. `tool_policy` carries the manifest's per-tool
/// decisions as published, `*` standing for every tool.
#[derive(Clone, Debug)]
pub struct McpUpstream {
    pub url: ValidatedUrl,
    pub headers: BTreeMap<String, String>,
    pub display_name: String,
    pub transport: Option<String>,
    pub tool_policy: BTreeMap<String, systemprompt_models::bridge::ids::ToolPolicy>,
}

pub type McpRegistry = HashMap<String, McpUpstream>;

/// The persisted shape of `mcp-servers.json`: the servers plus the gateway
/// that delivered them.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct McpServersFragment {
    pub gateway: ValidatedUrl,
    pub servers: Vec<ManagedMcpServer>,
}

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
                tool_policy: s
                    .tool_policy
                    .iter()
                    .flatten()
                    .map(|(tool, policy)| (tool.as_str().to_owned(), *policy))
                    .collect(),
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

pub fn clear(slot: &McpRegistrySlot) {
    if slot.load().is_empty() {
        return;
    }
    slot.store(Arc::new(HashMap::new()));
    tracing::info!(target: "bridge::proxy", "managed MCP server registry cleared");
}

pub fn rehydrate_from_disk(slot: &McpRegistrySlot, gateway: &ValidatedUrl) -> std::io::Result<()> {
    let meta_dir = crate::config::paths::bridge_metadata_dir()
        .ok_or_else(|| std::io::Error::other("MCP registry metadata path unresolvable"))?;
    let path = meta_dir.join(crate::config::paths::MCP_SERVERS_FRAGMENT);
    let Some(body) = crate::fsutil::read_optional(&path)? else {
        return Ok(());
    };
    if serde_json::from_str::<Vec<ManagedMcpServer>>(&body).is_ok() {
        tracing::info!(
            target: "bridge::proxy",
            path = %path.display(),
            "MCP registry fragment predates gateway stamping; waiting for a sync"
        );
        return Ok(());
    }
    let fragment = serde_json::from_str::<McpServersFragment>(&body)
        .map_err(|e| std::io::Error::other(format!("parse {}: {e}", path.display())))?;
    if !same_origin(&fragment.gateway, gateway) {
        tracing::info!(
            target: "bridge::proxy",
            fragment_gateway = %fragment.gateway,
            gateway = %gateway,
            "MCP registry fragment belongs to another gateway; waiting for a sync"
        );
        return Ok(());
    }
    publish(slot, &fragment.servers);
    Ok(())
}

#[must_use]
pub fn same_origin(a: &ValidatedUrl, b: &ValidatedUrl) -> bool {
    match (url::Url::parse(a.as_str()), url::Url::parse(b.as_str())) {
        (Ok(a), Ok(b)) => a.origin() == b.origin(),
        _ => a.as_str().trim_end_matches('/') == b.as_str().trim_end_matches('/'),
    }
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
