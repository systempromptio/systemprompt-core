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

use systemprompt_device::protocol::manifest::Manifest;
use systemprompt_device::protocol::names::{is_plain_id, mcp_server_slug};
use systemprompt_device::protocol::origin::{
    bearer_allowed, is_transport_secure, rehome_instance_mcp_url,
};
use systemprompt_models::bridge::manifest::ManagedMcpServer;
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
        if server.id != server.name && !is_plain_id(server.id.as_str()) {
            warnings.push(format!(
                "MCP server {:?} skipped: its id is not a plain id",
                server.id.as_str()
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
    let id = server.id.clone();
    let Ok(announced) = Url::parse(server.url.as_str()) else {
        warnings.push(format!(
            "MCP server {:?} skipped: its URL does not parse",
            server.id.as_str()
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
            server.id.as_str()
        ));
        return None;
    }
    let bearer = instance.is_some_and(|instance| bearer_allowed(&url, instance));
    let headers = server
        .headers
        .iter()
        .flatten()
        .filter(|(name, _)| !name.eq_ignore_ascii_case(AUTHORIZATION))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    Some(ManagedMcp {
        id,
        name: mcp_server_slug(server.name.as_str()),
        url: url.as_str().to_owned(),
        bearer,
        headers,
        tool_policy: server
            .tool_policy
            .iter()
            .flatten()
            .map(|(tool, policy)| (tool.as_str().to_owned(), *policy))
            .collect(),
    })
}
