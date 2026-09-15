//! The `mcp.<slug>` remote entries the bridge registers in the user's global
//! `opencode.json`. Every foreign key survives, including MCP servers the user
//! added; only entries pointing at the loopback proxy are bridge-owned.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Map, Value, json};

use crate::gateway::manifest::ManagedMcpServer;
use crate::host_sync::ApplyError;
use crate::integration::json_io::{object_entry, read_json_object, write_json};
use crate::proxy::LoopbackEndpoint;

use super::super::config::user_config_path;

const MCP_TABLE: &str = "mcp";

pub(super) fn write_mcp_blocks(
    loopback: &LoopbackEndpoint,
    servers: &[ManagedMcpServer],
) -> Result<(), ApplyError> {
    let path = user_config_path();
    let original = read_json_object(&path)?;
    let mut value = original.clone();

    let sidecar = crate::integration::mcp_sidecar::beside(&path);
    strip_bridge_servers(
        &crate::integration::mcp_sidecar::read(&sidecar)?,
        &mut value,
    );
    let mut written = Vec::with_capacity(servers.len());
    if !servers.is_empty() {
        let bearer = loopback
            .host_bearer(&crate::ids::HostId::new("opencode"))
            .map_err(|e| ApplyError::Io {
                context: "derive opencode host token for mcp".into(),
                source: e,
            })?;
        let table = object_entry(&mut value, &path, MCP_TABLE)?;
        for s in servers {
            let slug = crate::mcp_registry::normalize_key(s.name.as_str());
            table.insert(
                slug.clone(),
                json!({
                    "type": "remote",
                    "url": loopback.mcp_url(&slug),
                    "headers": { "Authorization": bearer },
                    "enabled": true,
                }),
            );
            written.push(slug);
        }
    }
    crate::integration::mcp_sidecar::write(&sidecar, &written)?;

    if value == original {
        return Ok(());
    }
    write_json(&path, &Value::Object(value))
}

fn strip_bridge_servers(recorded: &[String], root: &mut Map<String, Value>) {
    let Some(Value::Object(table)) = root.get_mut(MCP_TABLE) else {
        return;
    };
    table.retain(|name, _| !recorded.iter().any(|r| r == name));
    if table.is_empty() {
        root.remove(MCP_TABLE);
    }
}
