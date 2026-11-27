//! Managed remote MCP servers as the engine connects to them.
//!
//! Each server is named with the bridge's slug of its `name` (not prefixed with
//! the instance key, so a user server of the same name is replaced by the
//! instance's; two servers sharing a slug keep the last). A server the instance
//! announces at its own loopback container address is reached at the instance's
//! origin. Plain HTTP is accepted only on loopback. Only a server on the
//! instance's own origin is marked for the user's JWT; any other keeps its
//! declared headers only, and a declared `Authorization` header is never passed
//! on. A server with an explicit id must have a plain one; without one it is
//! known by its name, and a server with neither is skipped.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sp_protocol::ids::McpServerId;
use sp_protocol::names::{is_plain_id, mcp_server_slug};
use sp_protocol::origin::{bearer_allowed, is_transport_secure, rehome_instance_mcp_url};
use sp_protocol::wire::manifest::{ManagedMcpServer, Manifest};
use url::Url;

use crate::projection::model::ManagedMcp;

const AUTHORIZATION: &str = "authorization";

pub(crate) fn project(
    manifest: &Manifest,
    instance_url: &str,
    warnings: &mut Vec<String>,
) -> Vec<ManagedMcp> {
    let instance = Url::parse(instance_url).ok().filter(is_transport_secure);
    let mut out: Vec<ManagedMcp> = Vec::with_capacity(manifest.managed_mcp_servers.len());
    for server in &manifest.managed_mcp_servers {
        if server.id.as_deref().is_some_and(|id| !is_plain_id(id)) {
            warnings.push(format!(
                "MCP server {:?} skipped: its id is not a plain id",
                server.id()
            ));
            continue;
        }
        let Some(entry) = remote(server, instance.as_ref(), warnings) else {
            continue;
        };
        let shared = out.iter().position(|existing| existing.name == entry.name);
        match shared.and_then(|at| out.get_mut(at)) {
            Some(slot) => *slot = entry,
            None => out.push(entry),
        }
    }
    out
}

fn remote(
    server: &ManagedMcpServer,
    instance: Option<&Url>,
    warnings: &mut Vec<String>,
) -> Option<ManagedMcp> {
    let Ok(id) = McpServerId::try_new(server.id()) else {
        warnings.push(format!(
            "MCP server at {:?} skipped: it has neither an id nor a name",
            server.url
        ));
        return None;
    };
    let Ok(announced) = Url::parse(&server.url) else {
        warnings.push(format!(
            "MCP server {:?} skipped: its URL does not parse",
            server.id()
        ));
        return None;
    };
    let url = instance.map_or_else(
        || announced.clone(),
        |instance| rehome_instance_mcp_url(&announced, instance),
    );
    if !is_transport_secure(&url) {
        warnings.push(format!(
            "MCP server {:?} skipped: plain HTTP is only allowed on loopback",
            server.id()
        ));
        return None;
    }
    let bearer = instance.is_some_and(|instance| bearer_allowed(&url, instance));
    let headers = server
        .headers
        .iter()
        .filter(|(name, _)| !name.eq_ignore_ascii_case(AUTHORIZATION))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    Some(ManagedMcp {
        id,
        name: mcp_server_slug(&server.name),
        url: url.as_str().to_owned(),
        bearer,
        headers,
        tool_policy: server.tool_policy.clone(),
    })
}
