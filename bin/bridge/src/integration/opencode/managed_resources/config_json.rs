//! The bridge's entries in the user's V2 `opencode.json`: `mcp.servers.<slug>`
//! remote servers pointing at the loopback proxy, and the client defaults a
//! marketplace declares. Every foreign key survives, including MCP servers the
//! user added; only server names the sidecar recorded are bridge-owned. Each
//! entry sets `oauth: false`: the loopback bearer is the credential, and
//! `OpenCode` otherwise answers a 401 with OAuth client registration against
//! the proxy, which has no OAuth server. A
//! client default is set only where the user has not set the key, and is
//! never removed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde_json::{Map, Value};
use systemprompt_models::bridge::opencode::{McpOAuth, McpServer};

use crate::gateway::manifest::{ManagedMcpServer, SignedManifest};
use crate::host_sync::ApplyError;
use crate::integration::json_io::{object_entry, read_json_object, write_json};
use crate::proxy::LoopbackEndpoint;

use super::super::config::user_config_path;

const MCP_TABLE: &str = "mcp";
const SERVERS: &str = "servers";

pub(super) fn write_mcp_servers(
    loopback: &LoopbackEndpoint,
    servers: &[ManagedMcpServer],
) -> Result<(), ApplyError> {
    let path = user_config_path();
    let original = read_json_object(&path)?;
    let mut value = original.clone();

    let sidecar = crate::integration::mcp_sidecar::beside(&path);
    let recorded = crate::integration::mcp_sidecar::read(&sidecar)?;
    strip_bridge_servers(&recorded, &mut value);
    let mut written = Vec::with_capacity(servers.len());
    if !servers.is_empty() {
        let bearer = loopback
            .host_bearer(super::super::TOKEN_HOST)
            .map_err(|e| ApplyError::Io {
                context: "derive opencode host token for mcp".into(),
                source: e,
            })?;
        let mcp = object_entry(&mut value, &path, MCP_TABLE)?;
        let table = object_entry(mcp, &path, SERVERS)?;
        for s in servers {
            let slug = crate::mcp_registry::normalize_key(s.name.as_str());
            let server = McpServer::Remote {
                url: loopback.mcp_url(&slug),
                headers: BTreeMap::from([("Authorization".to_owned(), bearer.clone())]),
                codemode: None,
                oauth: Some(McpOAuth::Disabled),
            };
            let entry = serde_json::to_value(server).map_err(|source| ApplyError::Serialize {
                what: format!("OpenCode MCP server {slug}"),
                source,
            })?;
            table.insert(slug.clone(), entry);
            written.push(slug);
        }
    }
    crate::integration::mcp_sidecar::write(&sidecar, &written)?;

    if value == original {
        return Ok(());
    }
    write_json(&path, &Value::Object(value))
}

// JSON: OpenCode `opencode.json` — foreign config, unknown keys must be
// preserved.
fn strip_bridge_servers(recorded: &[String], root: &mut Map<String, Value>) {
    let Some(Value::Object(mcp)) = root.get_mut(MCP_TABLE) else {
        return;
    };
    if let Some(Value::Object(table)) = mcp.get_mut(SERVERS) {
        table.retain(|name, _| !recorded.iter().any(|r| r == name));
        if table.is_empty() {
            mcp.remove(SERVERS);
        }
    }
    mcp.retain(|name, _| name == SERVERS || !recorded.iter().any(|r| r == name));
    if mcp.is_empty() {
        root.remove(MCP_TABLE);
    }
}

pub(super) fn apply_cli_defaults(
    manifest: &SignedManifest,
    projected_themes: &[String],
) -> Result<(), ApplyError> {
    let mut defaults: Map<String, Value> = Map::new();
    for marketplace in &manifest.marketplaces {
        let Some(opencode) = &marketplace.opencode else {
            continue;
        };
        for (key, value) in &opencode.cli_defaults {
            defaults.entry(key.clone()).or_insert_with(|| value.clone());
        }
        if let Some(theme) = &opencode.theme
            && projected_themes.contains(&theme.name)
        {
            defaults
                .entry("theme".to_owned())
                .or_insert_with(|| Value::String(theme.name.clone()));
        }
    }
    if defaults.is_empty() {
        return Ok(());
    }
    let path = user_config_path();
    let original = read_json_object(&path)?;
    let mut value = original.clone();
    for (key, default) in defaults {
        value.entry(key).or_insert(default);
    }
    if value == original {
        return Ok(());
    }
    write_json(&path, &Value::Object(value))
}
